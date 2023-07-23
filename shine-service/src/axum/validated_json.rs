use crate::axum::Problem;
use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest},
    http::Request,
    Json, RequestExt,
};
use validator::Validate;

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
        let Json(data) = req
            .extract::<Json<J>, _>()
            .await
            .map_err(|err| Problem::bad_request().with_detail(format!("{err:?}")))?;
        data.validate().map_err(|err| Problem::bad_request().with_object_detail(&err))?;
        Ok(Self(data))
    }
}
