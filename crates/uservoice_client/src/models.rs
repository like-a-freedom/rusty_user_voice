use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    CategoryId, ForumId, IdeaId, LabelId, Pagination, StatusId, UpstreamPagination, UserId,
};

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

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
pub struct SuggestionLinks {
    pub forum: Option<ForumId>,
    pub category: Option<CategoryId>,
    pub status: Option<StatusId>,
    #[serde(default)]
    pub labels: Vec<LabelId>,
    pub created_by: Option<UserId>,
    pub assigned_owner: Option<UserId>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Suggestion {
    pub id: IdeaId,
    pub title: String,
    pub body: String,
    pub body_mime_type: Option<String>,
    pub channel: Option<String>,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub comments_count: u64,
    pub notes_count: u64,
    pub supporters_count: u64,
    pub requests_count: u64,
    pub portal_url: Option<String>,
    pub admin_url: Option<String>,
    pub links: SuggestionLinks,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub included: BTreeMap<String, Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct SuggestionsResponse {
    pub suggestions: Vec<Suggestion>,
    #[serde(default)]
    pub pagination: UpstreamPagination,
    #[serde(flatten)]
    pub included: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct SuggestionPage {
    pub items: Vec<Suggestion>,
    pub pagination: Pagination,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Forum {
    pub id: ForumId,
    pub name: String,
    pub is_public: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub links: BTreeMap<String, Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Status {
    pub id: StatusId,
    pub name: String,
    pub is_open: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Label {
    pub id: LabelId,
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct WorkspaceCatalog {
    pub forums: Vec<Forum>,
    pub categories: Vec<Category>,
    pub statuses: Vec<Status>,
    pub labels: Vec<Label>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ForumsResponse {
    pub forums: Vec<Forum>,
    #[serde(default)]
    pub pagination: UpstreamPagination,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CategoriesResponse {
    pub categories: Vec<Category>,
    #[serde(default)]
    pub pagination: UpstreamPagination,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StatusesResponse {
    pub statuses: Vec<Status>,
    #[serde(default)]
    pub pagination: UpstreamPagination,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LabelsResponse {
    pub labels: Vec<Label>,
    #[serde(default)]
    pub pagination: UpstreamPagination,
}

macro_rules! context_record {
    ($name:ident, $id:ty) => {
        #[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
        pub struct $name {
            pub id: $id,
            pub created_at: Option<DateTime<Utc>>,
            pub updated_at: Option<DateTime<Utc>>,
            #[serde(flatten)]
            pub extra: BTreeMap<String, Value>,
        }
    };
}

context_record!(Comment, crate::CommentId);
context_record!(Note, crate::NoteId);
context_record!(FeedbackRecord, crate::FeedbackRecordId);
context_record!(StatusUpdate, crate::StatusUpdateId);

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Supporter {
    pub id: crate::SupporterId,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct ActivityEntry {
    pub id: i64,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct ContextPage<T> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}

macro_rules! collection_response {
    ($name:ident, $field:ident, $item:ty) => {
        #[derive(Debug, Deserialize)]
        pub(crate) struct $name {
            pub $field: Vec<$item>,
            #[serde(default)]
            pub pagination: UpstreamPagination,
        }
    };
}

collection_response!(CommentsResponse, comments, Comment);
collection_response!(NotesResponse, notes, Note);
collection_response!(SupportersResponse, supporters, Supporter);
collection_response!(FeedbackRecordsResponse, feedback_records, FeedbackRecord);
collection_response!(StatusUpdatesResponse, status_updates, StatusUpdate);
collection_response!(ActivityResponse, suggestion_activity_entries, ActivityEntry);

#[derive(Debug, Default, Deserialize)]
pub(crate) struct MutationResponse {
    #[serde(default)]
    pub suggestions: Vec<Suggestion>,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub status_updates: Vec<StatusUpdate>,
    #[serde(default)]
    pub feedback_records: Vec<FeedbackRecord>,
}
