use chrono::{DateTime, SecondsFormat, Utc};

use crate::{CategoryId, ClientError, Continuation, ForumId, LabelId, StatusId};

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
