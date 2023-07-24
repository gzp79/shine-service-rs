use crate::axum::Problem;
use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, FromRequestParts, Path, Query},
    http::{request::Parts, Request},
    Json, RequestExt,
};
use serde::de::DeserializeOwned;
use validator::Validate;

pub struct ValidatedPath<T>(pub T);

#[async_trait]
impl<S, T> FromRequestParts<S> for ValidatedPath<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send + Validate,
{
    type Rejection = Problem;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(data) = Path::<T>::from_request_parts(parts, state).await.map_err(|err| {
            Problem::bad_request()
                .with_type("path_format_error")
                .with_detail(format!("{err}"))
        })?;
        data.validate().map_err(|err| {
            Problem::bad_request()
                .with_type("validation_error")
                .with_object_detail(&err)
        })?;
        Ok(Self(data))
    }
}

pub struct ValidatedQuery<T>(pub T);

#[async_trait]
impl<S, T> FromRequestParts<S> for ValidatedQuery<T>
where
    S: Send + Sync,
    T: 'static + DeserializeOwned + Validate,
{
    type Rejection = Problem;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(data) = Query::<T>::from_request_parts(parts, state).await.map_err(|err| {
            Problem::bad_request()
                .with_type("query_format_error")
                .with_detail(format!("{err}"))
        })?;
        data.validate().map_err(|err| {
            Problem::bad_request()
                .with_type("validation_error")
                .with_object_detail(&err)
        })?;
        Ok(Self(data))
    }
}

pub struct ValidatedJson<J>(pub J);

#[async_trait]
impl<S, B, J> FromRequest<S, B> for ValidatedJson<J>
where
    B: Send + 'static,
    S: Send + Sync,
    J: Validate + 'static,
    Json<J>: FromRequest<(), B, Rejection = JsonRejection>,
{
    type Rejection = Problem;

    async fn from_request(req: Request<B>, _state: &S) -> Result<Self, Self::Rejection> {
        let Json(data) = req.extract::<Json<J>, _>().await.map_err(|err| {
            Problem::bad_request()
                .with_type("request_format_error")
                .with_detail(format!("{err}"))
        })?;
        data.validate().map_err(|err| {
            Problem::bad_request()
                .with_type("validation_error")
                .with_object_detail(&err)
        })?;
        Ok(Self(data))
    }
}
