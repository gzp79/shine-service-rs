use crate::{
    axum::session::{Session, SessionMeta},
    service::{serde_session_key, SessionKey},
};
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::FromRequestParts,
    http::{request::Parts, Response, StatusCode},
    RequestPartsExt,
};
//use axum::{http::{Request, Response}, middleware::Next};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current user accessible as an Extractor from the handlers.
#[derive(Clone, Debug)]
pub struct CurrentUser {
    pub user_id: Uuid,

    pub session_start: DateTime<Utc>,
    pub key: SessionKey,

    pub name: String,
    /* pub email: Option<String>
       for safety and GDPR the best if only identity knows about it and if required
       a different set of endpoints can be created to manage it.
    */
    pub is_email_confirmed: bool,
    //pub client_agent: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // todo: extract cookie
        let mut user_session = parts
            .extract::<Session<UserSessionData>>()
            .await
            .expect("Missing SessionMeta extension");
        log::info!("{:#?}", user_session);

        if let Some(user_session) = user_session.take() {
            //todo:: read details from redis
            Ok(CurrentUser {
                user_id: user_session.user_id,
                session_start: Utc::now(),
                key: user_session.key,
                name: "todo".into(),
                is_email_confirmed: false,
            })
        } else {
            let response = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap();
            Err(response)
        }
    }
}

/// Low level session handling. It is usually recommended to use CurrentUser instead of this structure.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UserSessionData {
    #[serde(rename = "id")]
    pub user_id: Uuid,
    #[serde(rename = "sid", with = "serde_session_key")]
    pub key: SessionKey,
}

pub type UserSessionMeta = SessionMeta<UserSessionData>;
pub type UserSession = Session<UserSessionData>;

/*
async fn user_session_middleware<B>(
    Extension<(auth): TypedHeader<Authorization<Bearer>>,
    request: Request<B>,
    next: Next<B>,
) -> Response {
    let response = next.run(request).await;

    response
}
*/
