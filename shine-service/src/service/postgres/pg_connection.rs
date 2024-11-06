use crate::service::cacerts::{get_root_cert_store, CertError};
use async_trait::async_trait;
use bb8::{ManageConnection, Pool as BB8Pool, PooledConnection, RunError};
use bb8_postgres::PostgresConnectionManager;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{collections::HashMap, ops::DerefMut};
use thiserror::Error as ThisError;
use tokio::sync::RwLock;
use tokio_postgres::{Config as PGConfig, GenericClient, Statement};
use tokio_postgres_rustls::MakeRustlsConnect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PGStatementId(usize);

pub trait PGRawConnection: GenericClient {}
impl<T> PGRawConnection for T where T: GenericClient {}

pub struct PGConnection<T>
where
    T: PGRawConnection,
{
    prepared_statements: Arc<RwLock<HashMap<usize, Statement>>>,
    prepared_statement_id: Arc<AtomicUsize>,
    client: T,
}

impl<T: PGRawConnection> PGConnection<T> {
    #[inline]
    pub async fn create_statement(&self, prepared: Statement) -> PGStatementId {
        let id = self.prepared_statement_id.fetch_add(1, Ordering::Relaxed);
        self.set_statement(PGStatementId(id), prepared).await;
        PGStatementId(id)
    }

    #[inline]
    pub async fn get_statement(&self, prepared_id: PGStatementId) -> Option<Statement> {
        let prepared_statements = self.prepared_statements.read().await;
        prepared_statements.get(&prepared_id.0).cloned()
    }

    #[inline]
    pub async fn set_statement(&self, prepared_id: PGStatementId, prepared: Statement) {
        let mut prepared_statements = self.prepared_statements.write().await;
        prepared_statements.insert(prepared_id.0, prepared);
    }

    #[inline]
    pub async fn transaction(&mut self) -> Result<PGConnection<PGRawTransaction<'_>>, PGError> {
        Ok(PGConnection {
            prepared_statements: self.prepared_statements.clone(),
            prepared_statement_id: self.prepared_statement_id.clone(),
            client: self.client.transaction().await?,
        })
    }
}

impl PGConnection<PGRawClient> {
    fn new(pg_client: PGRawClient, prepared_statement_id: Arc<AtomicUsize>) -> Self {
        Self {
            client: pg_client,
            prepared_statement_id,
            prepared_statements: Arc::new(RwLock::new(HashMap::default())),
        }
    }
}

impl<'a> PGConnection<PGRawTransaction<'a>> {
    pub async fn commit(self) -> Result<(), PGError> {
        self.client.commit().await
    }

    pub async fn rollback(self) -> Result<(), PGError> {
        self.client.rollback().await
    }
}

impl<T: PGRawConnection> Deref for PGConnection<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl<T: PGRawConnection> DerefMut for PGConnection<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

pub struct PGConnectionManager {
    connection_manager: PostgresConnectionManager<MakeRustlsConnect>,
    prepared_statement_id: Arc<AtomicUsize>,
}

impl PGConnectionManager {
    pub fn new(config: PGConfig, tls: MakeRustlsConnect) -> Self {
        Self {
            connection_manager: PostgresConnectionManager::new(config, tls),
            prepared_statement_id: Arc::new(AtomicUsize::new(1)),
        }
    }
}

#[async_trait]
impl bb8::ManageConnection for PGConnectionManager {
    type Connection = PGConnection<PGRawClient>;
    type Error = PGError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let conn = self.connection_manager.connect().await?;
        Ok(PGConnection::new(conn, self.prepared_statement_id.clone()))
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.simple_query("").await.map(|_| ())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.connection_manager.has_broken(&mut conn.client)
    }
}

pub type PGConnectionError = RunError<<PGConnectionManager as ManageConnection>::Error>;
pub type PGConnectionPool = BB8Pool<PGConnectionManager>;
pub type PGPooledConnection<'a> = PooledConnection<'a, PGConnectionManager>;
pub type PGError = tokio_postgres::Error;
pub type PGStatement = tokio_postgres::Statement;

pub type PGRawClient = tokio_postgres::Client;
pub type PGRawTransaction<'a> = tokio_postgres::Transaction<'a>;
pub type PGClient = PGConnection<PGRawClient>;
pub type PGTransaction<'a> = PGConnection<PGRawTransaction<'a>>;

/// A shorthand used for the return types in the ToSql and FromSql implementations.
pub type PGConvertError = Box<dyn std::error::Error + Sync + Send>;

#[derive(ThisError, Debug)]
pub enum PGCreatePoolError {
    #[error(transparent)]
    PgError(#[from] PGError),
    #[error("Certificate load error")]
    CertError(#[source] CertError),
}

pub async fn create_postgres_pool(cns: &str) -> Result<PGConnectionPool, PGCreatePoolError> {
    let certs = get_root_cert_store().map_err(PGCreatePoolError::CertError)?;
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(certs)
        .with_no_client_auth();
    let tls = MakeRustlsConnect::new(tls_config);

    let pg_config = PGConfig::from_str(cns)?;
    log::debug!("Postgresql config: {pg_config:#?}");
    let postgres_manager = PGConnectionManager::new(pg_config, tls);
    let postgres = bb8::Pool::builder()
        .max_size(10) // Set the maximum number of connections in the pool
        .build(postgres_manager)
        .await?;

    Ok(postgres)
}
