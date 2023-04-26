use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum DBBuilderError {
    #[error("Query is not a raw string, has bound values")]
    QueryIsNotRaw,
}
