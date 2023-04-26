use sqlx::{any::AnyKind, error::Error as SqlxError};

use crate::QueryBuilder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DBKind {
    Postgres,
    Sqlite,
}

impl From<AnyKind> for DBKind {
    fn from(kind: AnyKind) -> Self {
        match kind {
            AnyKind::Postgres => Self::Postgres,
            AnyKind::Sqlite => Self::Sqlite,
        }
    }
}

impl DBKind {
    pub fn query_builder<'a>(self) -> QueryBuilder<'a> {
        QueryBuilder::new(self)
    }

    pub fn is_constraint_err(&self, err: &SqlxError, constraint: &str) -> bool {
        match err {
            SqlxError::Database(err) => match self {
                DBKind::Postgres => err.constraint().unwrap_or_default() == constraint,
                DBKind::Sqlite => err.code().as_deref().unwrap_or_default() == "2067",
            },
            _ => false,
        }
    }
}
