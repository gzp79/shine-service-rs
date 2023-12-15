use crate::axum::telemetry::otel_http;
use axum::{
    extract::MatchedPath,
    http::{Method, Request, Response},
};
use futures::ready;
use opentelemetry::{
    metrics::{Counter, Histogram, Meter},
    KeyValue,
};
use pin_project::pin_project;
use std::{
    error::Error as StdError,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};
use tower::{Layer, Service};
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;

/// Filter for request path
pub type RequestFilter = fn(&Method, &str) -> bool;

/// Layer/middleware for axum to create spans from requests.
#[derive(Default, Clone)]
pub struct OtelLayer {
    request_filter: Option<RequestFilter>,
    meter: Option<Meter>,
}

// add a builder like api
impl OtelLayer {
    #[must_use]
    pub fn filter(self, filter: RequestFilter) -> Self {
        OtelLayer {
            request_filter: Some(filter),
            ..self
        }
    }

    #[must_use]
    pub fn meter(self, meter: Meter) -> Self {
        OtelLayer {
            meter: Some(meter),
            ..self
        }
    }
}

impl<S> Layer<S> for OtelLayer {
    type Service = OtelService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        let meters = self.meter.as_ref().map(|meter| OtelMeters {
            request_counter: meter.u64_counter("request_count").init(),
            request_duration: meter.f64_histogram("request_duration").init(),
            error_counter: meter.u64_counter("error_count").init(),
        });

        OtelService {
            inner,
            request_filter: self.request_filter,
            meters,
        }
    }
}

#[derive(Clone)]
struct OtelContext {
    method: Method,
    route: String,
    start: Instant,
}

#[derive(Clone)]
struct OtelMeters {
    request_counter: Counter<u64>,
    request_duration: Histogram<f64>,
    error_counter: Counter<u64>,
}

#[derive(Clone)]
pub struct OtelService<S> {
    inner: S,
    request_filter: Option<RequestFilter>,
    meters: Option<OtelMeters>,
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

        let route = req
            .extensions()
            .get::<MatchedPath>()
            .map(|mp| mp.as_str().to_string())
            .unwrap_or_default();
        let context = OtelContext {
            method: req.method().to_owned(),
            route,
            start: Instant::now(),
        };

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
        ResponseFuture {
            inner: future,
            context,
            span,
            meters: self.meters.clone(),
        }
    }
}

#[pin_project]
pub struct ResponseFuture<F> {
    #[pin]
    inner: F,
    context: OtelContext,
    span: Span,
    meters: Option<OtelMeters>,
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

        if let Some(meters) = this.meters.as_ref() {
            let ep_attribute = [
                KeyValue::new("method", this.context.method.to_string()),
                KeyValue::new("route", this.context.route.clone()),
            ];

            if result.is_err() {
                meters.error_counter.add(1, &ep_attribute);
            }

            meters.request_counter.add(1, &ep_attribute);
            let duration = Instant::now().duration_since(this.context.start).as_secs_f64();
            meters.request_duration.record(duration, &ep_attribute);
        }

        otel_http::update_span_from_response_or_error(this.span, &result);
        Poll::Ready(result)
    }
}
