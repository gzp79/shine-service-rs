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

                rows.into_iter().map(|row| row.try_get(0)).collect::<Result<Vec<_>,_>>()
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
                let $rid: $rty = row.try_get(0)?;
                Ok($rid)
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
                match client.query_opt(&statement, &[$($pid,)*]).await?
                {
                    None => Ok(None),
                    Some(row) => Ok(Some(row.try_get(0)?)),
                }
            }
        }
    };

    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        out = ($($rid:ident: $rty:ty),*);
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);

        impl $id {
            #[allow(clippy::too_many_arguments)]
            pub async fn query<'a, T>(
                &self,
                client: &$crate::service::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Vec<($($rty,)*)>, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let rows = client.query(&statement, &[$($pid,)*]).await?;

                rows.into_iter().map(|row| {
                    let mut __id = 0;
                    $(
                        let $rid: $rty = match row.try_get(__id) {
                            Ok(v) => v,
                            Err(err) => return Err(err),
                        };
                        __id += 1;
                    )*
                    Ok(($($rid,)*))
                }).collect::<Result<Vec<_>,_>>()
            }

            #[allow(clippy::too_many_arguments)]
            pub async fn query_one<'a, T>(
                &self,
                client: &$crate::service::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<($($rty,)*), $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let row = client.query_one(&statement, &[$($pid,)*]).await?;
                let mut __id = 0;
                $(let $rid: $rty = row.try_get(__id)?; __id += 1;)*
                Ok(($($rid,)*))
            }

            #[allow(clippy::too_many_arguments)]
            pub async fn query_opt<'a, T>(
                &self,
                client: &$crate::service::PGConnection<T>,
                $($pid: &$pty,)*
            ) -> Result<Option<($($rty,)*)>, $crate::service::PGError>
            where
                T: $crate::service::PGRawConnection
            {
                let statement = self.statement(client).await?;
                let row = client.query_opt(&statement, &[$($pid,)*]).await?;
                match client.query_opt(&statement, &[$($pid,)*]).await?
                {
                    None => Ok(None),
                    Some(row) => {
                        let mut __id = 0;
                        $(let $rid: $rty = row.try_get(__id)?; __id += 1;)*
                        Ok(Some(($($rid,)*)))
                    }
                }
           }
        }
    };

    ($id:ident =>
        in = $($pid:ident: $pty:ty),*;
        out = $oty:ident{$($rid:ident: $rty:ty),*};
        sql = $stmt:expr ) => {

        $crate::pg_prepared_statement!($id => $stmt, [$($pid:$pty),*]);

        struct $oty {
            $(pub $rid: $rty),*
        }

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

                rows.into_iter().map(|row| {
                    let mut __id = 0;
                    $(
                        let $rid: $rty = match row.try_get(__id) {
                            Ok(v) => v,
                            Err(err) => return Err(err),
                        };
                        __id += 1;
                    )*
                    Ok($oty{$($rid,)*})
                }).collect::<Result<Vec<_>,_>>()
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
                let row = client.query_one(&statement, &[$($pid,)*]).await?;
                let mut __id = 0;
                $(let $rid: $rty = row.try_get(__id)?; __id += 1;)*
                Ok($oty{$($rid,)*})
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
                let row = client.query_opt(&statement, &[$($pid,)*]).await?;
                match client.query_opt(&statement, &[$($pid,)*]).await?
                {
                    None => Ok(None),
                    Some(row) => {
                        let mut __id = 0;
                        $(let $rid: $rty = row.try_get(__id)?; __id += 1;)*
                        Ok(Some($oty{$($rid,)*}))
                    }
                }
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
