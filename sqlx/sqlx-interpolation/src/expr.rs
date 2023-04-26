use crate::{DBKind, QueryBuilder, SqlBuilderExpression};
use chrono::Duration;

/// Represent an unescaped raw sql command snippet that is concatenated to the query.
/// Use with caution as command is not protected against sql injection vulnerability.
pub struct RawSql(pub String);

impl RawSql {
    pub fn new<S: ToString>(sql: S) -> RawSql {
        RawSql(sql.to_string())
    }
}

impl<'q> SqlBuilderExpression<'q> for RawSql {
    fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q> {
        query.sql(&self.0)
    }
}

pub struct Now;

impl<'q> SqlBuilderExpression<'q> for Now {
    fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q> {
        match query.kind() {
            DBKind::Postgres => query.add(RawSql::new(" now() ")),
            DBKind::Sqlite => query.add(RawSql::new(" datetime('now') ")),
        }
    }
}

pub struct NowShift(pub Duration);

impl<'q> SqlBuilderExpression<'q> for NowShift {
    fn add_to_query<'a>(self, query: &'a mut QueryBuilder<'q>) -> &'a mut QueryBuilder<'q> {
        let s = self.0.num_seconds();
        match query.kind() {
            DBKind::Postgres => {
                let sql = format!("now() + {} * interval '{} seconds'", s.signum(), s.abs());
                query.add(RawSql(sql))
            }
            DBKind::Sqlite => {
                let sql = format!("DATETIME(datetime('now'), \"{s} seconds\")");
                query.add(RawSql(sql))
            }
        }
    }
}
