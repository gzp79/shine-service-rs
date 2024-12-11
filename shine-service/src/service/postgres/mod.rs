mod query_builder;

pub use self::query_builder::*;
mod error_check;
pub use self::error_check::*;
mod pg_connection;
pub use self::pg_connection::*;
mod pg_type;
pub use self::pg_type::*;

/// Create a prepared SQL statements
#[macro_export]
macro_rules! pg_prepared_statement {
    ($id:ident => $stmt:expr, [$($pid:ident:$pty:ty),*]) => {

        #[derive(Clone, Copy, Debug)]
        struct $id($crate::service::PGStatementId);

        impl $id {
            async fn create_statement<T>(client: &$crate::service::PGConnection<T>) -> Result<$crate::service::PGStatement, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                log::debug!("creating prepared statement: \"{:#}\"", $stmt);
                client
                    .prepare_typed($stmt, &[$(<$pty as $crate::service::ToPGType>::PG_TYPE,)*])
                    .await
            }

            pub async fn new(client: &$crate::service::PGClient) -> Result<Self, $crate::service::PGError>
            {
                let stmt = Self::create_statement(&client).await?;
                Ok(Self(client.create_statement(stmt).await))
            }

            pub async fn statement<'a, T>(&self, client: &$crate::service::PGConnection<T>) -> Result<$crate::service::PGStatement, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                if let Some(stmt) = client.get_statement(self.0).await {
                    Ok(stmt)
                } else {
                    let stmt = Self::create_statement(&client).await?;
                    client.set_statement(self.0, stmt.clone()).await;
                    Ok(stmt)
                }
            }
        }
    }
}

/// Helper to create prepared SQL statements
#[macro_export]
macro_rules! pg_query {
    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        out = $rid:ident: $rty:ty;
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);

        impl $id {
            #[allow(clippy::too_many_arguments)]
            pub async fn query<'a, T>(
                &self,
                client: &$crate::service::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Vec<$rty>, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let rows = client.query(&statement, &[$($pid,)*]).await?;

                rows.into_iter().map(|row| row.try_get(&stringify!($rid))).collect::<Result<Vec<_>,_>>()
            }

            #[allow(clippy::too_many_arguments)]
            pub async fn query_one<'a, T>(
                &self,
                client: &$crate::service::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<$rty, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let row = client.query_one(&statement, &[$($pid,)*]).await?;
                let value: $rty = row.try_get(&stringify!($rid))?;
                Ok(value)
            }

            #[allow(clippy::too_many_arguments)]
            pub async fn query_opt<'a, T>(
                &self,
                client: &$crate::service::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Option<$rty>, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                let statement = self.statement(client).await?;
                client.query_opt(&statement, &[$($pid,)*])
                    .await?
                    .map(|r| r.try_get(&stringify!($rid)))
                    .transpose()
            }
        }
    };

    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        out = $oty:ty;
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);

        impl $id {
            #[allow(clippy::too_many_arguments)]
            pub async fn query<'a, T>(
                &self,
                client: &$crate::service::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Vec<$oty>, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let rows = client.query(&statement, &[$($pid,)*]).await?;

                rows.into_iter()
                    .map(|row| <$oty as postgres_from_row::FromRow>::try_from_row(&row))
                    .collect::<Result<Vec<_>,_>>()
            }

            #[allow(clippy::too_many_arguments)]
            pub async fn query_one<'a, T>(
                &self,
                client: &$crate::service::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<$oty, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let row = client
                    .query_one(&statement, &[$($pid,)*])
                    .await?;
                <$oty as postgres_from_row::FromRow>::try_from_row(&row)
            }

            #[allow(clippy::too_many_arguments)]
            pub async fn query_opt<'a, T>(
                &self,
                client: &$crate::service::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Option<$oty>, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                let statement = self.statement(client).await?;
                client.query_opt(&statement, &[$($pid,)*])
                    .await?
                    .map(|row| <$oty as postgres_from_row::FromRow>::try_from_row(&row) )
                    .transpose()
            }
        }
    };

    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);

        impl $id {
            #[allow(clippy::too_many_arguments)]
            pub async fn execute<'a, T>(
                &self,
                client: &$crate::service::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<u64, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                let statement = self.statement(client).await?;
                client.execute(&statement, &[$($pid,)*]).await
            }
        }
    };
}
