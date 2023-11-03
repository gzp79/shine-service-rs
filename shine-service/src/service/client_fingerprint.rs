use axum::{
    async_trait, extract::FromRequestParts, headers::UserAgent, http::request::Parts, RequestPartsExt, TypedHeader,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use ring::digest::{self, Context};
use std::convert::Infallible;

#[derive(Debug, PartialEq, Eq)]
/// Some fingerprinting of the client site to detect token stealing.
pub struct ClientFingerprint(String);

impl ClientFingerprint {
    pub fn from_agent(agent: String) -> Self {
        let mut context = Context::new(&digest::SHA256);
        context.update(agent.as_bytes());
        let hash = B64.encode(context.finish().as_ref());
        Self(hash)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn to_string(&self) -> String {
        self.0.clone()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ClientFingerprint
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let agent = parts
            .extract::<TypedHeader<UserAgent>>()
            .await
            .map(|u| u.to_string())
            .unwrap_or_default();

        Ok(ClientFingerprint::from_agent(agent))
    }
}
