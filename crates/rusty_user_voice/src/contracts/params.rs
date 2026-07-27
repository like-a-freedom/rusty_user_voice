use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uservoice_client::{CategoryId, Continuation, ForumId, IdeaId, LabelId, StatusId, UserId};

pub use uservoice_client::FeedbackChannel;

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
    pub query: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_id_new_is_not_nil() {
        let id = ChangeId::new();
        assert!(!id.as_uuid().is_nil());
    }

    #[test]
    fn change_id_from_uuid_roundtrips() {
        let uuid = uuid::Uuid::new_v4();
        let id = ChangeId::from_uuid(uuid);
        assert_eq!(id.as_uuid(), uuid);
    }

    #[test]
    fn update_idea_params_preview_sets_mode() {
        let params = UpdateIdeaParams::preview(IdeaId::new(1), Some("title".to_owned()), vec![]);
        assert_eq!(params.mode, MutationMode::Preview);
        assert_eq!(params.idea_id, IdeaId::new(1));
        assert!(params.change_id.is_none());
    }

    #[test]
    fn update_idea_params_apply_from_preview_copies_fields() {
        let preview = UpdateIdeaParams::preview(
            IdeaId::new(42),
            Some("title".to_owned()),
            vec![LabelId::new(14)],
        );
        let change_id = ChangeId::new();
        let apply = UpdateIdeaParams::apply_from_preview(&preview, change_id, "apply", "key-1");
        assert_eq!(apply.mode, MutationMode::Apply);
        assert_eq!(apply.idea_id, IdeaId::new(42));
        assert_eq!(apply.title, Some("title".to_owned()));
        assert_eq!(apply.add_label_ids, vec![LabelId::new(14)]);
        assert_eq!(apply.change_id, Some(change_id));
        assert_eq!(apply.confirmation, Some("apply".to_owned()));
        assert_eq!(apply.idempotency_key, Some("key-1".to_owned()));
    }

    #[test]
    fn publish_idea_update_params_preview_sets_defaults() {
        let params =
            PublishIdeaUpdateParams::preview(IdeaId::new(1), StatusId::new(10), "shipped", false);
        assert_eq!(params.mode, MutationMode::Preview);
        assert!(!params.notify_supporters);
        assert!(params.change_id.is_none());
    }

    #[test]
    fn publish_idea_update_params_apply_from_preview_copies() {
        let change_id = ChangeId::new();
        let preview_result = crate::MutationResult {
            mode: MutationMode::Preview,
            change_id: Some(change_id),
            idea_id: IdeaId::new(1),
            expected_updated_at: chrono::Utc::now(),
            expires_at: None,
            changes: Vec::new(),
            applied_operations: Vec::new(),
            feedback_record_id: None,
        };
        let apply = PublishIdeaUpdateParams::apply_from_preview(
            &preview_result,
            StatusId::new(20),
            "released",
            true,
            "apply",
            "key-1",
        );
        assert_eq!(apply.mode, MutationMode::Apply);
        assert_eq!(apply.idea_id, IdeaId::new(1));
        assert_eq!(apply.status_id, StatusId::new(20));
        assert_eq!(apply.message, "released");
        assert!(apply.notify_supporters);
        assert_eq!(apply.change_id, Some(change_id));
    }
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationMode {
    Preview,
    Apply,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ChangeId(uuid::Uuid);

impl ChangeId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub const fn from_uuid(value: uuid::Uuid) -> Self {
        Self(value)
    }

    pub const fn as_uuid(self) -> uuid::Uuid {
        self.0
    }
}

impl Default for ChangeId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateIdeaParams {
    pub mode: MutationMode,
    pub idea_id: IdeaId,
    pub title: Option<String>,
    pub body: Option<String>,
    pub forum_id: Option<ForumId>,
    pub category_id: Option<CategoryId>,
    pub assigned_owner_id: Option<UserId>,
    #[serde(default)]
    pub add_label_ids: Vec<LabelId>,
    #[serde(default)]
    pub remove_label_ids: Vec<LabelId>,
    pub internal_note: Option<String>,
    pub change_id: Option<ChangeId>,
    pub confirmation: Option<String>,
    pub idempotency_key: Option<String>,
}

impl UpdateIdeaParams {
    pub fn preview(idea_id: IdeaId, title: Option<String>, add_label_ids: Vec<LabelId>) -> Self {
        Self {
            mode: MutationMode::Preview,
            idea_id,
            title,
            body: None,
            forum_id: None,
            category_id: None,
            assigned_owner_id: None,
            add_label_ids,
            remove_label_ids: Vec::new(),
            internal_note: None,
            change_id: None,
            confirmation: None,
            idempotency_key: None,
        }
    }

    pub fn apply_from_preview(
        preview: &Self,
        change_id: ChangeId,
        confirmation: &str,
        idempotency_key: &str,
    ) -> Self {
        Self {
            mode: MutationMode::Apply,
            idea_id: preview.idea_id,
            title: preview.title.clone(),
            body: preview.body.clone(),
            forum_id: preview.forum_id,
            category_id: preview.category_id,
            assigned_owner_id: preview.assigned_owner_id,
            add_label_ids: preview.add_label_ids.clone(),
            remove_label_ids: preview.remove_label_ids.clone(),
            internal_note: preview.internal_note.clone(),
            change_id: Some(change_id),
            confirmation: Some(confirmation.to_owned()),
            idempotency_key: Some(idempotency_key.to_owned()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublishIdeaUpdateParams {
    pub mode: MutationMode,
    pub idea_id: IdeaId,
    pub status_id: StatusId,
    pub message: String,
    #[serde(default)]
    pub notify_supporters: bool,
    pub change_id: Option<ChangeId>,
    pub confirmation: Option<String>,
    pub idempotency_key: Option<String>,
}

impl PublishIdeaUpdateParams {
    pub fn preview(
        idea_id: IdeaId,
        status_id: StatusId,
        message: &str,
        notify_supporters: bool,
    ) -> Self {
        Self {
            mode: MutationMode::Preview,
            idea_id,
            status_id,
            message: message.to_owned(),
            notify_supporters,
            change_id: None,
            confirmation: None,
            idempotency_key: None,
        }
    }

    pub fn apply_from_preview(
        preview: &crate::MutationResult,
        status_id: StatusId,
        message: &str,
        notify_supporters: bool,
        confirmation: &str,
        idempotency_key: &str,
    ) -> Self {
        Self {
            mode: MutationMode::Apply,
            idea_id: preview.idea_id,
            status_id,
            message: message.to_owned(),
            notify_supporters,
            change_id: preview.change_id,
            confirmation: Some(confirmation.to_owned()),
            idempotency_key: Some(idempotency_key.to_owned()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordFeedbackParams {
    pub body: String,
    pub channel: FeedbackChannel,
    pub idea_id: Option<IdeaId>,
    pub user_id: Option<UserId>,
    pub user_email: Option<String>,
    pub source_url: Option<String>,
    pub source_guid: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub idempotency_key: String,
}
