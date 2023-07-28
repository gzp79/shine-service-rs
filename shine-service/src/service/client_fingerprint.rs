use axum::{
    async_trait, extract::FromRequestParts, headers::UserAgent, http::request::Parts, RequestPartsExt, TypedHeader,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use ring::digest::{self, Context};
use std::convert::Infallible;

#[derive(Debug, PartialEq, Eq)]
/// Some fingerprinting of the client site to detect token stealing.
pub struct ClientFingerprint {
    pub agent: String,
}

impl ClientFingerprint {
    pub fn from_compact_string(compact: String) -> Self {
        Self { agent: compact }
    }

    pub fn to_compact_string(&self) -> String {
        self.agent.clone()
    }

    pub fn hash(&self) -> String {
        let mut context = Context::new(&digest::SHA256);
        context.update(self.agent.as_bytes());
        B64.encode(context.finish().as_ref())
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

        Ok(ClientFingerprint { agent })
    }
}
