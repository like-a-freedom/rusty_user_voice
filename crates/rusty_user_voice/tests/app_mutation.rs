mod support;

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use rusty_user_voice::{
    App, ChangeId, DiskChangeLedger, FeedbackChannel, FixedClock, MutationMode, OutcomeStatus,
    PublishIdeaUpdateParams, RecordFeedbackParams, UpdateIdeaParams,
};
use tempfile::TempDir;
use uservoice_client::{IdeaId, LabelId, StatusId};

fn mutation_app() -> (
    TempDir,
    support::FakeUserVoiceApi,
    App<support::FakeUserVoiceApi>,
) {
    let directory = TempDir::new().expect("state directory");
    let api = support::FakeUserVoiceApi::complete();
    let app = App::new(Arc::new(api.clone()), 2)
        .with_ledger(Arc::new(DiskChangeLedger::new(
            directory.path().join("ledger"),
        )))
        .with_clock(Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 7, 24, 10, 0, 0).unwrap(),
        )));
    (directory, api, app)
}

#[tokio::test]
async fn preview_persists_plan_without_remote_writes() {
    let (_directory, api, app) = mutation_app();
    let result = app
        .update_idea(UpdateIdeaParams::preview(
            IdeaId::new(101),
            Some("Clearer export title".to_owned()),
            vec![LabelId::new(14)],
        ))
        .await
        .expect("preview");
    assert_eq!(result.status, OutcomeStatus::Ok);
    assert_eq!(result.data.mode, MutationMode::Preview);
    assert!(result.data.change_id.is_some());
    assert!(api.recorded_writes().is_empty());
}

#[tokio::test]
async fn apply_executes_stored_operations_once_and_replays_result() {
    let (_directory, api, app) = mutation_app();
    let preview_params = UpdateIdeaParams::preview(
        IdeaId::new(101),
        Some("Clearer export title".to_owned()),
        vec![LabelId::new(14)],
    );
    let preview = app
        .update_idea(preview_params.clone())
        .await
        .expect("preview");
    let change_id = preview.data.change_id.expect("change id");
    let apply = UpdateIdeaParams::apply_from_preview(
        &preview_params,
        change_id,
        "apply",
        "agent-run-20260724-001",
    );
    let first = app.update_idea(apply.clone()).await.expect("first apply");
    let second = app.update_idea(apply).await.expect("idempotent replay");
    assert_eq!(
        first.data.applied_operations,
        second.data.applied_operations
    );
    assert_eq!(api.recorded_writes().len(), 2);
}

#[tokio::test]
async fn apply_rejects_stale_preview_before_any_write() {
    let (_directory, api, app) = mutation_app();
    let preview_params = UpdateIdeaParams::preview(
        IdeaId::new(101),
        Some("Clearer export title".to_owned()),
        Vec::new(),
    );
    let preview = app
        .update_idea(preview_params.clone())
        .await
        .expect("preview");
    api.advance_suggestion_updated_at();
    let error = app
        .update_idea(UpdateIdeaParams::apply_from_preview(
            &preview_params,
            preview.data.change_id.expect("change id"),
            "apply",
            "agent-run-20260724-002",
        ))
        .await
        .expect_err("stale preview");
    assert!(error.to_string().contains("changed since preview"));
    assert!(api.recorded_writes().is_empty());
}

#[tokio::test]
async fn partial_write_is_completed_in_ledger_and_never_compensated_or_retried() {
    let (_directory, api, app) = mutation_app();
    api.fail_write_number(2);
    let preview_params = UpdateIdeaParams::preview(
        IdeaId::new(101),
        Some("Clearer export title".to_owned()),
        vec![LabelId::new(14)],
    );
    let preview = app
        .update_idea(preview_params.clone())
        .await
        .expect("preview");
    let apply = UpdateIdeaParams::apply_from_preview(
        &preview_params,
        preview.data.change_id.expect("change id"),
        "apply",
        "agent-run-20260724-partial",
    );

    let first = app
        .update_idea(apply.clone())
        .await
        .expect("partial result");
    assert_eq!(first.status, OutcomeStatus::Partial);
    assert_eq!(
        first.data.applied_operations[0].operation,
        "update_suggestion"
    );
    assert_eq!(first.data.applied_operations[0].outcome, "applied");
    assert_eq!(first.data.applied_operations[1].operation, "attach_label");
    assert!(
        first.data.applied_operations[1]
            .outcome
            .starts_with("failed:")
    );
    assert_eq!(api.recorded_writes().len(), 1);

    let replay = app.update_idea(apply).await.expect("partial replay");
    assert_eq!(
        replay.data.applied_operations,
        first.data.applied_operations
    );
    assert_eq!(api.recorded_writes().len(), 1);
}

#[tokio::test]
async fn publish_preview_creates_status_update() {
    let (_directory, _api, app) = mutation_app();
    let result = app
        .publish_idea_update(PublishIdeaUpdateParams::preview(
            IdeaId::new(101),
            uservoice_client::StatusId::new(5361859),
            "Shipped!",
            false,
        ))
        .await
        .expect("preview");
    assert_eq!(result.status, OutcomeStatus::Ok);
    assert!(result.data.change_id.is_some());
}

#[tokio::test]
async fn publish_preview_rejects_change_id() {
    let (_directory, _api, app) = mutation_app();
    let mut params = PublishIdeaUpdateParams::preview(
        IdeaId::new(101),
        uservoice_client::StatusId::new(5361859),
        "Shipped!",
        false,
    );
    params.change_id = Some(ChangeId::new());
    let err = app
        .publish_idea_update(params)
        .await
        .expect_err("should fail");
    assert!(err.to_string().contains("preview does not accept"));
}

