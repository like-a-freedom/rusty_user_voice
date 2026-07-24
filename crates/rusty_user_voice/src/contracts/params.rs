use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uservoice_client::{CategoryId, Continuation, ForumId, IdeaId, LabelId, StatusId};

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailLevel {
    #[default]
    Summary,
    Full,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceContextParams {
    #[serde(default = "default_true")]
    pub include_catalog: bool,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchIdeasParams {
    pub query: Option<String>,
    pub updated_after: Option<DateTime<Utc>>,
    pub updated_before: Option<DateTime<Utc>>,
    #[serde(default)]
    pub forum_ids: Vec<ForumId>,
    #[serde(default)]
    pub category_ids: Vec<CategoryId>,
    #[serde(default)]
    pub status_ids: Vec<StatusId>,
    #[serde(default)]
    pub label_ids: Vec<LabelId>,
    #[serde(default)]
    pub states: Vec<String>,
    pub sort: Option<String>,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    pub continuation: Option<Continuation>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetIdeaContextParams {
    pub idea_id: IdeaId,
    #[serde(default)]
    pub detail_level: DetailLevel,
    #[serde(default)]
    pub include_supporters: bool,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollectIdeasParams {
    pub updated_after: DateTime<Utc>,
    pub updated_before: Option<DateTime<Utc>>,
    #[serde(default)]
    pub detail_level: DetailLevel,
    #[serde(default = "default_collect_page")]
    pub per_page: u32,
    pub continuation: Option<Continuation>,
}

const fn default_true() -> bool {
    true
}

const fn default_per_page() -> u32 {
    100
}

const fn default_collect_page() -> u32 {
    20
}
