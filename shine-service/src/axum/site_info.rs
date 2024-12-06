use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
    RequestPartsExt,
};
use axum_extra::{headers::UserAgent, TypedHeader};
use std::convert::Infallible;

#[derive(Clone, Debug, PartialEq, Eq)]
/// General client info used for human readable site identification
pub struct SiteInfo {
    pub agent: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[async_trait]
impl<S> FromRequestParts<S> for SiteInfo
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

        let headers = parts.extract::<HeaderMap>().await.unwrap_or_default();

        Ok(SiteInfo {
            agent,
            country: headers
                .get("cf-ipcountry")
                .map(|c| c.to_str().unwrap_or_default().to_string()),
            region: headers
                .get("cf-region")
                .map(|c| c.to_str().unwrap_or_default().to_string()),
            city: headers
                .get("cf-ipcity")
                .map(|c| c.to_str().unwrap_or_default().to_string()),
        })
    }
}
