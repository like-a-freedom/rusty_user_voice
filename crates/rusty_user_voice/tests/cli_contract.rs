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
