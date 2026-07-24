use rmcp::ErrorData as McpError;
use thiserror::Error;

/// Application-layer error type. `#[non_exhaustive]` because new upstream or
/// ledger failure modes may be added in future revisions without a breaking
/// version bump (F-007, `api-non_exhaustive`).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("UserVoice request failed: {0}")]
    Client(#[from] uservoice_client::ClientError),
    #[error("state I/O failed")]
    Io(#[from] std::io::Error),
    #[error("state JSON failed")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    State(String),
}

impl From<AppError> for McpError {
    fn from(value: AppError) -> Self {
        match value {
            AppError::Validation(message) => McpError::invalid_params(message, None),
            AppError::Client(uservoice_client::ClientError::NotFound) => McpError::internal_error(
                "idea was not found",
                Some(serde_json::json!({
                    "status": "not_found",
                    "guidance": "call uservoice_search_ideas to refresh the idea_id",
                })),
            ),
            AppError::Conflict(message) => McpError::internal_error(
                message.clone(),
                Some(serde_json::json!({
                    "status": "conflict",
                    "guidance": conflict_guidance(&message),
                })),
            ),
            other => McpError::internal_error(other.to_string(), None),
        }
    }
}

fn conflict_guidance(message: &str) -> String {
    if let Some(expires_at) = message
        .strip_prefix("change_id ")
        .and_then(|rest| rest.split_once(" has expired; expires_at = "))
        .map(|(_, ts)| ts)
    {
        format!(
            "preview expired at {expires_at}; the idea may have changed — \
             re-search, then call uservoice_update_idea with mode=preview again"
        )
    } else {
        "state mismatch; re-read the affected resource and re-issue the preview".to_owned()
    }
}
