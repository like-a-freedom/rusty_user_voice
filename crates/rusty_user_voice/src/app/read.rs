use std::collections::BTreeMap;

use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use uservoice_client::{
    ActivityEntry, Comment, FeedbackRecord, Note, SearchSuggestionsRequest, StatusUpdate,
    Supporter, UserVoiceApi,
};

use crate::{
    App, DetailLevel, Envelope, GetIdeaContextParams, IdeaContext, IdeaPage, NextAction,
    OutcomeStatus, SearchIdeasParams, WorkspaceContext, WorkspaceContextParams, error::AppError,
};

enum SectionData {
    Comments(Vec<Comment>),
    Notes(Vec<Note>),
    Supporters(Vec<Supporter>),
    Feedback(Vec<FeedbackRecord>),
    StatusUpdates(Vec<StatusUpdate>),
    Activity(Vec<ActivityEntry>),
}

#[derive(Clone, Copy)]
enum SectionKind {
    Comments,
    Notes,
    Feedback,
    StatusUpdates,
    Activity,
}

impl SectionKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Comments => "comments",
            Self::Notes => "notes",
            Self::Feedback => "feedback_records",
            Self::StatusUpdates => "status_updates",
            Self::Activity => "activity",
        }
    }
}

impl<G: UserVoiceApi> App<G> {
    pub async fn workspace_context(
        &self,
        params: WorkspaceContextParams,
    ) -> Result<Envelope<WorkspaceContext>, AppError> {
        let current = self.api.current_user().await?;
        let (catalog, catalog_rate_limit) = if params.include_catalog {
            let response = self.api.workspace_catalog().await?;
            (Some(response.data), response.rate_limit)
        } else {
            (None, None)
        };
        let rate_limit = conservative_rate_limit(current.rate_limit.clone(), catalog_rate_limit);
        Ok(Envelope {
            status: OutcomeStatus::Ok,
            data: WorkspaceContext {
                current_user: current.data,
                catalog,
            },
            next_actions: vec![NextAction {
                tool: "uservoice_search_ideas".to_owned(),
                reason: "Find ideas using the returned workspace taxonomy.".to_owned(),
                idea_id: None,
                arguments: BTreeMap::new(),
            }],
            guidance: None,
            pagination: None,
            rate_limit,
        })
    }

    pub async fn search_ideas(
        &self,
        params: SearchIdeasParams,
    ) -> Result<Envelope<IdeaPage>, AppError> {
        let page = self
            .api
            .search_suggestions(SearchSuggestionsRequest {
                query: params.query,
                updated_after: params.updated_after,
                updated_before: params.updated_before,
                forum_ids: params.forum_ids,
                category_ids: params.category_ids,
                status_ids: params.status_ids,
                label_ids: params.label_ids,
                states: params.states,
                sort: params.sort,
                per_page: params.per_page,
                continuation: params.continuation,
            })
            .await?;
        let status = if page.data.items.is_empty() {
            OutcomeStatus::NoResults
        } else {
            OutcomeStatus::Ok
        };
        let ideas = page
            .data
            .items
            .into_iter()
            .map(|suggestion| IdeaContext {
                suggestion,
                comments: Vec::new(),
                notes: Vec::new(),
                supporters: Vec::new(),
                feedback_records: Vec::new(),
                status_updates: Vec::new(),
                activity: Vec::new(),
                missing_sections: Vec::new(),
            })
            .collect();
        Ok(Envelope {
            status,
            data: IdeaPage { ideas },
            next_actions: vec![NextAction {
                tool: "uservoice_get_idea_context".to_owned(),
                reason: "Load full context for a selected idea ID.".to_owned(),
                idea_id: None,
                arguments: BTreeMap::new(),
            }],
            guidance: None,
            pagination: Some(page.data.pagination),
            rate_limit: page.rate_limit,
        })
    }