#[tokio::test]
async fn record_feedback_creates_feedback_record() {
    let (_directory, _api, app) = mutation_app();
    let result = app
        .record_feedback(RecordFeedbackParams {
            body: "Great feature!".to_owned(),
            channel: FeedbackChannel::Api,
            idea_id: Some(IdeaId::new(101)),
            user_id: None,
            user_email: None,
            source_url: None,
            source_guid: None,
            created_at: None,
            idempotency_key: "test-key-1".to_owned(),
        })
        .await
        .expect("record feedback");
    assert_eq!(result.status, OutcomeStatus::Ok);
}

#[tokio::test]
async fn publish_apply_creates_status_update() {
    let (_directory, _api, app) = mutation_app();
    let preview = app
        .publish_idea_update(PublishIdeaUpdateParams::preview(
            IdeaId::new(101),
            StatusId::new(5361859),
            "Shipped!",
            false,
        ))
        .await
        .expect("preview");
    let apply = PublishIdeaUpdateParams::apply_from_preview(
        &preview.data,
        StatusId::new(5361859),
        "Shipped!",
        false,
        "apply",
        "key-1",
    );
    let result = app.publish_idea_update(apply).await.expect("apply");
    assert_eq!(result.status, OutcomeStatus::Ok);
}
#[tokio::test]
async fn record_feedback_rejects_empty_body() {
    let (_directory, _api, app) = mutation_app();
    let err = app
        .record_feedback(RecordFeedbackParams {
            body: "  ".to_owned(),
            channel: FeedbackChannel::Api,
            idea_id: Some(IdeaId::new(101)),
            user_id: None,
            user_email: None,
            source_url: None,
            source_guid: None,
            created_at: None,
            idempotency_key: "key-1".to_owned(),
        })
        .await
        .expect_err("should fail");
    assert!(err.to_string().contains("empty"));
}

#[tokio::test]
async fn record_feedback_rejects_no_target() {
    let (_directory, _api, app) = mutation_app();
    let err = app
        .record_feedback(RecordFeedbackParams {
            body: "test".to_owned(),
            channel: FeedbackChannel::Api,
            idea_id: None,
            user_id: None,
            user_email: None,
            source_url: None,
            source_guid: None,
            created_at: None,
            idempotency_key: "key-1".to_owned(),
        })
        .await
        .expect_err("should fail");
    assert!(err.to_string().contains("at least one"));
}

#[tokio::test]
async fn preview_with_body_and_internal_note_records_changes() {
    let (_directory, _api, app) = mutation_app();
    let result = app
        .update_idea(UpdateIdeaParams {
            mode: MutationMode::Preview,
            idea_id: IdeaId::new(101),
            title: None,
            body: Some("New body".to_owned()),
            forum_id: None,
            category_id: None,
            assigned_owner_id: None,
            add_label_ids: Vec::new(),
            remove_label_ids: Vec::new(),
            internal_note: Some("Internal note".to_owned()),
            change_id: None,
            confirmation: None,
            idempotency_key: None,
        })
        .await
        .expect("preview");
    assert_eq!(result.status, OutcomeStatus::Ok);
    assert_eq!(result.data.changes.len(), 2);
    assert_eq!(result.data.changes[0].field, "body");
    assert_eq!(result.data.changes[1].field, "internal_note");
}

#[tokio::test]
async fn preview_rejects_when_no_changes() {
    let (_directory, _api, app) = mutation_app();
    let err = app
        .update_idea(UpdateIdeaParams {
            mode: MutationMode::Preview,
            idea_id: IdeaId::new(101),
            title: None,
            body: None,
            forum_id: None,
            category_id: None,
            assigned_owner_id: None,
            add_label_ids: Vec::new(),
            remove_label_ids: Vec::new(),
            internal_note: None,
            change_id: None,
            confirmation: None,
            idempotency_key: None,
        })
        .await
        .expect_err("should fail");
    assert!(err.to_string().contains("at least one"));
}

#[tokio::test]
async fn preview_rejects_change_id() {
    let (_directory, _api, app) = mutation_app();
    let err = app
        .update_idea(UpdateIdeaParams {
            mode: MutationMode::Preview,
            idea_id: IdeaId::new(101),
            title: Some("title".to_owned()),
            body: None,
            forum_id: None,
            category_id: None,
            assigned_owner_id: None,
            add_label_ids: Vec::new(),
            remove_label_ids: Vec::new(),
            internal_note: None,
            change_id: Some(ChangeId::new()),
            confirmation: None,
            idempotency_key: None,
        })
        .await
        .expect_err("should fail");
    assert!(err.to_string().contains("preview does not accept"));
}

#[tokio::test]
async fn apply_rejects_missing_confirmation() {
    let (_directory, _api, app) = mutation_app();
    let preview = app
        .update_idea(UpdateIdeaParams::preview(
            IdeaId::new(101),
            Some("title".to_owned()),
            vec![],
        ))
        .await
        .expect("preview");
    let err = app
        .update_idea(UpdateIdeaParams::apply_from_preview(
            &UpdateIdeaParams::preview(IdeaId::new(101), Some("title".to_owned()), vec![]),
            preview.data.change_id.unwrap(),
            "wrong",
            "key-1",
        ))
        .await
        .expect_err("should fail");
    assert!(err.to_string().contains("confirmation"));
}
