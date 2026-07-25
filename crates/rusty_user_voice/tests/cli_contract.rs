mod support;

use std::{
    io::{self, Write},
    sync::Arc,
};

use clap::Parser;
use predicates::prelude::PredicateBooleanExt;
use rusty_user_voice::{App, CheckpointStore, Cli, cli_runner::run_cli};

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
