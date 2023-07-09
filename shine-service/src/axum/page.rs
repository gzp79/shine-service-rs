use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

pub struct Page {
    status: StatusCode,
    html: Html<String>,
}

impl Page {
    pub fn new<S: ToString>(body: S) -> Self {
        Self {
            status: StatusCode::OK,
            html: Html(body.to_string()),
        }
    }

    pub fn new_with_status<S: ToString>(status: StatusCode, body: S) -> Self {
        Self {
            status,
            html: Html(body.to_string()),
        }
    }
}

impl IntoResponse for Page {
    fn into_response(self) -> Response {
        (self.status, self.html).into_response()
    }
}
