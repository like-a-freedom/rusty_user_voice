use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uservoice_client::{
    ActivityEntry, Comment, FeedbackRecord, Note, Pagination, RateLimit, StatusUpdate, Suggestion,
    Supporter, User, WorkspaceCatalog,
};

/// Application-layer outcome enum. Source of truth for `Envelope<T>::status`.
///
/// The MCP wire token is produced by serde via `#[serde(rename_all =
/// "snake_case")]` — the variants serialise to `"ok"`, `"partial"`,
/// `"no_results"`, and `"conflict"` respectively (ADR-0002). `Partial`
/// serialises on the wire with `isError: false` so the agent loop continues.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OutcomeStatus {
    Ok,
    Partial,
    NoResults,
    Conflict,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct NextAction {
    pub tool: String,
    pub reason: String,
    pub idea_id: Option<uservoice_client::IdeaId>,
    pub arguments: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Envelope<T> {
    pub status: OutcomeStatus,
    pub data: T,
    pub next_actions: Vec<NextAction>,
    pub guidance: Option<String>,
    pub pagination: Option<Pagination>,
    pub rate_limit: Option<RateLimit>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct WorkspaceContext {
    pub current_user: User,
    pub catalog: Option<WorkspaceCatalog>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct IdeaContext {
    pub suggestion: Suggestion,
    pub comments: Vec<Comment>,
    pub notes: Vec<Note>,
    pub supporters: Vec<Supporter>,
    pub feedback_records: Vec<FeedbackRecord>,
    pub status_updates: Vec<StatusUpdate>,
    pub activity: Vec<ActivityEntry>,
    pub missing_sections: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct IdeaPage {
    pub ideas: Vec<IdeaContext>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct FieldChange {
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct AppliedOperation {
    pub operation: String,
    pub resource_id: Option<String>,
    pub outcome: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct MutationResult {
    pub mode: crate::MutationMode,
    pub change_id: Option<crate::ChangeId>,
    pub idea_id: uservoice_client::IdeaId,
    pub expected_updated_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub changes: Vec<FieldChange>,
    pub applied_operations: Vec<AppliedOperation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_record_id: Option<uservoice_client::FeedbackRecordId>,
}
