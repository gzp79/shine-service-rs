use crate::axum::Problem;
use axum::{
    async_trait,
    extract::{
        rejection::{JsonRejection, PathRejection, QueryRejection},
        FromRequest, FromRequestParts, Path, Query,
    },
    http::{request::Parts, Request},
    response::{IntoResponse, Response},
    Json, RequestExt,
};
use serde::de::DeserializeOwned;
use thiserror::Error as ThisError;
use validator::{Validate, ValidationErrors};

#[derive(Debug, ThisError)]
pub enum ValidationError {
    #[error("Path could not be parsed for input")]
    PathFormat(PathRejection),
    #[error("Query could not be parsed for input")]
    QueryFormat(QueryRejection),
    #[error("Body could not be parsed for input")]
    JsonFormat(JsonRejection),
    #[error("Input constraint violated")]
    Constraint(ValidationErrors),
}

impl ValidationError {
    fn into_problem(self) -> Problem {
        match self {
            ValidationError::PathFormat(err) => Problem::bad_request()
                .with_type("path_format_error")
                .with_detail(format!("{err}")),
            ValidationError::QueryFormat(err) => Problem::bad_request()
                .with_type("query_format_error")
                .with_detail(format!("{err}")),
            ValidationError::JsonFormat(err) => Problem::bad_request()
                .with_type("request_format_error")
                .with_detail(format!("{err}")),
            ValidationError::Constraint(detail) => Problem::bad_request()
                .with_type("validation_error")
                .with_object_detail(&detail),
        }
    }
}

impl IntoResponse for ValidationError {
    fn into_response(self) -> Response {
        self.into_problem().into_response()
    }
}

pub struct ValidatedPath<T>(pub T)
where
    T: 'static + DeserializeOwned + Validate;

#[async_trait]
impl<S, T> FromRequestParts<S> for ValidatedPath<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send + Validate,
{
    type Rejection = ValidationError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(data) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(ValidationError::PathFormat)?;
        data.validate().map_err(ValidationError::Constraint)?;
        Ok(Self(data))
    }
}

pub struct ValidatedQuery<T>(pub T)
where
    T: 'static + DeserializeOwned + Validate;

#[async_trait]
impl<S, T> FromRequestParts<S> for ValidatedQuery<T>
where
    S: Send + Sync,
    T: 'static + DeserializeOwned + Validate,
{
    type Rejection = ValidationError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(data) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(ValidationError::QueryFormat)?;
        data.validate().map_err(ValidationError::Constraint)?;
        Ok(Self(data))
    }
}

pub struct ValidatedJson<J>(pub J)
where
    J: Validate + 'static;

#[async_trait]
impl<S, B, J> FromRequest<S, B> for ValidatedJson<J>
where
    B: Send + 'static,
    S: Send + Sync,
    J: Validate + 'static,
    Json<J>: FromRequest<(), B, Rejection = JsonRejection>,
{
    type Rejection = ValidationError;

    async fn from_request(req: Request<B>, _state: &S) -> Result<Self, Self::Rejection> {
        let Json(data) = req.extract::<Json<J>, _>().await.map_err(ValidationError::JsonFormat)?;
        data.validate().map_err(ValidationError::Constraint)?;
        Ok(Self(data))
    }
}
