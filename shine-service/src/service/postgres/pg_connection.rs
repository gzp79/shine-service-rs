use crate::service::cacerts;
use async_trait::async_trait;
use bb8::{ManageConnection, Pool as BB8Pool, RunError};
use bb8_postgres::PostgresConnectionManager;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{collections::HashMap, ops::DerefMut};
use tokio::sync::RwLock;
use tokio_postgres::{Client as PGClient, Config as PGConfig, Statement};
use tokio_postgres_rustls::MakeRustlsConnect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PGStatementId(usize);

/// A custom extension to the PGClient:
/// - add helpers to handles prepared statements as they belong to the connection and
///   hence they have to be created for each connection independently
pub struct PGConnection {
    client: PGClient,
    prepared_statements: RwLock<HashMap<usize, Statement>>,
    prepared_statement_id: Arc<AtomicUsize>,
}

impl PGConnection {
    fn new(pg_client: PGClient, prepared_statement_id: Arc<AtomicUsize>) -> Self {
        Self {
            client: pg_client,
            prepared_statement_id,
            prepared_statements: RwLock::new(HashMap::default()),
        }
    }

    pub async fn create_statement(&self, prepared: Statement) -> PGStatementId {
        let id = self.prepared_statement_id.fetch_add(1, Ordering::Relaxed);
        self.set_statement(PGStatementId(id), prepared).await;
        PGStatementId(id)
    }

    pub async fn get_statement(&self, prepared_id: PGStatementId) -> Option<Statement> {
        let prepared_statements = self.prepared_statements.read().await;
        prepared_statements.get(&prepared_id.0).cloned()
    }

    pub async fn set_statement(&self, prepared_id: PGStatementId, prepared: Statement) {
        let mut prepared_statements = self.prepared_statements.write().await;
        prepared_statements.insert(prepared_id.0, prepared);
    }
}

impl Deref for PGConnection {
    type Target = PGClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for PGConnection {
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
    type Connection = PGConnection;
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
pub type PGError = tokio_postgres::Error;
pub type PGStatement = tokio_postgres::Statement;

pub async fn create_postgres_pool(cns: &str) -> Result<PGConnectionPool, PGConnectionError> {
    //todo: make tls optional (from feature as tls is a property of the connection type, see NoTls).
    //      It can be disabled when running in cloud on a virtual network.
    let tls_config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(cacerts::get_root_cert_store())
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
