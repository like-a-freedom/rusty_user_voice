use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    CategoryId, ForumId, IdeaId, LabelId, Pagination, StatusId, UpstreamPagination, UserId,
};

/// Deserialises a JSON array-or-null into `Vec<T>`. Null and missing both produce
/// an empty vec. The UserVoice API returns `null` for suggestion labels when an
/// idea has no labels attached, which serde's `#[serde(default)]` does not handle
/// (default only covers missing keys, not explicit null values).
fn deserialize_null_vec<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    match Option::<Vec<T>>::deserialize(deserializer)? {
        Some(v) => Ok(v),
        None => Ok(Vec::new()),
    }
}

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
    #[serde(default, deserialize_with = "deserialize_null_vec")]
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
    #[serde(default)]
    pub body: Option<String>,
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
    #[serde(default)]
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

/// Why a catalog section is missing.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PartialReason {
    /// The API throttled the read. Retrying after the reset is reasonable.
    Throttled,
    /// The API key cannot read the section. Retrying will not help.
    Forbidden,
    /// The endpoint or its response is not something this client can use.
    /// Retrying will not help until the client or the tenant changes.
    Unavailable,
    /// A transient failure. Retrying later is reasonable.
    Failed,
}

/// One accepted value of a `select` or `multi_select` field.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct EnumOption {
    pub id: Option<i64>,
    pub label: Option<String>,
    /// Present on `numeric_enum` options only.
    pub numeric_value: Option<f64>,
    pub is_default: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// One custom field definition, as `GET /admin/custom_fields` lists it.
///
/// `key` is what an update body must carry (`cf_tfs_link`). The value shape
/// follows `field_type` and `value_type`:
///
/// - `string` and `text` field types take a JSON string; a `url` value type
///   is still a string.
/// - `number` takes a JSON number, `boolean` a JSON boolean.
/// - `timestamp` takes a quoted date or RFC 3339 string; a unix number is
///   rejected by the API.
/// - `select` with `value_type: admin` takes the admin's numeric id.
/// - `select` and `multi_select` list their accepted options in
///   [`Self::string_enum_values`] or [`Self::numeric_enum_values`].
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct CustomField {
    pub key: String,
    pub name: Option<String>,
    /// `string`, `text`, `number`, `boolean`, `timestamp`, `select`, or
    /// `multi_select` — optional so one odd definition cannot fail the whole
    /// catalog.
    #[serde(default)]
    pub field_type: Option<String>,
    #[serde(default)]
    pub value_type: Option<String>,
    pub object_type: Option<String>,
    /// The accepted options of a `select` field. The API sends `null` for an
    /// admin select, which has no fixed option list.
    #[serde(
        default,
        deserialize_with = "deserialize_null_vec",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub string_enum_values: Vec<EnumOption>,
    #[serde(
        default,
        deserialize_with = "deserialize_null_vec",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub numeric_enum_values: Vec<EnumOption>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct WorkspaceCatalog {
    pub forums: Vec<Forum>,
    pub categories: Vec<Category>,
    pub statuses: Vec<Status>,
    pub labels: Vec<Label>,
    /// The custom fields defined for ideas, so a caller can pick a key
    /// instead of guessing one.
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
    /// Catalog sections the API refused or failed to return. The rest of the
    /// catalog is still usable, and the name says what is absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_sections: Vec<String>,
    /// Why the missing sections are absent, so a caller can tell a throttle
    /// (retry) from a permission problem (do not retry). `None` when every
    /// section loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_reason: Option<PartialReason>,
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

#[derive(Debug, Deserialize)]
pub(crate) struct CustomFieldsResponse {
    #[serde(default, deserialize_with = "deserialize_null_vec")]
    pub custom_fields: Vec<CustomField>,
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
