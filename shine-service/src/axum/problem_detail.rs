use crate::utils::serde_status_code;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::fmt;
use url::Url;

#[derive(Clone)]
pub struct ProblemConfig {
    pub include_internal: bool,
}

impl ProblemConfig {
    pub fn new(include_internal: bool) -> Self {
        Self { include_internal }
    }

    pub fn into_layer(self) -> Extension<Self> {
        Extension(self)
    }

    pub fn configure<P: IntoProblem>(&self, problem: P) -> ConfiguredProblem<P> {
        ConfiguredProblem {
            config: self.clone(),
            problem,
        }
    }
}

/// Implementation of a Problem Details response for HTTP APIs as of
/// the specification [RFC-7807](https://datatracker.ietf.org/doc/html/rfc7807).
#[derive(Debug, Serialize)]
pub struct Problem {
    #[serde(rename = "status", serialize_with = "serde_status_code::serialize")]
    status: StatusCode,
    #[serde(rename = "type")]
    ty: &'static str,
    #[serde(rename = "instance")]
    instance: Option<Url>,
    #[serde(rename = "detail")]
    detail: String,
    #[serde(rename = "extension")]
    extension: JsonValue,
}

impl Problem {
    pub fn new(status: StatusCode, ty: &'static str) -> Self {
        Problem {
            status,
            ty,
            instance: None,
            detail: String::new(),
            extension: JsonValue::Null,
        }
    }

    pub fn bad_request(ty: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ty)
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not-found")
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized")
    }

    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden")
    }

    pub fn internal_error<M, F>(config: &ProblemConfig, minimal: M, full: F) -> Self
    where
        M: fmt::Display,
        F: fmt::Debug,
    {
        let problem = Self::new(StatusCode::INTERNAL_SERVER_ERROR, "server-error");
        if config.include_internal {
            problem.with_detail(format!("{}: {:#?}", minimal, full))
        } else {
            problem.with_detail(minimal)
        }
    }

    pub fn with_detail<S: ToString>(self, detail: S) -> Self {
        Self {
            detail: detail.to_string(),
            ..self
        }
    }

    pub fn with_instance_str<S: AsRef<str>>(self, instance: S) -> Self {
        self.with_instance(Url::parse(instance.as_ref()).ok())
    }

    pub fn with_instance(self, instance: Option<Url>) -> Self {
        Self { instance, ..self }
    }

    pub fn with_public_extension<S: Serialize>(self, extension: S) -> Self {
        Self {
            extension: serde_json::to_value(extension).unwrap(),
            ..self
        }
    }

    pub fn with_extension<S: Serialize>(self, config: &ProblemConfig, extension: S) -> Self {
        if config.include_internal {
            self.with_public_extension(extension)
        } else {
            self
        }
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(self)).into_response();
        response
            .headers_mut()
            .insert("content-type", "application/problem+json".parse().unwrap());
        response
    }
}

pub trait IntoProblem {
    fn into_problem(self, config: &ProblemConfig) -> Problem;
}

impl IntoProblem for Problem {
    fn into_problem(self, _config: &ProblemConfig) -> Problem {
        self
    }
}

/// A problem that is already configured with a ProblemConfig and can be converted into a response.
pub struct ConfiguredProblem<P: IntoProblem> {
    pub config: ProblemConfig,
    pub problem: P,
}

impl<P: IntoProblem> IntoResponse for ConfiguredProblem<P> {
    fn into_response(self) -> Response {
        let ConfiguredProblem { problem, config } = self;
        problem.into_problem(&config).into_response()
    }
}
