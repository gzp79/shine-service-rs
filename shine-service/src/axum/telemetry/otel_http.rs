use axum::{
    extract::MatchedPath,
    http::{header, HeaderMap, Method, Request, Response, Uri, Version},
};
use opentelemetry::{propagation::Extractor, Context};
use std::{borrow::Cow, error::Error as StdError};
use tracing::field::Empty;

pub const TRACING_TARGET: &str = "otel::tracing";

#[inline]
#[must_use]
pub fn http_method(method: &Method) -> Cow<'static, str> {
    match method {
        &Method::CONNECT => "CONNECT".into(),
        &Method::DELETE => "DELETE".into(),
        &Method::GET => "GET".into(),
        &Method::HEAD => "HEAD".into(),
        &Method::OPTIONS => "OPTIONS".into(),
        &Method::PATCH => "PATCH".into(),
        &Method::POST => "POST".into(),
        &Method::PUT => "PUT".into(),
        &Method::TRACE => "TRACE".into(),
        other => other.to_string().into(),
    }
}

#[inline]
#[must_use]
pub fn http_flavor(version: Version) -> Cow<'static, str> {
    match version {
        Version::HTTP_09 => "0.9".into(),
        Version::HTTP_10 => "1.0".into(),
        Version::HTTP_11 => "1.1".into(),
        Version::HTTP_2 => "2.0".into(),
        Version::HTTP_3 => "3.0".into(),
        other => format!("{other:?}").into(),
    }
}

#[inline]
pub fn url_scheme(uri: &Uri) -> &str {
    uri.scheme_str().unwrap_or_default()
}

#[inline]
pub fn user_agent<B>(req: &Request<B>) -> &str {
    req.headers()
        .get(header::USER_AGENT)
        .map_or("", |h| h.to_str().unwrap_or(""))
}

#[inline]
pub fn http_host<B>(req: &Request<B>) -> &str {
    req.headers()
        .get(header::HOST)
        .map_or(req.uri().host(), |h| h.to_str().ok())
        .unwrap_or("")
}

#[must_use]
pub fn extract_context(headers: &HeaderMap) -> Context {
    pub struct HeaderExtractor<'a>(pub &'a HeaderMap);

    impl<'a> Extractor for HeaderExtractor<'a> {
        /// Get a value for a key from the HeaderMap.  If the value is not valid ASCII, returns None.
        fn get(&self, key: &str) -> Option<&str> {
            self.0.get(key).and_then(|value| value.to_str().ok())
        }

        /// Collect all the keys from the HeaderMap.
        fn keys(&self) -> Vec<&str> {
            self.0.keys().map(|value| value.as_str()).collect::<Vec<_>>()
        }
    }

    let extractor = HeaderExtractor(headers);
    opentelemetry::global::get_text_map_propagator(|propagator| propagator.extract(&extractor))
}

pub fn make_span_from_request<B>(req: &Request<B>) -> tracing::Span {
    // [opentelemetry-specification/.../http.md](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/semantic_conventions/http.md)
    // [opentelemetry-specification/.../span-general.md](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/semantic_conventions/span-general.md)
    // Can not use const or opentelemetry_semantic_conventions::trace::* for name of records
    let http_method = http_method(req.method());
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "", |mp| mp.as_str());
    let name = format!("[{http_method}] {route}");
    let name = name.trim();

    tracing::trace_span!(
        target: TRACING_TARGET,
        "HTTP request",
        http.request.method = %http_method,
        http.route = %route,
        http.client.address = Empty,
        http.response.status_code = Empty, // set on response
        network.protocol.version = %http_flavor(req.version()),
        server.address = http_host(req),
        user_agent.original = user_agent(req),
        url.path = req.uri().path(),
        url.query = req.uri().query(),
        url.scheme = url_scheme(req.uri()),
        otel.name = %name,
        otel.kind = ?opentelemetry::trace::SpanKind::Server,
        otel.status_code = Empty, // set on response
        trace_id = Empty, // set on response
        //request_id = Empty, // set
        exception.message = Empty, // set on response
        "span.type" = "web", // non-official open-telemetry key, only supported by Datadog
    )
}

pub fn update_span_from_response<B>(span: &tracing::Span, response: &Response<B>) {
    let status = response.status();
    span.record("http.response.status_code", status.as_u16());

    if status.is_server_error() {
        span.record("otel.status_code", "ERROR");
        // see[](https://github.com/open-telemetry/opentelemetry-specification/blob/v1.22.0/specification/trace/semantic_conventions/http.md#status)
        // Span Status MUST be left unset if HTTP status code was in the 1xx, 2xx or 3xx ranges,
        // unless there was another error (e.g., network error receiving the response body;
        // or 3xx codes with max redirects exceeded), in which case status MUST be set to Error.
        // } else {
        //     span.record("otel.status_code", "OK");
    }
}

pub fn update_span_from_error<E>(span: &tracing::Span, error: &E)
where
    E: StdError,
{
    span.record("otel.status_code", "ERROR");
    //span.record("http.status_code", 500);
    span.record("exception.message", error.to_string());
    error.source().map(|s| span.record("exception.message", s.to_string()));
}

pub fn update_span_from_response_or_error<B, E>(span: &tracing::Span, response: &Result<Response<B>, E>)
where
    E: StdError,
{
    match response {
        Ok(response) => {
            update_span_from_response(span, response);
        }
        Err(err) => {
            update_span_from_error(span, err);
        }
    }
}
