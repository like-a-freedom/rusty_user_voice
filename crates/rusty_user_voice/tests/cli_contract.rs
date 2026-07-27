mod support;

use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use clap::Parser;
use predicates::prelude::PredicateBooleanExt;
use rusty_user_voice::{
    App, CheckpointStore, Cli, SearchIdeasParams,
    cli_runner::{run_cli, run_cli_with_stderr},
};
use uservoice_client::{ApiResponse, UserVoiceApi};

#[derive(Clone)]
struct CapturingApi {
    inner: support::FakeUserVoiceApi,
    captured: Arc<Mutex<Option<SearchIdeasParams>>>,
}

impl UserVoiceApi for CapturingApi {
    async fn search_suggestions(
        &self,
        request: uservoice_client::SearchSuggestionsRequest,
    ) -> Result<ApiResponse<uservoice_client::SuggestionPage>, uservoice_client::ClientError> {
        *self.captured.lock().expect("lock") = Some(SearchIdeasParams {
            query: request.query.clone(),
            updated_after: request.updated_after,
            updated_before: request.updated_before,
            forum_ids: request.forum_ids.clone(),
            category_ids: request.category_ids.clone(),
            status_ids: request.status_ids.clone(),
            label_ids: request.label_ids.clone(),
            states: request.states.clone(),
            sort: request.sort.clone(),
            per_page: request.per_page,
            continuation: request.continuation.clone(),
        });
        self.inner.search_suggestions(request).await
    }

