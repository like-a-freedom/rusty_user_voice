use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uservoice_client::{
    ActivityEntry, Comment, FeedbackRecord, Note, Pagination, RateLimit, StatusUpdate, Suggestion,
    Supporter, User, WorkspaceCatalog,
};

/// Application-layer outcome enum. Source of truth for `Envelope<T>::status`.
/// The MCP wire token (`"success"`, `"partial"`, `"no_results"`, `"conflict"`)
/// is produced by `OutcomeStatus::as_wire_status` at the MCP server seam — see
/// ADR-0002. `Partial` serialises on the wire with `isError: false` so the agent
/// loop continues.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OutcomeStatus {
    Ok,
    Partial,
    NoResults,
    Conflict,
}

impl OutcomeStatus {
    /// Returns the canonical MCP wire status token. See ADR-0002.
    pub const fn as_wire_status(self) -> &'static str {
        match self {
            Self::Ok => "success",
            Self::Partial => "partial",
            Self::NoResults => "no_results",
            Self::Conflict => "conflict",
        }
    }
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
