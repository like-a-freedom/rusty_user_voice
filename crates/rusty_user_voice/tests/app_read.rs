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
