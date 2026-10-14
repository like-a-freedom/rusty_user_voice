use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

fn unique_home(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "uv-login-{}-{}-{}",
        tag,
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn user_config(home: &Path) -> PathBuf {
    home.join(".config/rusty-user-voice/config.yaml")
}

fn write_config(home: &Path, body: &str) {
    let path = user_config(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

/// Minimal config: key + URL only.
fn seed_config(home: &Path) {
    write_config(
        home,
        "uservoice_api_key: old\nuservoice_base_url: https://old.uservoice.com\n",
    );
}

/// Config that also carries the non-auth keys `--force` must preserve.
fn seed_config_with_extras(home: &Path) {
    write_config(
        home,
        concat!(
            "uservoice_api_key: old\n",
            "uservoice_base_url: https://old.uservoice.com\n",
            "uservoice_state_dir: /tmp/uv-preserved-state\n",
            "uservoice_max_in_flight: 5\n",
            "default_output: json\n",
            "log_level: debug\n",
        ),
    );
}

/// Hermetic child: own HOME, no inherited USERVOICE_* env. assert_cmd always
/// gives the child piped (non-TTY) stdin, so the TTY gate is exercised
/// headlessly.
fn login_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("uservoice-cli").unwrap();
    cmd.env("HOME", home)
        .env_remove("USERVOICE_API_KEY")
        .env_remove("USERVOICE_BASE_URL")
        .env_remove("USERVOICE_STATE_DIR")
        .env_remove("USERVOICE_MAX_IN_FLIGHT")
        .env_remove("USERVOICE_DEFAULT_OUTPUT")
        .env_remove("USERVOICE_LOG_LEVEL");
    cmd
}

#[test]
fn login_help_lists_expected_flags() {
    let mut cmd = Command::cargo_bin("uservoice-cli").unwrap();
    cmd.arg("login").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--base-url"))
        .stdout(predicate::str::contains("--api-key-stdin"))
        .stdout(predicate::str::contains("--ca-cert"))
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("--no-verify"));
}

#[test]
fn login_no_verify_writes_config_and_redacted_json() {
    let home = unique_home("save");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "--output",
        "json",
        "login",
        "--base-url",
        "https://acme.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
    ]);
    // MUST call write_stdin: the child blocks on stdin until the pipe closes.
    cmd.write_stdin("pat-from-pipe");
    let out = cmd.assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"].as_bool(), Some(true));
    assert_eq!(v["base_url"], "https://acme.uservoice.com");
    assert_eq!(v["validated"].as_bool(), Some(false));
    assert!(
        v["user_id"].is_null(),
        "--no-verify performs no network call, so user_id stays null"
    );
    assert!(v["next"].as_str().unwrap().contains("workspace"));
    for forbidden in ["uservoice_api_key", "api_key", "token", "pat-from-pipe"] {
        assert!(
            v.get(forbidden).is_none(),
            "{forbidden} must never appear in output"
        );
    }
    let saved = std::fs::read_to_string(user_config(&home)).unwrap();
    assert!(saved.contains("pat-from-pipe"));
    assert!(saved.contains("uservoice_base_url"));
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn login_refuses_overwrite_without_force() {
    let home = unique_home("guard");
    seed_config(&home);
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://x.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
    ]);
    cmd.write_stdin("new-key");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
    // Guard must not modify the existing file (it also fires before the key
    // is consumed for anything else).
    let saved = std::fs::read_to_string(user_config(&home)).unwrap();
    assert!(saved.contains("https://old.uservoice.com"));
    assert!(!saved.contains("new-key"));
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn guard_json_reports_confirmation_required_with_exit_3() {
    let home = unique_home("guardjson");
    seed_config(&home);
    let mut cmd = login_cmd(&home);
    cmd.args([
        "--output",
        "json",
        "login",
        "--base-url",
        "https://x.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
    ]);
    cmd.write_stdin("new-key");
    let out = cmd.assert().code(3).get_output().clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["ok"].as_bool(), Some(false));
    assert_eq!(v["reason"], "confirmation_required");
    assert!(v["hint"].as_str().unwrap().contains("--force"));
    // Pipe-clean contract: stdout stays empty on failure.
    assert!(out.stdout.is_empty());
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn key_stdin_empty_input_is_rejected() {
    let home = unique_home("stdinempty");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "--output",
        "json",
        "login",
        "--base-url",
        "https://s.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
    ]);
    cmd.write_stdin("");
    let out = cmd.assert().code(1).get_output().clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], "missing_value");
    assert!(
        !user_config(&home).exists(),
        "nothing must be saved on failure"
    );
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn headless_missing_key_fails_fast_with_json_envelope() {
    let home = unique_home("headless");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "--output",
        "json",
        "login",
        "--base-url",
        "https://h.uservoice.com",
        "--no-verify",
    ]);
    // No write_stdin: piped-but-empty stdin, and never a TTY → fail fast, never prompt.
    let out = cmd.assert().code(1).get_output().clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], "missing_value");
    assert!(v["hint"].as_str().unwrap().contains("--api-key-stdin"));
    assert!(!user_config(&home).exists());
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn env_key_is_used_when_stdin_absent() {
    let home = unique_home("envkey");
    let mut cmd = login_cmd(&home);
    cmd.env("USERVOICE_API_KEY", "env-key-123");
    cmd.args([
        "--output",
        "json",
        "login",
        "--base-url",
        "https://e.uservoice.com",
        "--no-verify",
    ]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"].as_bool(), Some(true));
    let saved = std::fs::read_to_string(user_config(&home)).unwrap();
    assert!(saved.contains("env-key-123"));
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn login_force_overwrites_and_logout_removes() {
    let home = unique_home("force");
    seed_config(&home);
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://y.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
        "--force",
    ]);
    cmd.write_stdin("new-key");
    cmd.assert().success();
    let saved = std::fs::read_to_string(user_config(&home)).unwrap();
    assert!(saved.contains("https://y.uservoice.com"));
    let mut out = login_cmd(&home);
    out.arg("logout");
    out.assert()
        .success()
        .stdout(predicate::str::contains("Logged out"));
    assert!(!user_config(&home).exists());
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn force_preserves_non_auth_keys() {
    let home = unique_home("preserve");
    seed_config_with_extras(&home);
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://y.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
        "--force",
    ]);
    cmd.write_stdin("new-key");
    cmd.assert().success();
    let saved = std::fs::read_to_string(user_config(&home)).unwrap();
    // Decision 13: --force rewrites only the 3 login-owned keys.
    assert!(saved.contains("https://y.uservoice.com"));
    assert!(saved.contains("new-key"));
    for preserved in [
        "/tmp/uv-preserved-state",
        "uservoice_max_in_flight: 5",
        "default_output: json",
        "log_level: debug",
    ] {
        assert!(
            saved.contains(preserved),
            "--force must preserve {preserved}"
        );
    }
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn logout_without_config_reports_not_logged_in() {
    let home = unique_home("logout");
    let mut cmd = login_cmd(&home);
    cmd.args(["--output", "json", "logout"]);
    let out = cmd.assert().code(1).get_output().clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], "not_logged_in");
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn saved_file_is_owner_only_on_unix() {
    let home = unique_home("perms");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://z.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
    ]);
    cmd.write_stdin("k");
    cmd.assert().success();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(user_config(&home))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn api_key_flag_is_still_rejected() {
    // Credential Boundary: no --api-key flag may appear, including on login.
    let home = unique_home("nokeyflag");
    let mut cmd = login_cmd(&home);
    cmd.args(["login", "--api-key", "must-not-be-accepted", "--no-verify"]);
    cmd.assert()
        .code(2)
        .stderr(predicate::str::contains("unexpected argument '--api-key'"));
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn machine_mode_from_config_default_output() {
    let home = unique_home("cfgmode");
    write_config(&home, "default_output: json\n");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://c.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
        "--force",
    ]);
    cmd.write_stdin("k");
    let out = cmd.assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out)
        .expect("default_output: json must select the machine envelope without --output");
    assert_eq!(v["ok"].as_bool(), Some(true));
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn machine_mode_from_env_default_output() {
    let home = unique_home("envmode");
    let mut cmd = login_cmd(&home);
    cmd.env("USERVOICE_DEFAULT_OUTPUT", "json");
    cmd.args([
        "login",
        "--base-url",
        "https://c.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
    ]);
    cmd.write_stdin("k");
    let out = cmd.assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out)
        .expect("USERVOICE_DEFAULT_OUTPUT=json must select the machine envelope");
    assert_eq!(v["ok"].as_bool(), Some(true));
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn no_verify_still_rejects_invalid_base_url() {
    // Decision 4: --no-verify skips the network call, never local validation.
    let home = unique_home("badurl");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "--output",
        "json",
        "login",
        "--base-url",
        "https://example.com",
        "--api-key-stdin",
        "--no-verify",
    ]);
    cmd.write_stdin("k");
    let out = cmd.assert().code(1).get_output().clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], "validation_failed");
    assert!(
        !user_config(&home).exists(),
        "a broken URL must never be saved"
    );
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn no_verify_still_rejects_unreadable_ca_cert() {
    let home = unique_home("badcert");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "--output",
        "json",
        "login",
        "--base-url",
        "https://z.uservoice.com",
        "--api-key-stdin",
        "--ca-cert",
        "/nonexistent/uv-ca.pem",
        "--no-verify",
    ]);
    cmd.write_stdin("k");
    let out = cmd.assert().code(1).get_output().clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], "validation_failed");
    assert!(v["detail"].as_str().unwrap().contains("ca_cert"));
    assert!(
        !user_config(&home).exists(),
        "a broken ca_cert must never be saved"
    );
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn prose_mode_is_the_default_without_any_output_hint() {
    let home = unique_home("prose");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://p.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
    ]);
    cmd.write_stdin("k");
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("Saved to"), "{text}");
    assert!(text.contains("Next:"), "{text}");
    assert!(!text.contains("\"ok\""), "human mode must not emit JSON");
    std::fs::remove_dir_all(&home).ok();
}
