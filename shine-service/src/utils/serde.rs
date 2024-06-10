pub mod serde_string {
    use serde::Serializer;
    use std::fmt::Display;

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Display,
        S: Serializer,
    {
        serializer.collect_str(value)
    }
}

pub mod serde_status_code {
    use axum::http::StatusCode;
    use serde::Serializer;

    pub fn serialize<S>(value: &StatusCode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(value.as_u16() as i32)
    }
}

pub mod serde_uri {
    use axum::http::Uri;
    use serde::Serializer;

    pub fn serialize<S>(value: &Uri, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn serialize_opt<S>(value: &Option<Uri>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(value) = value {
            serializer.collect_str(value)
        } else {
            serializer.serialize_none()
        }
    }
}
