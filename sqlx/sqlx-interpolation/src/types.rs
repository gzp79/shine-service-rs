use crate::{expr::RawSql, DBKind, QueryBuilder, SqlBuilderExpression};

/// Column type for unique, auto-incremented id.
pub struct EntityId;

impl<'q> SqlBuilderExpression<'q> for EntityId {
    fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q> {
        match query.kind() {
            DBKind::Postgres => query.add(RawSql::new(" SERIAL PRIMARY KEY ")),
            DBKind::Sqlite => query.add(RawSql::new(" INTEGER PRIMARY KEY AUTOINCREMENT ")),
        }
    }
}

pub struct BinaryBlob;

impl<'q> SqlBuilderExpression<'q> for BinaryBlob {
    fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q> {
        match query.kind() {
            DBKind::Postgres => query.add(RawSql::new(" BYTEA ")),
            DBKind::Sqlite => query.add(RawSql::new(" BLOB ")),
        }
    }
}
