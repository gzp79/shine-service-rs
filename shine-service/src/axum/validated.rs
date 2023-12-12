use std::borrow::Cow;

use crate::{axum::Problem, utils::serde_string};
use axum::{
    async_trait,
    extract::{
        rejection::{JsonRejection, PathRejection, QueryRejection},
        FromRequest, FromRequestParts, Path, Query, Request,
    },
    http::request::Parts,
    response::{IntoResponse, Response},
    Json, RequestExt,
};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error as ThisError;
use validator::{Validate, ValidationError, ValidationErrors};

pub trait ValidationErrorEx {
    fn with_message<N>(self, message: N) -> Self
    where
        Self: Sized,
        N: Into<Cow<'static, str>>;

    fn with_param<N, T>(self, name: N, val: &T) -> Self
    where
        Self: Sized,
        N: Into<Cow<'static, str>>,
        T: Serialize;

    fn into_constraint_error(self, field: &'static str) -> InputError
    where
        Self: Sized;

    fn into_constraint_problem(self, field: &'static str) -> Problem
    where
        Self: Sized,
    {
        self.into_constraint_error(field).into_problem()
    }
}

impl ValidationErrorEx for ValidationError {
    fn with_message<N>(self, message: N) -> Self
    where
        Self: Sized,
        N: Into<Cow<'static, str>>,
    {
        Self {
            message: Some(message.into()),
            ..self
        }
    }

    fn with_param<N, T>(mut self, name: N, val: &T) -> Self
    where
        Self: Sized,
        N: Into<Cow<'static, str>>,
        T: Serialize,
    {
        self.add_param(name.into(), val);
        self
    }

    fn into_constraint_error(self, field: &'static str) -> InputError
    where
        Self: Sized,
    {
        let mut error = ValidationErrors::new();
        error.add(field, self);
        InputError::Constraint(error)
    }
}

#[derive(Debug, ThisError, Serialize)]
pub enum InputError {
    #[error("Path could not be parsed for input")]
    #[serde(with = "serde_string")]
    PathFormat(PathRejection),
    #[error("Query could not be parsed for input")]
    #[serde(with = "serde_string")]
    QueryFormat(QueryRejection),
    #[error("Body could not be parsed for input")]
    #[serde(with = "serde_string")]
    JsonFormat(JsonRejection),
    #[error("Input constraint violated")]
    Constraint(ValidationErrors),
}

impl InputError {
    fn into_problem(self) -> Problem {
        match self {
            InputError::PathFormat(err) => Problem::bad_request()
                .with_type("path_format_error")
                .with_detail(format!("{err:?}")),
            InputError::QueryFormat(err) => Problem::bad_request()
                .with_type("query_format_error")
                .with_detail(format!("{err}")),
            InputError::JsonFormat(JsonRejection::JsonDataError(err)) => Problem::bad_request()
                .with_type("body_format_error")
                .with_detail(err.body_text()),
            InputError::JsonFormat(JsonRejection::JsonSyntaxError(err)) => Problem::bad_request()
                .with_type("body_format_error")
                .with_detail(err.body_text()),
            InputError::JsonFormat(err) => Problem::internal_error().with_detail(format!("{err}")),
            InputError::Constraint(detail) => Problem::bad_request()
                .with_type("validation_error")
                .with_object_detail(&detail),
        }
    }
}

impl IntoResponse for InputError {
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
    type Rejection = InputError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(data) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(InputError::PathFormat)?;
        data.validate().map_err(InputError::Constraint)?;
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
    type Rejection = InputError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(data) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(InputError::QueryFormat)?;
        data.validate().map_err(InputError::Constraint)?;
        Ok(Self(data))
    }
}

pub struct ValidatedJson<J>(pub J)
where
    J: Validate + 'static;

#[async_trait]
impl<S, J> FromRequest<S> for ValidatedJson<J>
where
    S: Send + Sync,
    J: Validate + 'static,
    Json<J>: FromRequest<(), Rejection = JsonRejection>,
{
    type Rejection = InputError;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let Json(data) = req.extract::<Json<J>, _>().await.map_err(InputError::JsonFormat)?;
        data.validate().map_err(InputError::Constraint)?;
        Ok(Self(data))
    }
}
