use crate::service::SessionKey;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug)]
pub struct UserSession {
    pub user_id: Uuid,
    pub key: SessionKey,

    pub created_at: DateTime<Utc>,
    //pub client_agent: String,
}
