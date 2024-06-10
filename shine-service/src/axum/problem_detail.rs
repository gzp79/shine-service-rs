use crate::utils::{serde_status_code, serde_uri};
use axum::{
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::Value as JsonValue;

#[derive(Clone)]
pub struct ProblemConfig {
    pub include_internal: bool,
}

#[derive(Debug, Serialize)]
pub struct Problem {
    #[serde(rename = "status", serialize_with = "serde_status_code::serialize")]
    status: StatusCode,
    #[serde(rename = "type")]
    ty: &'static str,
    #[serde(rename = "instance", serialize_with = "serde_uri::serialize_opt")]
    instance: Option<Uri>,
    #[serde(rename = "detail")]
    detail: JsonValue,
}

impl Problem {
    pub fn new(status: StatusCode, ty: &'static str) -> Self {
        Problem {
            status,
            ty,
            instance: None,
            detail: JsonValue::Null,
        }
    }

    pub fn bad_request(ty: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ty)
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized")
    }

    pub fn internal_error() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "server_error")
    }

    pub fn with_instance<I: Into<Uri>>(self, instance: I) -> Self {
        Self {
            instance: Some(instance.into()),
            ..self
        }
    }

    pub fn with_detail<S: Serialize>(self, detail: S) -> Self {
        Self {
            detail: serde_json::to_value(detail).unwrap(),
            ..self
        }
    }

    pub fn with_detail_msg<S: ToString>(self, detail: S) -> Self {
        Self {
            detail: JsonValue::String(detail.to_string()),
            ..self
        }
    }

    pub fn with_confidential<FL, FF>(self, config: &ProblemConfig, minimal: FL, full: FF) -> Self
    where
        FL: FnOnce(Self) -> Self,
        FF: FnOnce(Self) -> Self,
    {
        if config.include_internal {
            full(self)
        } else {
            minimal(self)
        }
    }
}

/// Implementation of a Problem Details response for HTTP APIs, as defined
/// in [RFC-7807](https://datatracker.ietf.org/doc/html/rfc7807).
pub trait IntoProblem {
    fn into_problem(self, config: &ProblemConfig) -> Problem;
}

pub struct ProblemDetail<P: IntoProblem> {
    pub config: ProblemConfig,
    pub problem: P,
}

impl<P: IntoProblem> ProblemDetail<P> {
    pub fn from(config: &ProblemConfig, problem: P) -> Self {
        Self {
            config: config.clone(),
            problem,
        }
    }
}

impl<P: IntoProblem> IntoResponse for ProblemDetail<P> {
    fn into_response(self) -> Response {
        let ProblemDetail { problem, config } = self;

        let body = problem.into_problem(&config);
        let mut response = (body.status, Json(body)).into_response();

        response
            .headers_mut()
            .insert("content-type", "application/problem+json".parse().unwrap());
        response
    }
}
