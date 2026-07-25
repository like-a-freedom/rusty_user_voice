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
            AppError::Client(uservoice_client::ClientError::PaginationLimit {
                section,
                limit_hit: true,
            }) => McpError::internal_error(
                "context section pagination exceeded the 100-page safety ceiling",
                Some(serde_json::json!({
                    "status": "conflict",
                    "guidance": format!(
                        "section '{section}' hit the bounded fan-out safety ceiling \
                         (ADR-0004); refine the query with a smaller date range or idea \
                         filter, then call uservoice_search_ideas first"
                    ),
                    "pagination": { "limit_hit": true },
                    "next_actions": [{
                        "tool": "uservoice_search_ideas",
                        "reason": "narrow the query so each context section can be drained in ≤100 pages",
                        "arguments": { "page": 1 },
                    }],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_limit_surfaces_as_conflict_with_limit_hit_and_next_action() {
        let err: McpError = AppError::Client(uservoice_client::ClientError::PaginationLimit {
            section: "comments".to_owned(),
            limit_hit: true,
        })
        .into();
        let payload = err
            .data
            .as_ref()
            .expect("pagination-limit error must carry data");
        assert_eq!(payload["status"], "conflict");
        assert_eq!(payload["pagination"]["limit_hit"], serde_json::json!(true));
        let actions = payload["next_actions"]
            .as_array()
            .expect("next_actions array");
        assert!(
            actions
                .iter()
                .any(|a| a["tool"] == "uservoice_search_ideas"),
            "next_actions must include uservoice_search_ideas"
        );
        assert!(
            payload["guidance"]
                .as_str()
                .expect("guidance string")
                .contains("comments"),
            "guidance should name the capped section"
        );
    }

    #[test]
    fn pagination_limit_without_limit_hit_falls_through() {
        let err: McpError = AppError::Client(uservoice_client::ClientError::PaginationLimit {
            section: "forums".to_owned(),
            limit_hit: false,
        })
        .into();
        assert!(
            err.data.is_none() || err.data.as_ref().unwrap().get("status").is_none(),
            "non-conflict pagination caps must not get a status payload"
        );
    }
}