    pub async fn get_idea_context(
        &self,
        params: GetIdeaContextParams,
    ) -> Result<Envelope<IdeaContext>, AppError> {
        let suggestion = self.api.suggestion(params.idea_id).await?;
        let mut rate_limit = suggestion.rate_limit.clone();
        let mut context = IdeaContext {
            suggestion: suggestion.data,
            comments: Vec::new(),
            notes: Vec::new(),
            supporters: Vec::new(),
            feedback_records: Vec::new(),
            status_updates: Vec::new(),
            activity: Vec::new(),
            missing_sections: Vec::new(),
        };
        if matches!(params.detail_level, DetailLevel::Summary) {
            return Ok(context_envelope(context, rate_limit));
        }

        let semaphore = Arc::new(Semaphore::new(self.max_in_flight.get()));
        let mut tasks = JoinSet::new();
        for section in [
            SectionKind::Comments,
            SectionKind::Notes,
            SectionKind::Feedback,
            SectionKind::StatusUpdates,
            SectionKind::Activity,
        ] {
            let api = Arc::clone(&self.api);
            let semaphore = Arc::clone(&semaphore);
            let idea_id = params.idea_id;
            let per_page = params.per_page;
            tasks.spawn(async move {
                let Ok(_permit) = semaphore.acquire_owned().await else {
                    return (
                        section.name(),
                        Err(uservoice_client::ClientError::HttpStatus { status: 500 }),
                    );
                };
                let value = match section {
                    SectionKind::Comments => api
                        .comments(idea_id, per_page)
                        .await
                        .map(|value| (SectionData::Comments(value.data.items), value.rate_limit)),
                    SectionKind::Notes => api
                        .notes(idea_id, per_page)
                        .await
                        .map(|value| (SectionData::Notes(value.data.items), value.rate_limit)),
                    SectionKind::Feedback => api
                        .feedback_records(idea_id, per_page)
                        .await
                        .map(|value| (SectionData::Feedback(value.data.items), value.rate_limit)),
                    SectionKind::StatusUpdates => {
                        api.status_updates(idea_id, per_page).await.map(|value| {
                            (
                                SectionData::StatusUpdates(value.data.items),
                                value.rate_limit,
                            )
                        })
                    }
                    SectionKind::Activity => api
                        .activity(idea_id, per_page)
                        .await
                        .map(|value| (SectionData::Activity(value.data.items), value.rate_limit)),
                };
                (section.name(), value)
            });
        }
        if params.include_supporters {
            let api = Arc::clone(&self.api);
            let semaphore = Arc::clone(&semaphore);
            let idea_id = params.idea_id;
            let limit = params.per_page;
            tasks.spawn(async move {
                let Ok(_permit) = semaphore.acquire_owned().await else {
                    return (
                        "supporters",
                        Err(uservoice_client::ClientError::HttpStatus { status: 500 }),
                    );
                };
                (
                    "supporters",
                    api.supporters(idea_id, limit)
                        .await
                        .map(|value| (SectionData::Supporters(value.data.items), value.rate_limit)),
                )
            });
        }

        while let Some(result) = tasks.join_next().await {
            match result {
                Ok((_, Ok((SectionData::Comments(items), rate)))) => {
                    context.comments = items;
                    rate_limit = conservative_rate_limit(rate_limit, rate);
                }
                Ok((_, Ok((SectionData::Notes(items), rate)))) => {
                    context.notes = items;
                    rate_limit = conservative_rate_limit(rate_limit, rate);
                }
                Ok((_, Ok((SectionData::Supporters(items), rate)))) => {
                    context.supporters = items;
                    rate_limit = conservative_rate_limit(rate_limit, rate);
                }
                Ok((_, Ok((SectionData::Feedback(items), rate)))) => {
                    context.feedback_records = items;
                    rate_limit = conservative_rate_limit(rate_limit, rate);
                }
                Ok((_, Ok((SectionData::StatusUpdates(items), rate)))) => {
                    context.status_updates = items;
                    rate_limit = conservative_rate_limit(rate_limit, rate);
                }
                Ok((_, Ok((SectionData::Activity(items), rate)))) => {
                    context.activity = items;
                    rate_limit = conservative_rate_limit(rate_limit, rate);
                }
                Ok((name, Err(_))) => context.missing_sections.push(name.to_owned()),
                Err(_) => context.missing_sections.push("task".to_owned()),
            }
        }
        context.missing_sections.sort();
        let mut envelope = context_envelope(context, rate_limit);
        if !envelope.data.missing_sections.is_empty() {
            envelope.status = OutcomeStatus::Partial;
            envelope.guidance = Some(
                "Retry uservoice_get_idea_context for the same idea; existing sections are usable."
                    .to_owned(),
            );
            let idea_id = envelope.data.suggestion.id;
            envelope.next_actions = vec![NextAction {
                tool: "uservoice_get_idea_context".to_owned(),
                reason: "Retry to recover missing sections.".to_owned(),
                idea_id: Some(idea_id),
                arguments: BTreeMap::from([
                    ("idea_id".to_owned(), serde_json::json!(idea_id)),
                    ("detail_level".to_owned(), serde_json::json!("full")),
                ]),
            }];
        }
        Ok(envelope)
    }
}

fn conservative_rate_limit(
    current: Option<uservoice_client::RateLimit>,
    candidate: Option<uservoice_client::RateLimit>,
) -> Option<uservoice_client::RateLimit> {
    match (current, candidate) {
        (Some(current), Some(candidate)) if candidate.remaining < current.remaining => {
            Some(candidate)
        }
        (Some(current), _) => Some(current),
        (None, candidate) => candidate,
    }
}

fn context_envelope(
    context: IdeaContext,
    rate_limit: Option<uservoice_client::RateLimit>,
) -> Envelope<IdeaContext> {
    let idea_id = context.suggestion.id;
    Envelope {
        status: OutcomeStatus::Ok,
        data: context,
        next_actions: vec![NextAction {
            tool: "uservoice_update_idea".to_owned(),
            reason: "Preview an evidence-backed change if feedback requires action.".to_owned(),
            idea_id: Some(idea_id),
            arguments: BTreeMap::from([
                ("mode".to_owned(), serde_json::json!("preview")),
                ("idea_id".to_owned(), serde_json::json!(idea_id)),
            ]),
        }],
        guidance: None,
        pagination: None,
        rate_limit,
    }
}
