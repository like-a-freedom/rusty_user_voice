use std::collections::BTreeMap;

use uservoice_client::{SearchSuggestionsRequest, UserVoiceApi};

use crate::{
    App, CollectIdeasParams, DetailLevel, Envelope, GetIdeaContextParams, IdeaPage, NextAction,
    OutcomeStatus, error::AppError,
};

impl<G: UserVoiceApi> App<G> {
    pub async fn collect_ideas(
        &self,
        params: CollectIdeasParams,
    ) -> Result<Envelope<IdeaPage>, AppError> {
        if matches!(params.detail_level, DetailLevel::Full) && params.per_page > 10 {
            return Err(AppError::Validation(
                "full-detail collection requires per_page <= 10".to_owned(),
            ));
        }
        let page = self
            .api
            .search_suggestions(SearchSuggestionsRequest {
                query: None,
                updated_after: Some(params.updated_after),
                updated_before: params.updated_before,
                forum_ids: Vec::new(),
                category_ids: Vec::new(),
                status_ids: Vec::new(),
                label_ids: Vec::new(),
                states: Vec::new(),
                sort: Some("updated_at".to_owned()),
                per_page: params.per_page,
                continuation: params.continuation,
            })
            .await?;
        let pagination = page.data.pagination.clone();
        let mut ideas = Vec::with_capacity(page.data.items.len());
        let mut partial = false;
        for suggestion in page.data.items {
            if matches!(params.detail_level, DetailLevel::Full) {
                let context = self
                    .get_idea_context(GetIdeaContextParams {
                        idea_id: suggestion.id,
                        detail_level: DetailLevel::Full,
                        include_supporters: true,
                        per_page: 100,
                    })
                    .await?;
                partial |= matches!(context.status, OutcomeStatus::Partial);
                ideas.push(context.data);
            } else {
                ideas.push(crate::IdeaContext {
                    suggestion,
                    comments: Vec::new(),
                    notes: Vec::new(),
                    supporters: Vec::new(),
                    feedback_records: Vec::new(),
                    status_updates: Vec::new(),
                    activity: Vec::new(),
                    missing_sections: Vec::new(),
                });
            }
        }
        Ok(Envelope {
            status: if partial {
                OutcomeStatus::Partial
            } else if ideas.is_empty() {
                OutcomeStatus::NoResults
            } else {
                OutcomeStatus::Ok
            },
            data: IdeaPage { ideas },
            next_actions: pagination
                .next
                .as_ref()
                .map(|_| NextAction {
                    tool: "uservoice_collect_ideas".to_owned(),
                    reason: "Continue with the returned pagination value.".to_owned(),
                    idea_id: None,
                    arguments: BTreeMap::from([
                        (
                            "updated_after".to_owned(),
                            serde_json::json!(params.updated_after),
                        ),
                        (
                            "updated_before".to_owned(),
                            serde_json::json!(params.updated_before),
                        ),
                        (
                            "continuation".to_owned(),
                            serde_json::json!(&pagination.next),
                        ),
                    ]),
                })
                .into_iter()
                .collect(),
            guidance: partial.then(|| {
                "At least one idea has missing_sections; retry those idea contexts before making a decision."
                    .to_owned()
            }),
            pagination: Some(pagination),
            rate_limit: page.rate_limit,
        })
    }
}