    async fn current_user(
        &self,
    ) -> Result<ApiResponse<uservoice_client::User>, uservoice_client::ClientError> {
        self.inner.current_user().await
    }
    async fn workspace_catalog(
        &self,
    ) -> Result<ApiResponse<uservoice_client::WorkspaceCatalog>, uservoice_client::ClientError>
    {
        self.inner.workspace_catalog().await
    }
    async fn suggestion(
        &self,
        id: uservoice_client::IdeaId,
    ) -> Result<ApiResponse<uservoice_client::Suggestion>, uservoice_client::ClientError> {
        self.inner.suggestion(id).await
    }
    async fn comments(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::Comment>>,
        uservoice_client::ClientError,
    > {
        self.inner.comments(id, pp).await
    }
    async fn notes(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::Note>>,
        uservoice_client::ClientError,
    > {
        self.inner.notes(id, pp).await
    }
    async fn supporters(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::Supporter>>,
        uservoice_client::ClientError,
    > {
        self.inner.supporters(id, pp).await
    }
    async fn feedback_records(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::FeedbackRecord>>,
        uservoice_client::ClientError,
    > {
        self.inner.feedback_records(id, pp).await
    }
    async fn status_updates(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::StatusUpdate>>,
        uservoice_client::ClientError,
    > {
        self.inner.status_updates(id, pp).await
    }
    async fn activity(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::ActivityEntry>>,
        uservoice_client::ClientError,
    > {
        self.inner.activity(id, pp).await
    }
    async fn update_suggestion(
        &self,
        id: uservoice_client::IdeaId,
        req: uservoice_client::UpdateSuggestionRequest,
    ) -> Result<ApiResponse<uservoice_client::Suggestion>, uservoice_client::ClientError> {
        self.inner.update_suggestion(id, req).await
    }
    async fn attach_label(
        &self,
        id: uservoice_client::IdeaId,
        lid: uservoice_client::LabelId,
    ) -> Result<ApiResponse<()>, uservoice_client::ClientError> {
        self.inner.attach_label(id, lid).await
    }
    async fn detach_label(
        &self,
        id: uservoice_client::IdeaId,
        lid: uservoice_client::LabelId,
    ) -> Result<ApiResponse<()>, uservoice_client::ClientError> {
        self.inner.detach_label(id, lid).await
    }
    async fn create_note(
        &self,
        id: uservoice_client::IdeaId,
        body: String,
    ) -> Result<ApiResponse<uservoice_client::Note>, uservoice_client::ClientError> {
        self.inner.create_note(id, body).await
    }
    async fn create_status_update(
        &self,
        req: uservoice_client::CreateStatusUpdateRequest,
    ) -> Result<ApiResponse<uservoice_client::StatusUpdate>, uservoice_client::ClientError> {
        self.inner.create_status_update(req).await
    }
    async fn create_feedback_record(
        &self,
        req: uservoice_client::CreateFeedbackRecordRequest,
    ) -> Result<ApiResponse<uservoice_client::FeedbackRecord>, uservoice_client::ClientError> {
        self.inner.create_feedback_record(req).await
    }
}

#[test]
fn api_key_is_not_a_cli_argument() {
    let error = Cli::try_parse_from([
        "uservoice-cli",
        "--api-key",
        "must-not-be-accepted",
        "workspace",
    ])
    .expect_err("secret argument must be rejected");
    assert!(
        error
            .to_string()
            .contains("unexpected argument '--api-key'")
    );
}

#[test]
fn collect_parses_ndjson_and_cursor() {
    let cli = Cli::parse_from([
        "uservoice-cli",
        "--output",
        "ndjson",
        "ideas",
        "collect",
        "--updated-after",
        "2025-01-01T00:00:00Z",
        "--cursor",
        "cursor-2",
    ]);
    assert_eq!(cli.output, rusty_user_voice::OutputMode::Ndjson);
    assert!(matches!(
        cli.command,
        rusty_user_voice::Command::Ideas { .. }
    ));
}

#[test]
fn settings_debug_output_redacts_api_key() {
    let settings = rusty_user_voice::Settings::from_values(
        "https://acme.uservoice.com",
        "fixture-super-secret",
        None,
        Some(2),
    )
    .expect("settings");
    let rendered = format!("{settings:?}");
    assert!(!rendered.contains("fixture-super-secret"));
    assert!(rendered.contains("[REDACTED]"));
}

#[test]
fn help_output_does_not_leak_api_key_flag() {
    assert_cmd::Command::cargo_bin("uservoice-cli")
        .expect("binary")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("USERVOICE_API_KEY").not())
        .stdout(predicates::str::contains("--api-key").not());
}

struct FailOnFlush(Vec<u8>);

impl Write for FailOnFlush {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("fixture consumer disconnected"))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn errors_never_emit_token_email_or_feedback_body() {
    const TOKEN: &str = "fixture-secret-do-not-log";
    const EMAIL: &str = "private-person@example.invalid";
    const BODY: &str = "private feedback body content";

    for (status, response_body) in [
        (401, format!(r#"{{"errors":{{"error":"{TOKEN}"}}}}"#)),
        (422, format!(r#"{{"errors":{{"email":"{EMAIL}"}}}}"#)),
        (429, format!(r#"{{"errors":{{"body":"{BODY}"}}}}"#)),
        (200, format!("{{not-json:{EMAIL}:{BODY}:{TOKEN}}}")),
    ] {
        let server = wiremock::MockServer::start().await;
        let mut template =
            wiremock::ResponseTemplate::new(status).set_body_raw(response_body, "application/json");
        if status == 429 {
            template = template
                .insert_header("X-Rate-Limit-Limit", "60")
                .insert_header("X-Rate-Limit-Remaining", "0")
                .insert_header("X-Rate-Limit-Reset", "4102444800")
                .insert_header("Retry-After", "4102444800");
        }
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v2/admin/users/current"))
            .respond_with(template)
            .expect(1)
            .mount(&server)
            .await;

        let state = tempfile::tempdir().expect("state");
        let output = assert_cmd::Command::cargo_bin("uservoice-cli")
            .expect("binary")
            .env("USERVOICE_BASE_URL", server.uri())
            .env("USERVOICE_API_KEY", TOKEN)
            .env("USERVOICE_STATE_DIR", state.path())
            .args(["--output", "json", "workspace"])
            .output()
            .expect("run CLI");

        let rendered = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        for forbidden in [TOKEN, EMAIL, BODY] {
            assert!(
                !rendered.contains(forbidden),
                "process output leaked fixture data ({status}, {forbidden:?}):\n{rendered}",
            );
        }
    }
}

#[tokio::test]
async fn collect_commits_checkpoint_only_after_output_flush() {
    let directory = tempfile::tempdir().expect("state directory");
    let checkpoints = CheckpointStore::new(directory.path().join("checkpoints"));
    let app = App::new(Arc::new(support::FakeUserVoiceApi::complete()), 2);
    let args = [
        "uservoice-cli",
        "--output",
        "ndjson",
        "ideas",
        "collect",
        "--updated-after",
        "2025-01-01T00:00:00Z",
        "--checkpoint-name",
        "flush-order",
    ];

    let error = run_cli(
        &app,
        &checkpoints,
        Cli::try_parse_from(args).expect("CLI"),
        &mut FailOnFlush(Vec::new()),
    )
    .await
    .expect_err("flush must fail");
    assert!(error.to_string().contains("state I/O"));
    assert!(
        checkpoints
            .load("flush-order")
            .await
            .expect("load")
            .is_none()
    );

    let mut output = Vec::new();
    run_cli(
        &app,
        &checkpoints,
        Cli::try_parse_from(args).expect("CLI"),
        &mut output,
    )
    .await
    .expect("collect");
    assert!(
        checkpoints
            .load("flush-order")
            .await
            .expect("load")
            .expect("checkpoint")
            .completed
    );
}

#[test]
fn collect_parses_taxonomy_filters() {
    let cli = Cli::parse_from([
        "uservoice-cli",
        "--output",
        "ndjson",
        "ideas",
        "collect",
        "--updated-after",
        "2025-01-01T00:00:00Z",
        "--query",
        "osmp",
        "--forum-ids",
        "11,12",
        "--category-ids",
        "13",
        "--status-ids",
        "14",
        "--label-ids",
        "15,16",
        "--states",
        "open,closed",
        "--sort",
        "votes",
    ]);
    assert_eq!(cli.output, rusty_user_voice::OutputMode::Ndjson);
    match cli.command {
        rusty_user_voice::Command::Ideas { command } => match *command {
            rusty_user_voice::IdeasCommand::Collect(args) => {
                assert_eq!(args.query.as_deref(), Some("osmp"));
                assert_eq!(args.forum_ids, vec![11, 12]);
                assert_eq!(args.category_ids, vec![13]);
                assert_eq!(args.status_ids, vec![14]);
                assert_eq!(args.label_ids, vec![15, 16]);
                assert_eq!(args.states, vec!["open", "closed"]);
                assert_eq!(args.sort.as_deref(), Some("votes"));
            }
            _ => panic!("expected Collect command"),
        },
        _ => panic!("expected Ideas command"),
    }
}

#[tokio::test]
async fn collect_forwards_taxonomy_filters_to_api() {
    let directory = tempfile::tempdir().expect("state directory");
    let checkpoints = CheckpointStore::new(directory.path().join("checkpoints"));
    let fake = support::FakeUserVoiceApi::complete();
    let captured = Arc::new(std::sync::Mutex::new(
        None::<rusty_user_voice::SearchIdeasParams>,
    ));
    let captor = CapturingApi {
        inner: fake,
        captured: captured.clone(),
    };
    let app = App::new(Arc::new(captor), 2);

    let mut output = Vec::new();
    run_cli(
        &app,
        &checkpoints,
        Cli::try_parse_from([
            "uservoice-cli",
            "--output",
            "ndjson",
            "ideas",
            "collect",
            "--updated-after",
            "2025-01-01T00:00:00Z",
            "--query",
            "osmp",
            "--forum-ids",
            "11,12",
            "--category-ids",
            "13",
            "--status-ids",
            "14",
            "--label-ids",
            "15",
            "--states",
            "open",
            "--sort",
            "votes",
        ])
        .expect("CLI"),
        &mut output,
    )
    .await
    .expect("collect with filters");

    let params = captured
        .lock()
        .expect("captured lock")
        .take()
        .expect("was set");
    assert_eq!(params.query.as_deref(), Some("osmp"));
    assert_eq!(
        params.forum_ids,
        vec![
            uservoice_client::ForumId::new(11),
            uservoice_client::ForumId::new(12),
        ]
    );
    assert_eq!(
        params.category_ids,
        vec![uservoice_client::CategoryId::new(13)]
    );
    assert_eq!(params.status_ids, vec![uservoice_client::StatusId::new(14)]);
    assert_eq!(params.label_ids, vec![uservoice_client::LabelId::new(15)]);
    assert_eq!(params.states, vec!["open"]);
    assert_eq!(params.sort.as_deref(), Some("votes"));
}

fn test_response<T>(data: T) -> ApiResponse<T> {
    ApiResponse {
        data,
        rate_limit: None,
        continuation: None,
    }
}

// -- Commit 2: --all auto-pagination helpers and tests --

#[derive(Clone)]
struct TwoPageApi;

impl UserVoiceApi for TwoPageApi {
    async fn search_suggestions(
        &self,
        request: uservoice_client::SearchSuggestionsRequest,
    ) -> Result<ApiResponse<uservoice_client::SuggestionPage>, uservoice_client::ClientError> {
        let is_page_two = matches!(
            request.continuation,
            Some(uservoice_client::Continuation::Cursor { ref cursor }) if cursor == "page2"
        );
        let (id, has_more, next) = if is_page_two {
            (202, false, None)
        } else {
            (
                101,
                true,
                Some(uservoice_client::Continuation::Cursor {
                    cursor: "page2".to_owned(),
                }),
            )
        };
        Ok(test_response(uservoice_client::SuggestionPage {
            items: vec![make_suggestion(id)],
            pagination: uservoice_client::Pagination {
                per_page: 100,
                total_records: Some(2),
                next,
                has_more,
                limit_hit: false,
            },
        }))
    }

    async fn current_user(
        &self,
    ) -> Result<ApiResponse<uservoice_client::User>, uservoice_client::ClientError> {
        self.inner().current_user().await
    }
    async fn workspace_catalog(
        &self,
    ) -> Result<ApiResponse<uservoice_client::WorkspaceCatalog>, uservoice_client::ClientError>
    {
        self.inner().workspace_catalog().await
    }
    async fn suggestion(
        &self,
        id: uservoice_client::IdeaId,
    ) -> Result<ApiResponse<uservoice_client::Suggestion>, uservoice_client::ClientError> {
        self.inner().suggestion(id).await
    }
    async fn comments(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::Comment>>,
        uservoice_client::ClientError,
    > {
        self.inner().comments(id, pp).await
    }
    async fn notes(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::Note>>,
        uservoice_client::ClientError,
    > {
        self.inner().notes(id, pp).await
    }
    async fn supporters(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::Supporter>>,
        uservoice_client::ClientError,
    > {
        self.inner().supporters(id, pp).await
    }
    async fn feedback_records(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::FeedbackRecord>>,
        uservoice_client::ClientError,
    > {
        self.inner().feedback_records(id, pp).await
    }
    async fn status_updates(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::StatusUpdate>>,
        uservoice_client::ClientError,
    > {
        self.inner().status_updates(id, pp).await
    }
    async fn activity(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::ActivityEntry>>,
        uservoice_client::ClientError,
    > {
        self.inner().activity(id, pp).await
    }
    async fn update_suggestion(
        &self,
        id: uservoice_client::IdeaId,
        req: uservoice_client::UpdateSuggestionRequest,
    ) -> Result<ApiResponse<uservoice_client::Suggestion>, uservoice_client::ClientError> {
        self.inner().update_suggestion(id, req).await
    }
    async fn attach_label(
        &self,
        id: uservoice_client::IdeaId,
        lid: uservoice_client::LabelId,
    ) -> Result<ApiResponse<()>, uservoice_client::ClientError> {
        self.inner().attach_label(id, lid).await
    }
    async fn detach_label(
        &self,
        id: uservoice_client::IdeaId,
        lid: uservoice_client::LabelId,
    ) -> Result<ApiResponse<()>, uservoice_client::ClientError> {
        self.inner().detach_label(id, lid).await
    }
    async fn create_note(
        &self,
        id: uservoice_client::IdeaId,
        body: String,
    ) -> Result<ApiResponse<uservoice_client::Note>, uservoice_client::ClientError> {
        self.inner().create_note(id, body).await
    }
    async fn create_status_update(
        &self,
        req: uservoice_client::CreateStatusUpdateRequest,
    ) -> Result<ApiResponse<uservoice_client::StatusUpdate>, uservoice_client::ClientError> {
        self.inner().create_status_update(req).await
    }
    async fn create_feedback_record(
        &self,
        req: uservoice_client::CreateFeedbackRecordRequest,
    ) -> Result<ApiResponse<uservoice_client::FeedbackRecord>, uservoice_client::ClientError> {
        self.inner().create_feedback_record(req).await
    }
}

impl TwoPageApi {
    fn inner(&self) -> support::FakeUserVoiceApi {
        support::FakeUserVoiceApi::complete()
    }
}

fn make_suggestion(id: i64) -> uservoice_client::Suggestion {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "title": format!("Idea {id}"),
        "body": "Body",
        "body_mime_type": "text/plain",
        "channel": "portal",
        "state": "published",
        "created_at": "2025-01-02T03:04:05Z",
        "updated_at": "2025-02-03T04:05:06Z",
        "closed_at": null,
        "comments_count": 0,
        "notes_count": 0,
        "supporters_count": 0,
        "requests_count": 0,
        "portal_url": null,
        "admin_url": null,
        "links": {
            "forum": 11,
            "category": 12,
            "status": 13,
            "labels": [],
            "created_by": 7,
            "assigned_owner": null
        }
    }))
    .expect("valid suggestion fixture")
}

#[derive(Clone)]
struct FailOnSecondPageApi {
    call_count: Arc<Mutex<usize>>,
}

impl UserVoiceApi for FailOnSecondPageApi {
    async fn search_suggestions(
        &self,
        _request: uservoice_client::SearchSuggestionsRequest,
    ) -> Result<ApiResponse<uservoice_client::SuggestionPage>, uservoice_client::ClientError> {
        let mut count = self.call_count.lock().expect("lock");
        *count += 1;
        if *count >= 2 {
            return Err(uservoice_client::ClientError::HttpStatus { status: 500 });
        }
        Ok(test_response(uservoice_client::SuggestionPage {
            items: vec![make_suggestion(101)],
            pagination: uservoice_client::Pagination {
                per_page: 100,
                total_records: Some(2),
                next: Some(uservoice_client::Continuation::Cursor {
                    cursor: "page2".to_owned(),
                }),
                has_more: true,
                limit_hit: false,
            },
        }))
    }

    async fn current_user(
        &self,
    ) -> Result<ApiResponse<uservoice_client::User>, uservoice_client::ClientError> {
        support::FakeUserVoiceApi::complete().current_user().await
    }
    async fn workspace_catalog(
        &self,
    ) -> Result<ApiResponse<uservoice_client::WorkspaceCatalog>, uservoice_client::ClientError>
    {
        support::FakeUserVoiceApi::complete()
            .workspace_catalog()
            .await
    }
    async fn suggestion(
        &self,
        id: uservoice_client::IdeaId,
    ) -> Result<ApiResponse<uservoice_client::Suggestion>, uservoice_client::ClientError> {
        support::FakeUserVoiceApi::complete().suggestion(id).await
    }
    async fn comments(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::Comment>>,
        uservoice_client::ClientError,
    > {
        support::FakeUserVoiceApi::complete().comments(id, pp).await
    }
    async fn notes(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::Note>>,
        uservoice_client::ClientError,
    > {
        support::FakeUserVoiceApi::complete().notes(id, pp).await
    }
    async fn supporters(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::Supporter>>,
        uservoice_client::ClientError,
    > {
        support::FakeUserVoiceApi::complete()
            .supporters(id, pp)
            .await
    }
    async fn feedback_records(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::FeedbackRecord>>,
        uservoice_client::ClientError,
    > {
        support::FakeUserVoiceApi::complete()
            .feedback_records(id, pp)
            .await
    }
    async fn status_updates(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::StatusUpdate>>,
        uservoice_client::ClientError,
    > {
        support::FakeUserVoiceApi::complete()
            .status_updates(id, pp)
            .await
    }
    async fn activity(
        &self,
        id: uservoice_client::IdeaId,
        pp: u32,
    ) -> Result<
        ApiResponse<uservoice_client::ContextPage<uservoice_client::ActivityEntry>>,
        uservoice_client::ClientError,
    > {
        support::FakeUserVoiceApi::complete().activity(id, pp).await
    }
    async fn update_suggestion(
        &self,
        id: uservoice_client::IdeaId,
        req: uservoice_client::UpdateSuggestionRequest,
    ) -> Result<ApiResponse<uservoice_client::Suggestion>, uservoice_client::ClientError> {
        support::FakeUserVoiceApi::complete()
            .update_suggestion(id, req)
            .await
    }
    async fn attach_label(
        &self,
        id: uservoice_client::IdeaId,
        lid: uservoice_client::LabelId,
    ) -> Result<ApiResponse<()>, uservoice_client::ClientError> {
        support::FakeUserVoiceApi::complete()
            .attach_label(id, lid)
            .await
    }
    async fn detach_label(
        &self,
        id: uservoice_client::IdeaId,
        lid: uservoice_client::LabelId,
    ) -> Result<ApiResponse<()>, uservoice_client::ClientError> {
        support::FakeUserVoiceApi::complete()
            .detach_label(id, lid)
            .await
    }
    async fn create_note(
        &self,
        id: uservoice_client::IdeaId,
        body: String,
    ) -> Result<ApiResponse<uservoice_client::Note>, uservoice_client::ClientError> {
        support::FakeUserVoiceApi::complete()
            .create_note(id, body)
            .await
    }
    async fn create_status_update(
        &self,
        req: uservoice_client::CreateStatusUpdateRequest,
    ) -> Result<ApiResponse<uservoice_client::StatusUpdate>, uservoice_client::ClientError> {
        support::FakeUserVoiceApi::complete()
            .create_status_update(req)
            .await
    }
    async fn create_feedback_record(
        &self,
        req: uservoice_client::CreateFeedbackRecordRequest,
    ) -> Result<ApiResponse<uservoice_client::FeedbackRecord>, uservoice_client::ClientError> {
        support::FakeUserVoiceApi::complete()
            .create_feedback_record(req)
            .await
    }
}

#[tokio::test]
async fn search_all_conflicts_with_json_output() {
    let directory = tempfile::tempdir().expect("state directory");
    let checkpoints = CheckpointStore::new(directory.path().join("checkpoints"));
    let app = App::new(Arc::new(support::FakeUserVoiceApi::complete()), 2);
    let mut output = Vec::new();
    let mut stderr_buf = Vec::new();

    let result = run_cli_with_stderr(
        &app,
        &checkpoints,
        Cli::try_parse_from([
            "uservoice-cli",
            "--output",
            "json",
            "ideas",
            "search",
            "--query",
            "test",
            "--all",
        ])
        .expect("CLI parses"),
        &mut output,
        &mut stderr_buf,
    )
    .await;
    let err = result.expect_err("--output json --all must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("--all") && msg.contains("json"),
        "error should mention --all and json: {msg}"
    );
}

#[tokio::test]
async fn search_all_streams_ndjson_across_two_pages() {
    let directory = tempfile::tempdir().expect("state directory");
    let checkpoints = CheckpointStore::new(directory.path().join("checkpoints"));
    let app = App::new(Arc::new(TwoPageApi), 2);
    let mut output = Vec::new();
    let mut stderr_buf = Vec::new();

    let result = run_cli_with_stderr(
        &app,
        &checkpoints,
        Cli::try_parse_from([
            "uservoice-cli",
            "ideas",
            "search",
            "--query",
            "test",
            "--all",
        ])
        .expect("CLI"),
        &mut output,
        &mut stderr_buf,
    )
    .await;
    assert!(result.is_ok(), "--all should succeed: {:?}", result.err());

    let lines: Vec<&[u8]> = output
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(lines.len(), 2, "should have 2 NDJSON lines from 2 pages");

    let first: serde_json::Value = serde_json::from_slice(lines[0]).expect("line 1 valid JSON");
    assert_eq!(first["suggestion"]["id"], 101);

    let second: serde_json::Value = serde_json::from_slice(lines[1]).expect("line 2 valid JSON");
    assert_eq!(second["suggestion"]["id"], 202);

    let stderr = String::from_utf8_lossy(&stderr_buf);
    assert!(
        stderr.contains("next: page2"),
        "stderr should contain cursor hint: {stderr}"
    );
}

#[tokio::test]
async fn search_all_streams_partial_on_failure() {
    let directory = tempfile::tempdir().expect("state directory");
    let checkpoints = CheckpointStore::new(directory.path().join("checkpoints"));
    let api = FailOnSecondPageApi {
        call_count: Arc::new(Mutex::new(0)),
    };
    let app = App::new(Arc::new(api), 2);
    let mut output = Vec::new();
    let mut stderr_buf = Vec::new();

    let result = run_cli_with_stderr(
        &app,
        &checkpoints,
        Cli::try_parse_from([
            "uservoice-cli",
            "ideas",
            "search",
            "--query",
            "test",
            "--all",
        ])
        .expect("CLI"),
        &mut output,
        &mut stderr_buf,
    )
    .await;
    assert!(result.is_err(), "--all should fail on page 2");

    let lines: Vec<&[u8]> = output
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(lines.len(), 1, "should have 1 NDJSON line from page 1");
    let idea: serde_json::Value = serde_json::from_slice(lines[0]).expect("valid JSON");
    assert_eq!(idea["suggestion"]["id"], 101);

    let stderr = String::from_utf8_lossy(&stderr_buf);
    assert!(
        stderr.contains("fail:"),
        "stderr should contain fail message: {stderr}"
    );
    assert!(
        stderr.contains("resume with --cursor"),
        "stderr should suggest resume: {stderr}"
    );
}

#[tokio::test]
async fn search_all_explicit_ndjson_output_works() {
    let directory = tempfile::tempdir().expect("state directory");
    let checkpoints = CheckpointStore::new(directory.path().join("checkpoints"));
    let app = App::new(Arc::new(TwoPageApi), 2);
    let mut output = Vec::new();
    let mut stderr_buf = Vec::new();

    let result = run_cli_with_stderr(
        &app,
        &checkpoints,
        Cli::try_parse_from([
            "uservoice-cli",
            "--output",
            "ndjson",
            "ideas",
            "search",
            "--query",
            "test",
            "--all",
        ])
        .expect("CLI"),
        &mut output,
        &mut stderr_buf,
    )
    .await;
    assert!(
        result.is_ok(),
        "--output ndjson --all should succeed: {:?}",
        result.err()
    );

    let lines: Vec<&[u8]> = output
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(lines.len(), 2, "should have 2 NDJSON lines");
}
