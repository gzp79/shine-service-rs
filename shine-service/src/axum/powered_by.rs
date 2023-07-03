use axum::{
    body::Body,
    http::{header::InvalidHeaderValue, HeaderValue, Request},
    response::Response,
};
use futures::future::BoxFuture;
use std::task::{Context, Poll};
use tower::{Layer, Service};

const POWERED_BY_HEADER: &str = "x-powered-by";

#[derive(Clone)]
pub struct PoweredBy {
    version: HeaderValue,
}

impl PoweredBy {
    pub fn new<S>(version: S) -> Result<Self, InvalidHeaderValue>
    where
        S: TryInto<HeaderValue, Error = InvalidHeaderValue>,
    {
        Ok(Self {
            version: version.try_into()?,
        })
    }

    pub fn from_service_info<S1: AsRef<str>, S2: AsRef<str>>(
        service: S1,
        version: S2,
    ) -> Result<Self, InvalidHeaderValue> {
        Ok(Self {
            version: HeaderValue::from_str(&format!("{}@{}", service.as_ref(), version.as_ref()))?,
        })
    }
}

impl<S> Layer<S> for PoweredBy {
    type Service = PoweredByMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PoweredByMiddleware {
            inner,
            layer: self.clone(),
        }
    }
}

#[derive(Clone)]
#[must_use]
pub struct PoweredByMiddleware<S> {
    inner: S,
    layer: PoweredBy,
}

impl<S> Service<Request<Body>> for PoweredByMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let layer = self.layer.clone();
        let future = self.inner.call(request);
        Box::pin(async move {
            let mut response: Response = future.await?;
            let headers = response.headers_mut();
            headers.append(POWERED_BY_HEADER, layer.version);
            Ok(response)
        })
    }
}
