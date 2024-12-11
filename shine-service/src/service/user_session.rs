use crate::{
    axum::{ConfiguredProblem, IntoProblem, Problem, ProblemConfig},
    service::{
        serde_session_key, ClientFingerprint, ClientFingerprintError, RedisConnectionError, RedisConnectionPool,
        SessionKey,
    },
};
use axum::{async_trait, extract::FromRequestParts, http::request::Parts, Extension, RequestPartsExt};
use axum_extra::extract::{cookie::Key, SignedCookieJar};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use ring::digest;
use serde::{Deserialize, Serialize};
use shine_macros::RedisJsonValue;
use std::{ops, sync::Arc};
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum UserSessionError {
    #[error("Missing session info")]
    Unauthenticated,
    #[error("Invalid session secret")]
    InvalidSecret(String),
    #[error("Session expired")]
    SessionExpired,
    #[error("Fingerprint error")]
    ClientFingerprintError(#[from] ClientFingerprintError),
    #[error("Session is compromised")]
    SessionCompromised,
    #[error("Failed to get redis connection")]
    RedisPoolError(#[source] RedisConnectionError),
    #[error("Redis error")]
    RedisError(#[from] redis::RedisError),
}

impl IntoProblem for UserSessionError {
    fn into_problem(self, config: &ProblemConfig) -> Problem {
        match self {
            UserSessionError::RedisPoolError(err) => Problem::internal_error(config, "Redis connection error", err),
            UserSessionError::RedisError(err) => Problem::internal_error(config, "Redis error", err),
            _ => Problem::unauthorized()
                .with_detail(self.to_string())
                .with_extension(config, format!("{:#?}", self)),
        }
    }
}

/// Current user accessible as an Extractor from the handlers and also the
/// stored data in the session cookie
#[derive(Clone, Debug, Hash, Serialize, Deserialize, RedisJsonValue)]
pub struct CurrentUser {
    #[serde(rename = "u")]
    pub user_id: Uuid,
    #[serde(rename = "key", with = "serde_session_key")]
    pub key: SessionKey,
    #[serde(rename = "sd")]
    pub session_start: DateTime<Utc>,
    #[serde(rename = "nm")]
    pub name: String,
    #[serde(rename = "r")]
    pub roles: Vec<String>,
    #[serde(rename = "fp")]
    pub fingerprint: String,
    #[serde(rename = "v")]
    pub version: i32,
}

pub struct CheckedCurrentUser(CurrentUser);

impl CheckedCurrentUser {
    pub fn into_user(self) -> CurrentUser {
        self.0
    }
}

impl From<CheckedCurrentUser> for CurrentUser {
    fn from(value: CheckedCurrentUser) -> Self {
        value.0
    }
}

impl ops::Deref for CheckedCurrentUser {
    type Target = CurrentUser;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl ops::DerefMut for CheckedCurrentUser {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for CheckedCurrentUser
where
    S: Send + Sync,
{
    type Rejection = ConfiguredProblem<UserSessionError>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(problem_config) = parts
            .extract::<Extension<ProblemConfig>>()
            .await
            .expect("Missing ProblemConfig extension");
        let Extension(validator) = parts
            .extract::<Extension<Arc<UserSessionCacheReader>>>()
            .await
            .expect("Missing UserSessionCacheReader extension");

        let unchecked = parts.extract::<UncheckedCurrentUser>().await?;
        let mut user = unchecked.0;
        validator
            .refresh_user(&mut user)
            .await
            .map_err(|err| problem_config.configure(err))?;
        Ok(CheckedCurrentUser(user))
    }
}

pub struct UncheckedCurrentUser(CurrentUser);

impl UncheckedCurrentUser {
    pub fn into_user(self) -> CurrentUser {
        self.0
    }
}

impl From<UncheckedCurrentUser> for CurrentUser {
    fn from(value: UncheckedCurrentUser) -> Self {
        value.0
    }
}

impl ops::Deref for UncheckedCurrentUser {
    type Target = CurrentUser;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl ops::DerefMut for UncheckedCurrentUser {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for UncheckedCurrentUser
where
    S: Send + Sync,
{
    type Rejection = ConfiguredProblem<UserSessionError>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(problem_config) = parts
            .extract::<Extension<ProblemConfig>>()
            .await
            .expect("Missing ProblemConfig extension");
        let Extension(validator) = parts
            .extract::<Extension<Arc<UserSessionCacheReader>>>()
            .await
            .expect("Missing UserSessionCacheReader extension");

        let fingerprint = parts
            .extract::<ClientFingerprint>()
            .await
            .map_err(|err| problem_config.configure(UserSessionError::from(err.problem)))?;

        let jar = SignedCookieJar::from_headers(&parts.headers, validator.cookie_secret.clone());
        let user = jar
            .get(&validator.cookie_name)
            .and_then(|cookie| serde_json::from_str::<CurrentUser>(cookie.value()).ok())
            .ok_or_else(|| problem_config.configure(UserSessionError::Unauthenticated))?;

        // perform the least minimal validation
        if user.fingerprint != fingerprint.as_str() {
            Err(problem_config.configure(UserSessionError::SessionCompromised))
        } else {
            Ok(UncheckedCurrentUser(user))
        }
    }
}

/// Handle the user data query in the redis cache.
pub struct UserSessionCacheReader {
    cookie_name: String,
    cookie_secret: Key,
    key_prefix: String,
    redis: RedisConnectionPool,
}

impl UserSessionCacheReader {
    pub fn new(
        name_suffix: Option<&str>,
        cookie_secret: &str,
        key_prefix: &str,
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
            key_prefix: key_prefix.to_string(),
            redis,
        })
    }

    pub fn into_layer(self) -> Extension<Arc<Self>> {
        Extension(Arc::new(self))
    }

    /// Refresh the session data in the cache. It should be in sync with the identity service
    /// and introduce any breaking change with great care as that can break authentication in all the service.
    async fn refresh_user(&self, user: &mut CurrentUser) -> Result<(), UserSessionError> {
        #[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
        #[serde(rename_all = "camelCase")]
        struct SessionSentinel {
            pub created_at: DateTime<Utc>,
            pub fingerprint: String,
        }

        #[derive(Serialize, Deserialize, Debug, RedisJsonValue)]
        #[serde(rename_all = "camelCase")]
        struct SessionData {
            pub name: String,
            pub is_email_confirmed: bool,
            pub roles: Vec<String>,
        }

        let (sentinel_key, key) = {
            let key_hash = digest::digest(&digest::SHA256, user.key.as_bytes());
            let key_hash = hex::encode(key_hash);

            let prefix = format!("{}session:{}:{}", self.key_prefix, user.user_id.as_simple(), key_hash);
            let sentinel_key = format!("{prefix}:openness");
            let key = format!("{prefix}:data");
            (sentinel_key, key)
        };

        let mut client = self.redis.get().await.map_err(UserSessionError::RedisPoolError)?;

        // query sentinel and the available data versions
        let (sentinel, data_versions): (Option<SessionSentinel>, Vec<i32>) = redis::pipe()
            .get(sentinel_key)
            .hkeys(&key)
            .query_async(&mut *client)
            .await
            .map_err(UserSessionError::RedisError)?;

        // check if sentinel is present
        let sentinel = match sentinel {
            Some(sentinel) => sentinel,
            _ => return Err(UserSessionError::SessionExpired),
        };

        // find the latest data version
        let version = match data_versions.into_iter().max() {
            Some(version) => version,
            _ => return Err(UserSessionError::SessionExpired),
        };

        // find data. In a very unlikely case data could have been just deleted.
        let data: SessionData = match client
            .hget(&key, format!("{version}"))
            .await
            .map_err(UserSessionError::RedisError)?
        {
            Some(data) => data,
            _ => return Err(UserSessionError::SessionExpired),
        };

        // check the fingerprint and other validations
        if user.fingerprint != sentinel.fingerprint
            || user.version > version
            || user.session_start != sentinel.created_at
        {
            return Err(UserSessionError::SessionCompromised);
        }

        user.name = data.name;
        user.roles = data.roles;
        user.version = version;
        Ok(())
    }
}
