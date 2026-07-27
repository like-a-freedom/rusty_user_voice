mod support;

use std::sync::Arc;

use rusty_user_voice::{App, CollectIdeasParams, DetailLevel, GetIdeaContextParams, OutcomeStatus};
use uservoice_client::IdeaId;

#[tokio::test]
async fn full_context_contains_all_requested_sections() {
    let api = support::FakeUserVoiceApi::complete();
    let app = App::new(Arc::new(api), 2);
    let result = app
        .get_idea_context(GetIdeaContextParams {
            idea_id: IdeaId::new(101),
            detail_level: DetailLevel::Full,
            include_supporters: true,
            per_page: 100,
        })
        .await
        .expect("context");
    assert_eq!(result.status, OutcomeStatus::Ok);
    assert_eq!(result.data.comments.len(), 1);
    assert_eq!(result.data.notes.len(), 1);
    assert_eq!(result.data.supporters.len(), 1);
    assert_eq!(result.data.feedback_records.len(), 1);
    assert_eq!(result.data.status_updates.len(), 1);
    assert_eq!(result.data.activity.len(), 1);
    assert!(result.data.missing_sections.is_empty());
}

#[tokio::test]
async fn one_optional_failure_returns_partial_with_recovery_action() {
    let api = support::FakeUserVoiceApi::with_failed_section("notes");
    let app = App::new(Arc::new(api), 2);
    let result = app
        .get_idea_context(GetIdeaContextParams {
            idea_id: IdeaId::new(101),
            detail_level: DetailLevel::Full,
            include_supporters: false,
            per_page: 100,
        })
        .await
        .expect("partial context");
    assert_eq!(result.status, OutcomeStatus::Partial);
    assert_eq!(result.data.missing_sections, vec!["notes"]);
    assert_eq!(result.next_actions[0].tool, "uservoice_get_idea_context");
}

#[tokio::test]
async fn full_collection_rejects_an_agent_context_burst_over_ten_ideas() {
    let app = App::new(Arc::new(support::FakeUserVoiceApi::complete()), 2);
    let error = app
        .collect_ideas(CollectIdeasParams {
            updated_after: "2025-01-01T00:00:00Z".parse().expect("date"),
            updated_before: None,
            query: None,
            forum_ids: Vec::new(),
            category_ids: Vec::new(),
            status_ids: Vec::new(),
            label_ids: Vec::new(),
            states: Vec::new(),
            sort: None,
            detail_level: DetailLevel::Full,
            per_page: 11,
            continuation: None,
        })
        .await
        .expect_err("full page must stay bounded");
    assert!(error.to_string().contains("per_page <= 10"));
}

#[tokio::test]
async fn collect_ideas_summary_returns_ok_with_ideas() {
    let app = App::new(Arc::new(support::FakeUserVoiceApi::complete()), 2);
    let result = app
        .collect_ideas(CollectIdeasParams {
            updated_after: "2025-01-01T00:00:00Z".parse().expect("date"),
            updated_before: None,
            query: None,
            forum_ids: Vec::new(),
            category_ids: Vec::new(),
            status_ids: Vec::new(),
            label_ids: Vec::new(),
            states: Vec::new(),
            sort: None,
            detail_level: DetailLevel::Summary,
            per_page: 100,
            continuation: None,
        })
        .await
        .expect("collect");
    assert_eq!(result.status, OutcomeStatus::Ok);
    assert!(!result.data.ideas.is_empty());
}

#[tokio::test]
async fn collect_ideas_with_forum_filter() {
    let app = App::new(Arc::new(support::FakeUserVoiceApi::complete()), 2);
    let result = app
        .collect_ideas(CollectIdeasParams {
            updated_after: "2025-01-01T00:00:00Z".parse().expect("date"),
            updated_before: None,
            query: None,
            forum_ids: vec![uservoice_client::ForumId::new(11)],
            category_ids: Vec::new(),
            status_ids: Vec::new(),
            label_ids: Vec::new(),
            states: Vec::new(),
            sort: None,
            detail_level: DetailLevel::Summary,
            per_page: 100,
            continuation: None,
        })
        .await
        .expect("collect with forum filter");
    assert_eq!(result.status, OutcomeStatus::Ok);
}

#[tokio::test]
async fn collect_ideas_full_detail_with_partial_context() {
    let api = support::FakeUserVoiceApi::with_failed_section("notes");
    let app = App::new(Arc::new(api), 2);
    let result = app
        .collect_ideas(CollectIdeasParams {
            updated_after: "2025-01-01T00:00:00Z".parse().expect("date"),
            updated_before: None,
            query: None,
            forum_ids: Vec::new(),
            category_ids: Vec::new(),
            status_ids: Vec::new(),
            label_ids: Vec::new(),
            states: Vec::new(),
            sort: None,
            detail_level: DetailLevel::Full,
            per_page: 10,
            continuation: None,
        })
        .await
        .expect("collect full detail");
    assert_eq!(result.status, OutcomeStatus::Partial);
    assert!(!result.data.ideas.is_empty());
}

