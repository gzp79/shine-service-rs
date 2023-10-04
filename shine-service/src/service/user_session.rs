use crate::service::{serde_session_key, RedisConnectionPool, SessionKey};
use axum::{
    async_trait,
    body::Body,
    extract::FromRequestParts,
    http::{request::Parts, Response, StatusCode},
    Extension, RequestPartsExt,
};
use axum_extra::extract::{cookie::Key, SignedCookieJar};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_macros::RedisJsonValue;
use std::sync::Arc;
use thiserror::Error as ThisError;
use uuid::Uuid;

use super::ClientFingerprint;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum CurrentUserAuthenticity {
    #[serde(rename = "a")]
    Authentic,
    #[serde(rename = "c")]
    Cached,
    #[serde(rename = "n")]
    NotValidate,
}

/// Current user accessible as an Extractor from the handlers and also the
/// stored data in the session cookie
#[derive(Clone, Debug, Hash, Serialize, Deserialize, RedisJsonValue)]
pub struct CurrentUser {
    /// Indicates if this information confirms to the UserSessionValidator.
    /// If validator is skipped (for example in AuthSession handler in the identity service), it defaults to false.
    #[serde(rename = "a")]
    pub authenticity: CurrentUserAuthenticity,
    #[serde(rename = "id")]
    pub user_id: Uuid,
    #[serde(rename = "k", with = "serde_session_key")]
    pub key: SessionKey,
    #[serde(rename = "t")]
    pub session_start: DateTime<Utc>,
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "r")]
    pub roles: Vec<String>,
    #[serde(rename = "fp")]
    pub fingerprint_hash: String,
    #[serde(rename = "v")]
    pub version: i32,
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

        let fingerprint = parts.extract::<ClientFingerprint>().await.unwrap();

        let jar = SignedCookieJar::from_headers(&parts.headers, validator.cookie_secret.clone());
        let user = jar
            .get(&validator.cookie_name)
            .and_then(|cookie| serde_json::from_str::<CurrentUser>(cookie.value()).ok());

        if let Some(mut user) = user {
            if validator.validate(&mut user, fingerprint).await? {
                return Ok(user);
            }
        }

        let response = Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::empty())
            .unwrap();
        Err(response)
    }
}

#[derive(Debug, ThisError)]
pub enum UserSessionError {
    #[error("Invalid session secret: {0}")]
    InvalidSecret(String),
}

/// Add extra validation to the user session. While sessions are signed, this
/// layer gets an up to date version from the identity service.
pub struct UserSessionValidator {
    cookie_name: String,
    cookie_secret: Key,
    redis: RedisConnectionPool,
}

impl UserSessionValidator {
    pub fn new(
        name_suffix: Option<&str>,
        cookie_secret: &str,
        redis: RedisConnectionPool,
    ) -> Result<Self, UserSessionError> {
        let name_suffix = name_suffix.unwrap_or_default();
        let cookie_secret = {
            let key = B64
                .decode(cookie_secret)
                .map_err(|err| UserSessionError::InvalidSecret(format!("{err}")))?;
            Key::try_from(&key[..]).map_err(|err| UserSessionError::InvalidSecret(format!("{err}")))?
        };

        Ok(Self {
            cookie_name: format!("sid{}", name_suffix),
            cookie_secret,
            redis,
        })
    }

    pub fn into_layer(self) -> Extension<Arc<Self>> {
        Extension(Arc::new(self))
    }

    pub async fn validate(
        &self,
        user: &mut CurrentUser,
        fingerprint: ClientFingerprint,
    ) -> Result<bool, Response<Body>> {
        user.authenticity = CurrentUserAuthenticity::Authentic;
        // issue#12:
        // check the in memory lru for the (user_id,key)
        // check the redis cache

        if user.fingerprint_hash != fingerprint.hash() {
            Ok(false)
        } else {
            Ok(true)
        }
    }
}
