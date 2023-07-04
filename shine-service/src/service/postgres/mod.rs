mod query_builder;
pub use self::query_builder::*;
mod error_check;
pub use self::error_check::*;
mod pg_connection;
pub use self::pg_connection::*;

/// Helper to create prepared SQL statements
#[macro_export]
macro_rules! pg_prepared_statement {
    ($id:ident => $stmt:expr, [$($ty:ident),*]) => {
        struct $id($crate::service::PGStatementId);

        impl $id {
            async fn create_statement(client: &bb8::PooledConnection<'_, $crate::service::PGConnectionManager>) -> Result<$crate::service::PGStatement, $crate::service::PGError> {
                log::debug!("creating prepared statement: \"{:#}\"", $stmt);
                let stmt = client
                    .prepare_typed($stmt, &[$(tokio_postgres::types::Type::$ty,)*])
                    .await?;
                Ok(stmt)
            }

            pub async fn new(client: &bb8::PooledConnection<'_, $crate::service::PGConnectionManager>) -> Result<Self, $crate::service::PGError> {
                let stmt = Self::create_statement(client).await?;
                Ok(Self(client.create_statement(stmt).await))
            }

            pub async fn get(&self, client: &bb8::PooledConnection<'_, $crate::service::PGConnectionManager>) -> Result<$crate::service::PGStatement, $crate::service::PGError> {
                if let Some(stmt) = client.get_statement(self.0).await {
                    Ok(stmt)
                } else {
                    let stmt = Self::create_statement(client).await?;
                    client.set_statement(self.0, stmt.clone()).await;
                    Ok(stmt)
                }
            }
        }
    };
}