#[tokio::test]
async fn collect_ideas_with_all_filters() {
    let app = App::new(Arc::new(support::FakeUserVoiceApi::complete()), 2);
    let result = app
        .collect_ideas(CollectIdeasParams {
            updated_after: "2025-01-01T00:00:00Z".parse().expect("date"),
            updated_before: None,
            query: Some("test".to_owned()),
            forum_ids: vec![uservoice_client::ForumId::new(11)],
            category_ids: vec![uservoice_client::CategoryId::new(12)],
            status_ids: vec![uservoice_client::StatusId::new(13)],
            label_ids: vec![uservoice_client::LabelId::new(14)],
            states: vec!["open".to_owned()],
            sort: Some("-updated_at".to_owned()),
            detail_level: DetailLevel::Summary,
            per_page: 100,
            continuation: None,
        })
        .await
        .expect("collect with all filters");
    assert_eq!(result.status, OutcomeStatus::Ok);
}

#[tokio::test]
async fn collect_ideas_empty_results_returns_no_results() {
    use uservoice_client::{SearchSuggestionsRequest, UserVoiceApi};

    #[derive(Clone)]
    struct EmptyApi;

    impl UserVoiceApi for EmptyApi {
        async fn current_user(
            &self,
        ) -> Result<
            uservoice_client::ApiResponse<uservoice_client::User>,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn workspace_catalog(
            &self,
        ) -> Result<
            uservoice_client::ApiResponse<uservoice_client::WorkspaceCatalog>,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn search_suggestions(
            &self,
            _req: SearchSuggestionsRequest,
        ) -> Result<
            uservoice_client::ApiResponse<uservoice_client::SuggestionPage>,
            uservoice_client::ClientError,
        > {
            Ok(uservoice_client::ApiResponse {
                data: uservoice_client::SuggestionPage {
                    items: Vec::new(),
                    pagination: uservoice_client::Pagination {
                        per_page: 100,
                        total_records: Some(0),
                        next: None,
                        has_more: false,
                        limit_hit: false,
                    },
                },
                rate_limit: None,
                continuation: None,
            })
        }
        async fn suggestion(
            &self,
            _id: IdeaId,
        ) -> Result<
            uservoice_client::ApiResponse<uservoice_client::Suggestion>,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn comments(
            &self,
            _id: IdeaId,
            _pp: u32,
        ) -> Result<
            uservoice_client::ApiResponse<uservoice_client::ContextPage<uservoice_client::Comment>>,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn notes(
            &self,
            _id: IdeaId,
            _pp: u32,
        ) -> Result<
            uservoice_client::ApiResponse<uservoice_client::ContextPage<uservoice_client::Note>>,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn supporters(
            &self,
            _id: IdeaId,
            _pp: u32,
        ) -> Result<
            uservoice_client::ApiResponse<
                uservoice_client::ContextPage<uservoice_client::Supporter>,
            >,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn feedback_records(
            &self,
            _id: IdeaId,
            _pp: u32,
        ) -> Result<
            uservoice_client::ApiResponse<
                uservoice_client::ContextPage<uservoice_client::FeedbackRecord>,
            >,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn status_updates(
            &self,
            _id: IdeaId,
            _pp: u32,
        ) -> Result<
            uservoice_client::ApiResponse<
                uservoice_client::ContextPage<uservoice_client::StatusUpdate>,
            >,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn activity(
            &self,
            _id: IdeaId,
            _pp: u32,
        ) -> Result<
            uservoice_client::ApiResponse<
                uservoice_client::ContextPage<uservoice_client::ActivityEntry>,
            >,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn update_suggestion(
            &self,
            _id: IdeaId,
            _req: uservoice_client::UpdateSuggestionRequest,
        ) -> Result<
            uservoice_client::ApiResponse<uservoice_client::Suggestion>,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn attach_label(
            &self,
            _id: IdeaId,
            _lid: uservoice_client::LabelId,
        ) -> Result<uservoice_client::ApiResponse<()>, uservoice_client::ClientError> {
            unimplemented!()
        }
        async fn detach_label(
            &self,
            _id: IdeaId,
            _lid: uservoice_client::LabelId,
        ) -> Result<uservoice_client::ApiResponse<()>, uservoice_client::ClientError> {
            unimplemented!()
        }
        async fn create_note(
            &self,
            _id: IdeaId,
            _body: String,
        ) -> Result<
            uservoice_client::ApiResponse<uservoice_client::Note>,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn create_status_update(
            &self,
            _req: uservoice_client::CreateStatusUpdateRequest,
        ) -> Result<
            uservoice_client::ApiResponse<uservoice_client::StatusUpdate>,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
        async fn create_feedback_record(
            &self,
            _req: uservoice_client::CreateFeedbackRecordRequest,
        ) -> Result<
            uservoice_client::ApiResponse<uservoice_client::FeedbackRecord>,
            uservoice_client::ClientError,
        > {
            unimplemented!()
        }
    }

    let app = App::new(Arc::new(EmptyApi), 2);
    let result = app
        .collect_ideas(CollectIdeasParams {
            updated_after: "2025-01-01T00:00:00Z".parse().expect("date"),
            updated_before: None,
            query: None,
            forum_ids: Vec::new(),
            category_ids: Vec::new(),
            status_ids: Vec::new(),
            label_ids: Vec::new(),
            states: Vec::new(),
            sort: None,
            detail_level: DetailLevel::Summary,
            per_page: 100,
            continuation: None,
        })
        .await
        .expect("collect");
    assert_eq!(result.status, OutcomeStatus::NoResults);
}
