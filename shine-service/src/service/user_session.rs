use crate::{
    axum::session::{Session, SessionMeta},
    service::{serde_session_key, SessionKey, RedisConnectionPool},
};
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::FromRequestParts,
    http::{request::Parts, Response, StatusCode},
    Extension, RequestPartsExt,
};
use std::sync::Arc;
//use axum::{http::{Request, Response}, middleware::Next};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_macros::RedisJsonValue;
use uuid::Uuid;

/// Current user accessible as an Extractor from the handlers and also the
/// stored data in the session cookie
#[derive(Clone, Debug, Hash, Serialize, Deserialize, RedisJsonValue)]
pub struct CurrentUser {
    /// Indicates if this information confirms to the UserSessionValidator configuration.
    #[serde(rename = "a")]
    pub is_authentic: bool,
    #[serde(rename = "id")]
    pub user_id: Uuid,
    #[serde(rename = "k", with = "serde_session_key")]
    pub key: SessionKey,
    #[serde(rename = "t")]
    pub session_start: DateTime<Utc>,
    #[serde(rename = "n")]
    pub name: String,
    //pub client_agent: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(validator) = parts
            .extract::<Extension<Arc<UserSessionValidator>>>()
            .await
            .expect("Missing UserSessionValidator extension");

        let user_session = parts
            .extract::<Session<CurrentUser>>()
            .await
            .expect("Missing SessionMeta extension")
            .take();
        log::info!("{:#?}", user_session);

        if let Some(mut user) = user_session {
            validator.validate(&mut user).await?;
            Ok(user)
        } else {
            let response = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap();
            Err(response)
        }
    }
}

pub type UserSessionMeta = SessionMeta<CurrentUser>;
pub type UserSession = Session<CurrentUser>;

/// Add extra validation to the user session. While sessions are signed, this
/// layer gets an up to date version from the identity service.
pub struct UserSessionValidator {
    redis: RedisConnectionPool,
}

impl UserSessionValidator {
    pub fn new(redis: RedisConnectionPool) -> Self {
        Self { redis }
    }
    pub fn into_layer(self) -> Extension<Arc<Self>> {
        Extension(Arc::new(self))
    }

    pub async fn validate(&self, user: &mut CurrentUser) -> Result<(), Response<Body>> {
        user.is_authentic = false;
        // todo: check the in memory lru for the (user_id,key)
        //  if not found check the redis cache
        Ok(())
    }
}
