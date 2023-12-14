use crate::axum::telemetry::otel_http;
use axum::http::{Method, Request, Response};
use futures::ready;
use pin_project::pin_project;
use std::{
    error::Error as StdError,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer, Service};
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;

/// Filter for request path
pub type RequestFilter = fn(&Method, &str) -> bool;

/// Layer/middleware for axum to create spans from requests.
#[derive(Default, Debug, Clone)]
pub struct OtelLayer {
    request_filter: Option<RequestFilter>,
}

// add a builder like api
impl OtelLayer {
    #[must_use]
    pub fn filter(self, filter: RequestFilter) -> Self {
        OtelLayer {
            request_filter: Some(filter),
        }
    }
}

impl<S> Layer<S> for OtelLayer {
    type Service = OtelService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OtelService {
            inner,
            request_filter: self.request_filter,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OtelService<S> {
    inner: S,
    request_filter: Option<RequestFilter>,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for OtelService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    S::Error: StdError + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let req = req;
        let span = if self.request_filter.map_or(true, |f| f(req.method(), req.uri().path())) {
            let span = otel_http::make_span_from_request(&req);
            span.set_parent(otel_http::extract_context(req.headers()));
            span
        } else {
            tracing::Span::none()
        };
        let future = {
            let _ = span.enter();
            self.inner.call(req)
        };
        ResponseFuture { inner: future, span }
    }
}

#[pin_project]
pub struct ResponseFuture<F> {
    #[pin]
    pub(crate) inner: F,
    pub(crate) span: Span,
    // pub(crate) start: Instant,
}

impl<Fut, B, E> Future for ResponseFuture<Fut>
where
    Fut: Future<Output = Result<Response<B>, E>>,
    E: std::error::Error + 'static,
{
    type Output = Result<Response<B>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = this.span.enter();
        let result = ready!(this.inner.poll(cx));
        otel_http::update_span_from_response_or_error(this.span, &result);
        Poll::Ready(result)
    }
}
