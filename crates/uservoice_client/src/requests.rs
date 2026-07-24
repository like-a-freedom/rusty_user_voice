use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

use crate::{CategoryId, ClientError, Continuation, ForumId, IdeaId, LabelId, StatusId, UserId};

#[derive(Clone, Debug)]
pub struct SearchSuggestionsRequest {
    pub query: Option<String>,
    pub updated_after: Option<DateTime<Utc>>,
    pub updated_before: Option<DateTime<Utc>>,
    pub forum_ids: Vec<ForumId>,
    pub category_ids: Vec<CategoryId>,
    pub status_ids: Vec<StatusId>,
    pub label_ids: Vec<LabelId>,
    pub states: Vec<String>,
    pub sort: Option<String>,
    pub per_page: u32,
    pub continuation: Option<Continuation>,
}

impl SearchSuggestionsRequest {
    pub fn validate(&self) -> Result<(), ClientError> {
        crate::pagination::validate_per_page(self.per_page)?;
        Ok(())
    }

    pub(crate) fn query_pairs(&self) -> Result<Vec<(String, String)>, ClientError> {
        self.validate()?;
        let mut pairs = vec![("per_page".to_owned(), self.per_page.to_string())];
        push_option(&mut pairs, "q", self.query.clone());
        push_option(&mut pairs, "sort", self.sort.clone());
        push_date(&mut pairs, "updated_after", self.updated_after);
        push_date(&mut pairs, "updated_before", self.updated_before);
        push_ids(&mut pairs, "forum", &self.forum_ids, ForumId::get);
        push_ids(&mut pairs, "category", &self.category_ids, CategoryId::get);
        push_ids(&mut pairs, "status", &self.status_ids, StatusId::get);
        push_ids(&mut pairs, "label", &self.label_ids, LabelId::get);
        if !self.states.is_empty() {
            pairs.push(("state".to_owned(), self.states.join(",")));
        }
        match &self.continuation {
            Some(Continuation::Cursor { cursor }) => {
                pairs.push(("cursor".to_owned(), cursor.clone()));
            }
            Some(Continuation::Page { page }) => {
                pairs.push(("page".to_owned(), page.to_string()));
            }
            None => {}
        }
        Ok(pairs)
    }
}

fn push_option(pairs: &mut Vec<(String, String)>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|item| !item.is_empty()) {
        pairs.push((key.to_owned(), value));
    }
}

fn push_date(pairs: &mut Vec<(String, String)>, key: &str, value: Option<DateTime<Utc>>) {
    if let Some(value) = value {
        pairs.push((
            key.to_owned(),
            value.to_rfc3339_opts(SecondsFormat::Secs, false),
        ));
    }
}

fn push_ids<T: Copy, F>(pairs: &mut Vec<(String, String)>, key: &str, values: &[T], value: F)
where
    F: Fn(T) -> i64,
{
    if values.is_empty() {
        return;
    }
    let joined = values
        .iter()
        .copied()
        .map(value)
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    pairs.push((key.to_owned(), joined));
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct UpdateSuggestionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<UpdateSuggestionLinks>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct UpdateSuggestionLinks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forum: Option<ForumId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<CategoryId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_owner: Option<UserId>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LabelMutationRequest {
    pub links: LabelMutationLinks,
}

#[derive(Clone, Debug, Serialize)]
pub struct LabelMutationLinks {
    pub label: LabelId,
    pub suggestion: IdeaId,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateNoteRequest {
    pub body: String,
    pub body_mime_type: &'static str,
    pub links: CreateNoteLinks,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateNoteLinks {
    pub suggestion: IdeaId,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateStatusUpdateRequest {
    pub body: String,
    pub links: CreateStatusUpdateLinks,
    pub notify_supporters: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateStatusUpdateLinks {
    pub suggestion: IdeaId,
    pub status: StatusId,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, schemars::JsonSchema, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackChannel {
    Admin,
    Api,
    Bookmarklet,
    Gainsight,
    Import,
    Portal,
    Roadmap,
    Slack,
    Zendesk,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateFeedbackRecordRequest {
    pub body: String,
    pub body_mime_type: &'static str,
    pub channel: FeedbackChannel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_guid: Option<String>,
    pub links: CreateFeedbackRecordLinks,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateFeedbackRecordLinks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<IdeaId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,
}
