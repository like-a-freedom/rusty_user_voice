mod support;

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use rusty_user_voice::{
    App, DiskChangeLedger, FixedClock, MutationMode, OutcomeStatus, UpdateIdeaParams,
};
use tempfile::TempDir;
use uservoice_client::{IdeaId, LabelId};

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
