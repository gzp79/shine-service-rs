use tokio_postgres::error::SqlState;

pub trait PGErrorChecks {
    fn is_constraint(&self, table: &str, constraint: &str) -> bool;
}

impl PGErrorChecks for tokio_postgres::Error {
    fn is_constraint(&self, table: &str, constraint: &str) -> bool {
        if let Some(err) = self.as_db_error() {
            if &SqlState::UNIQUE_VIOLATION == err.code()
                && err.table() == Some(table)
                && err.message().contains(constraint)
            {
                return true;
            }

            if &SqlState::FOREIGN_KEY_VIOLATION == err.code()
                && err.table() == Some(table)
                && err.message().contains(constraint)
            {
                return true;
            }

            if &SqlState::CHECK_VIOLATION == err.code()
                && err.table() == Some(table)
                && err.message().contains(constraint)
            {
                return true;
            }
        }
        false
    }
}
