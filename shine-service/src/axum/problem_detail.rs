use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::Value;
use std::{collections::HashMap, error::Error};

/// Implementation of a Problem Details response for HTTP APIs, as defined
/// in [RFC-7807](https://datatracker.ietf.org/doc/html/rfc7807).
pub struct Problem {
    /// The status code of the problem.
    pub status_code: StatusCode,
    /// The actual body of the problem.
    pub body: HashMap<String, Value>,
}

impl Problem {
    pub fn new(status_code: StatusCode) -> Self {
        Self {
            status_code,
            body: HashMap::new(),
        }
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED).with_type("unauthorized")
    }

    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN).with_type("forbidden")
    }

    pub fn bad_request() -> Self {
        Self::new(StatusCode::BAD_REQUEST)
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND).with_type("not_found")
    }

    pub fn internal_error() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR).with_type("server_error")
    }

    pub fn internal_error_from<E: Error>(err: E) -> Self {
        Self::internal_error().with_detail(format!("{err:?}"))
    }

    /// Specify the "type" to use for the problem.
    ///
    /// # Parameters
    /// - `value` - The value to use for the "type"
    #[must_use]
    pub fn with_type<S: ToString>(self, value: S) -> Self {
        self.with_string_value("type", value)
    }

    /// Specify the "title" to use for the problem.
    ///
    /// # Parameters
    /// - `value` - The value to use for the "title"
    #[must_use]
    pub fn with_title<S: ToString>(self, value: S) -> Self {
        self.with_string_value("title", value)
    }

    /// Specify the "detail" as a message to use for the problem.
    ///
    /// # Parameters
    /// - `value` - The value to use for the "detail"
    #[must_use]
    pub fn with_detail<S: ToString>(self, value: S) -> Self {
        self.with_string_value("detail", value)
    }

    /// Specify the "detail" to use for the problem.
    ///
    /// # Parameters
    /// - `value` - The value to use for the "detail"
    #[must_use]
    pub fn with_object_detail<S: Serialize>(self, value: &S) -> Self {
        self.with_object_value("detail", value)
    }

    /// Specify the "instance" to use for the problem.
    ///
    /// # Parameters
    /// - `value` - The value to use for the "instance"
    #[must_use]
    pub fn with_instance<S: ToString>(self, value: S) -> Self {
        self.with_string_value("instance", value)
    }

    /// Specify an arbitrary value string to include in the problem.
    ///
    /// # Parameters
    /// - `key` - The key for the value.
    /// - `value` - The value itself.
    #[must_use]
    pub fn with_string_value<V: ToString>(mut self, key: &str, value: V) -> Self {
        self.body.insert(key.to_owned(), value.to_string().into());

        self
    }

    /// Specify an arbitrary value object to include in the problem.
    ///
    /// # Parameters
    /// - `key` - The key for the value.
    /// - `value` - The value itself.
    #[must_use]
    pub fn with_object_value<V: Serialize>(mut self, key: &str, value: &V) -> Self {
        let value = serde_json::to_value(value).expect("Failed to serialize ");
        self.body.insert(key.to_owned(), value);

        self
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        if self.body.is_empty() {
            self.status_code.into_response()
        } else {
            let body = Json(self.body);
            let mut response = (self.status_code, body).into_response();

            response
                .headers_mut()
                .insert("content-type", "application/problem+json".parse().unwrap());
            response
        }
    }
}
