use std::{convert::Infallible, fmt, marker::PhantomData, ops, sync::Arc};

use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response},
    Extension, RequestPartsExt,
};
use axum_extra::extract::{
    cookie::{Cookie, Expiration, Key, SameSite},
    SignedCookieJar,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum SessionError {
    #[error("Invalid session secret: {0}")]
    InvalidSecret(String),
}

/// Layer to configure Session cookies
#[derive(Clone)]
pub struct SessionMeta<T> {
    pub cookie_name: String,
    key: Key,
    _ph: PhantomData<T>,
}

impl<T> SessionMeta<T> {
    pub fn new(b64_key: &str) -> Result<Self, SessionError> {
        let key = B64
            .decode(b64_key)
            .map_err(|err| SessionError::InvalidSecret(format!("{err}")))?;
        let key = Key::try_from(&key[..]).map_err(|err| SessionError::InvalidSecret(format!("{err}")))?;
        Ok(Self {
            cookie_name: "sid".into(),
            key,
            _ph: PhantomData,
        })
    }

    pub fn with_cookie_name<S: ToString>(self, cookie_name: S) -> Self {
        Self {
            cookie_name: cookie_name.to_string(),
            ..self
        }
    }

    pub fn into_layer(self) -> Extension<Arc<Self>> {
        Extension(Arc::new(self))
    }
}

/// Extractor to get and set session cookie. Before use, it requires to add a SessionMeta layer with the appropriate T type to the `Router`.
pub struct Session<T> {
    meta: Arc<SessionMeta<T>>,
    data: Option<T>,
}

impl<T> Session<T> {
    pub fn set(&mut self, data: T) {
        self.data = Some(data);
    }

    pub fn take(&mut self) -> Option<T> {
        self.data.take()
    }
}

impl<T> ops::Deref for Session<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> ops::DerefMut for Session<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T> fmt::Debug for Session<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session").field("data", &self.data).finish()
    }
}

#[async_trait]
impl<S, T> FromRequestParts<S> for Session<T>
where
    S: Send + Sync,
    T: 'static + DeserializeOwned + Clone + Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(meta) = parts
            .extract::<Extension<Arc<SessionMeta<T>>>>()
            .await
            .expect("Missing SessionMeta extension");

        let jar = SignedCookieJar::from_headers(&parts.headers, meta.key.clone());
        if let Some(session) = jar.get(&meta.cookie_name) {
            let data = serde_json::from_str::<T>(session.value()).ok();
            Ok(Session { meta, data })
        } else {
            Ok(Session { meta, data: None })
        }
    }
}

impl<T: Serialize> IntoResponse for Session<T> {
    fn into_response(self) -> Response {
        let Session { data: session, meta } = self;

        let mut jar = SignedCookieJar::new(meta.key.clone());

        if let Some(session) = session {
            let raw_data = serde_json::to_string(&session).expect("failed to serialize session data");
            let mut cookie = Cookie::new(meta.cookie_name.clone(), raw_data);
            cookie.set_secure(true);
            cookie.set_expires(Expiration::Session);
            cookie.set_same_site(SameSite::Lax);
            cookie.set_path("/");

            jar = jar.add(cookie)
        }

        jar.into_response()
    }
}
