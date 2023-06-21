mod query_builder;
pub use self::query_builder::*;
mod error_check;
pub use self::error_check::*;

use crate::service::cacerts;
use bb8::{ManageConnection, Pool as BB8Pool, RunError};
use bb8_postgres::PostgresConnectionManager;
use std::str::FromStr;
use tokio_postgres::Config as PGConfig;
use tokio_postgres_rustls::MakeRustlsConnect;

pub type PGConnection = PostgresConnectionManager<MakeRustlsConnect>;
pub type PGConnectionError = RunError<<PGConnection as ManageConnection>::Error>;
pub type PGConnectionPool = BB8Pool<PGConnection>;
pub type PGError = tokio_postgres::Error;

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
    let postgres_manager = PostgresConnectionManager::new(pg_config, tls);
    let postgres = bb8::Pool::builder()
        .max_size(10) // Set the maximum number of connections in the pool
        .build(postgres_manager)
        .await?;

    Ok(postgres)
}

/// Helper to create prepared SQL statements
#[macro_export]
macro_rules! pg_prepared_statement {
    ($id:ident => $stmt:expr, [$($ty:ident),*]) => {
        struct $id(tokio_postgres::Statement);

        impl $id {
            async fn new(client: &bb8::PooledConnection<'_, $crate::service::PGConnection>) -> Result<Self, $crate::service::PGError> {
                let stmt = client
                    .prepare_typed($stmt, &[$(tokio_postgres::types::Type::$ty,)*])
                    .await?;
                Ok(Self(stmt))
            }
        }

        impl std::ops::Deref for $id {
            type Target = tokio_postgres::Statement;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}
