use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::UserId;

/// Authenticated principal returned by `GET /admin/users/current`.
///
/// Unknown fields are preserved in `extra` (via `#[serde(flatten)]`) so schema
/// drift on the UserVoice side does not break decoding. The known fields
/// (`id`, `name`, `email_address`, dates) are the only ones the application
/// layer depends on; everything else passes through.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub email_address: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Wire shape of the `GET /admin/users/current` response body.
#[derive(Debug, Deserialize)]
pub(crate) struct UsersResponse {
    pub users: Vec<User>,
}
