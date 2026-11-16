mod support;

use std::collections::BTreeMap;
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
async fn a_second_apply_under_a_different_key_is_rejected_by_the_freshness_check() {
    // The at-most-once guarantee belongs to the Idempotency Key, not to the
    // Change ID. Replaying the same key writes once. A second apply under a
    // new key is stopped by the freshness check, because the first write
    // moved the Idea. The skill corpus documents exactly this.
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

    app.update_idea(UpdateIdeaParams::apply_from_preview(
        &preview_params,
        change_id,
        "apply",
        "agent-run-20260724-key-a",
    ))
    .await
    .expect("first apply");
    let writes_after_first = api.recorded_writes().len();

    // The upstream write moved updated_at, as a real API does.
    api.advance_suggestion_updated_at();

    let second = app
        .update_idea(UpdateIdeaParams::apply_from_preview(
            &preview_params,
            change_id,
            "apply",
            "agent-run-20260724-key-b",
        ))
        .await
        .expect_err("a second apply under a new key must be rejected");
    assert!(second.to_string().contains("changed since preview"));
    assert_eq!(
        api.recorded_writes().len(),
        writes_after_first,
        "the rejected apply must not write"
    );
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
            custom_fields: Default::default(),
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
            custom_fields: Default::default(),
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
            custom_fields: Default::default(),
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

fn update_with_custom_fields(
    idea_id: IdeaId,
    pairs: &[(&str, serde_json::Value)],
) -> UpdateIdeaParams {
    let mut params = UpdateIdeaParams::preview(idea_id, None, Vec::new());
    for (key, value) in pairs {
        params
            .custom_fields
            .insert((*key).to_owned(), value.clone());
    }
    params
}

#[tokio::test]
async fn preview_shows_a_custom_field_before_and_after() {
    let api = support::FakeUserVoiceApi::with_custom_fields(BTreeMap::from([(
        "cf_tfs_link".to_owned(),
        serde_json::json!("https://old.example.com/1"),
    )]));
    let directory = TempDir::new().expect("state directory");
    let app = App::new(Arc::new(api.clone()), 2)
        .with_ledger(Arc::new(DiskChangeLedger::new(
            directory.path().join("ledger"),
        )))
        .with_clock(Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 7, 24, 10, 0, 0).unwrap(),
        )));

    let params = update_with_custom_fields(
        IdeaId::new(101),
        &[(
            "cf_tfs_link",
            serde_json::json!("https://new.example.com/2"),
        )],
    );
    let result = app.update_idea(params).await.expect("preview");

    let change = result
        .data
        .changes
        .iter()
        .find(|change| change.field == "cf_tfs_link")
        .expect("the custom field must appear in the diff");
    assert_eq!(change.before.as_deref(), Some("https://old.example.com/1"));
    assert_eq!(change.after.as_deref(), Some("https://new.example.com/2"));
    assert!(api.recorded_writes().is_empty());
}

#[tokio::test]
async fn a_custom_field_alone_is_a_valid_change() {
    let (_directory, _api, app) = mutation_app();
    let params = update_with_custom_fields(
        IdeaId::new(101),
        &[("cf_reviewed", serde_json::json!(true))],
    );
    let result = app
        .update_idea(params)
        .await
        .expect("a custom field is a requested change");
    assert_eq!(result.data.changes.len(), 1);
    assert_eq!(result.data.changes[0].field, "cf_reviewed");
    assert_eq!(result.data.changes[0].before, None);
    assert_eq!(result.data.changes[0].after.as_deref(), Some("true"));
}

#[tokio::test]
async fn apply_sends_custom_field_values() {
    let (_directory, api, app) = mutation_app();
    let preview_params = update_with_custom_fields(
        IdeaId::new(101),
        &[
            (
                "cf_tfs_link",
                serde_json::json!("https://example.com/tfs/9"),
            ),
            ("cf_reviewed", serde_json::json!(true)),
        ],
    );
    let preview = app
        .update_idea(preview_params.clone())
        .await
        .expect("preview");
    app.update_idea(UpdateIdeaParams::apply_from_preview(
        &preview_params,
        preview.data.change_id.expect("change id"),
        "apply",
        "custom-fields-apply-1",
    ))
    .await
    .expect("apply");

    let sent = api.recorded_updates().pop().expect("one update call");
    assert_eq!(
        sent.custom_fields.get("cf_tfs_link"),
        Some(&serde_json::json!("https://example.com/tfs/9"))
    );
    assert_eq!(
        sent.custom_fields.get("cf_reviewed"),
        Some(&serde_json::json!(true))
    );
    assert!(sent.title.is_none());
}

#[tokio::test]
async fn preview_rejects_a_custom_field_key_the_api_would_ignore() {
    // MCP callers send the map directly, without the CLI parser, so the app
    // is the choke point: the API ignores unknown keys and would report a
    // successful update that wrote nothing.
    let (_directory, api, app) = mutation_app();
    let params =
        update_with_custom_fields(IdeaId::new(101), &[("tfs_link", serde_json::json!("x"))]);
    let error = app
        .update_idea(params)
        .await
        .expect_err("the API would silently ignore this key");
    assert!(error.to_string().contains("cf_"), "{error}");
    assert!(api.recorded_writes().is_empty());
}

#[tokio::test]
async fn apply_rejects_custom_fields_that_differ_from_the_preview() {
    let (_directory, api, app) = mutation_app();
    let preview_params = update_with_custom_fields(
        IdeaId::new(101),
        &[("cf_tfs_link", serde_json::json!("one"))],
    );
    let preview = app
        .update_idea(preview_params.clone())
        .await
        .expect("preview");

    let mut apply = UpdateIdeaParams::apply_from_preview(
        &preview_params,
        preview.data.change_id.expect("change id"),
        "apply",
        "custom-fields-apply-2",
    );
    apply
        .custom_fields
        .insert("cf_tfs_link".to_owned(), serde_json::json!("two"));
    let error = app
        .update_idea(apply)
        .await
        .expect_err("a different custom field value is a different payload");
    assert!(
        error
            .to_string()
            .contains("apply payload does not match the preview")
    );
    assert!(api.recorded_writes().is_empty());
}
