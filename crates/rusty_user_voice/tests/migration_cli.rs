//! Regression tests for the directory rename: `rusty-user-voice` →
//! `rusty_user_voice`.
//!
//! The unit tests in `config.rs` cover the migration helpers. These drive the
//! real binary against a throwaway `HOME`, so a helper that stops being
//! called — the failure mode that would silently log an existing install out —
//! fails here.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

fn unique_home(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "uv-migrate-{}-{}-{}",
        tag,
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `~/.config/rusty-user-voice/config.yaml` — the pre-rename location.
fn legacy_config(home: &Path) -> PathBuf {
    home.join(".config/rusty-user-voice/config.yaml")
}

/// `~/.config/rusty_user_voice/config.yaml` — the current location.
fn current_config(home: &Path) -> PathBuf {
    home.join(".config/rusty_user_voice/config.yaml")
}

/// The legacy platform state directory: `<state>/rusty-user-voice`.
fn legacy_state_dir(home: &Path) -> PathBuf {
    platform_state_root(home).join("rusty-user-voice")
}

/// The current platform state directory: `<state>/rusty_user_voice`.
fn current_state_dir(home: &Path) -> PathBuf {
    platform_state_root(home).join("rusty_user_voice")
}

/// Mirrors `platform_state_dir`'s platform branch: `dirs::state_dir()` on
/// Linux (XDG, with `XDG_STATE_HOME` removed by [`migrate_cmd`]), the local
/// data dir on macOS, `%LOCALAPPDATA%` on Windows.
fn platform_state_root(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support")
    }
    #[cfg(windows)]
    {
        home.join("AppData/Local")
    }
    #[cfg(all(not(target_os = "macos"), not(windows)))]
    {
        home.join(".local/state")
    }
}

/// Hermetic child: own HOME, no inherited USERVOICE_* or XDG_* env, so the
/// platform defaults resolve inside the temp home.
fn migrate_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("uservoice-cli").unwrap();
    cmd.env("HOME", home)
        .env_remove("USERVOICE_API_KEY")
        .env_remove("USERVOICE_BASE_URL")
        .env_remove("USERVOICE_STATE_DIR")
        .env_remove("USERVOICE_MAX_IN_FLIGHT")
        .env_remove("USERVOICE_DEFAULT_OUTPUT")
        .env_remove("USERVOICE_LOG_LEVEL")
        .env_remove("XDG_STATE_HOME")
        .env_remove("XDG_DATA_HOME");
    cmd
}

/// A config that points at a closed loopback port: the settings load runs for
/// real, the request fails without DNS or a network dependency.
const LOOPBACK_CONFIG: &str = concat!(
    "uservoice_api_key: legacy-key\n",
    "uservoice_base_url: http://127.0.0.1:1\n",
);

fn seed_legacy_config(home: &Path) {
    let path = legacy_config(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, LOOPBACK_CONFIG).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
}

/// A data command: it must resolve `Settings`, which is what loads the config
/// and the state dir, and then fails on the connection.
fn data_command(home: &Path) -> Command {
    let mut cmd = migrate_cmd(home);
    cmd.args(["--output", "json", "workspace", "--include-catalog"]);
    cmd
}

#[test]
fn a_data_command_moves_the_legacy_config_into_the_current_path() {
    let home = unique_home("read");
    seed_legacy_config(&home);

    let out = data_command(&home).output().expect("run the command");
    let stderr = String::from_utf8_lossy(&out.stderr);

    // The failure must be the connection, never a lost credential: that is
    // the symptom the migration exists to prevent.
    assert!(
        !stderr.contains("USERVOICE_API_KEY is required"),
        "the legacy config must be readable after the rename: {stderr}"
    );
    assert!(
        !stderr.contains("not_logged_in"),
        "a legacy install must never read as logged out: {stderr}"
    );

    let migrated = std::fs::read_to_string(current_config(&home))
        .expect("the config must exist at the current path");
    assert!(migrated.contains("legacy-key"));
    assert!(
        !legacy_config(&home).exists(),
        "the legacy file must be moved, not copied"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(current_config(&home))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "the migrated API-key file must be owner-only");
    }
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn a_blocked_migration_falls_back_to_reading_the_legacy_config_in_place() {
    // The move cannot complete: `~/.config/rusty_user_voice` is occupied by a
    // plain file, so `create_dir_all` fails. The credential must still load
    // from the legacy path — a failed rename may not read as a lost login.
    let home = unique_home("fallback");
    seed_legacy_config(&home);
    let blocker = home.join(".config/rusty_user_voice");
    std::fs::create_dir_all(blocker.parent().unwrap()).unwrap();
    std::fs::write(&blocker, "not a directory").unwrap();

    let out = data_command(&home).output().expect("run the command");
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !stderr.contains("USERVOICE_API_KEY is required"),
        "an unmovable legacy config must still be read in place: {stderr}"
    );
    assert!(
        !stderr.contains("not_logged_in"),
        "a failed migration must not log the install out: {stderr}"
    );
    let legacy = std::fs::read_to_string(legacy_config(&home))
        .expect("the legacy config must stay where it is");
    assert!(legacy.contains("legacy-key"));
    assert!(
        !blocker.is_dir(),
        "the blocker file must not have been replaced"
    );
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn a_data_command_moves_the_legacy_state_dir_into_the_current_path() {
    let home = unique_home("state");
    seed_legacy_config(&home);
    let legacy = legacy_state_dir(&home);
    std::fs::create_dir_all(legacy.join("ledger")).unwrap();
    std::fs::write(legacy.join("ledger").join("a.json"), "{}").unwrap();

    let _ = data_command(&home).output().expect("run the command");

    let moved = current_state_dir(&home).join("ledger").join("a.json");
    assert!(
        moved.exists(),
        "the ledger must be at {}",
        current_state_dir(&home).display()
    );
    assert!(
        !legacy.exists(),
        "the legacy state dir must be moved, not copied"
    );
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn login_moves_a_legacy_config_before_the_overwrite_guard() {
    let home = unique_home("login");
    seed_legacy_config(&home);

    let mut cmd = migrate_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://x.uservoice.com",
        "--api-key-stdin",
        "--no-verify",
    ]);
    cmd.write_stdin("new-key");
    // The guard sees the migrated config and refuses to overwrite it, which
    // is only possible if the migration ran first.
    let out = cmd.assert().code(3).get_output().clone();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--force"), "{stderr}");

    let migrated =
        std::fs::read_to_string(current_config(&home)).expect("the config must be migrated");
    assert!(
        migrated.contains("legacy-key"),
        "the guard must not consume the existing credential"
    );
    assert!(!migrated.contains("new-key"));
    assert!(!legacy_config(&home).exists());
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn logout_removes_a_legacy_config_when_only_it_exists() {
    let home = unique_home("logout");
    seed_legacy_config(&home);

    let mut cmd = migrate_cmd(&home);
    cmd.args(["logout"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Logged out"));

    assert!(
        !legacy_config(&home).exists(),
        "the credential must not survive a logout"
    );
    assert!(!current_config(&home).exists());
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn logout_clears_a_legacy_config_when_a_deployed_config_shadows_it() {
    // A `config.yaml` next to the binary short-circuits the settings load, so
    // logout cannot rely on that load to perform the migration.
    let home = unique_home("logout-shadow");
    seed_legacy_config(&home);

    let bin_dir = unique_home("bin");
    let bin_path = bin_dir.join("uservoice-cli");
    std::fs::copy(env!("CARGO_BIN_EXE_uservoice-cli"), &bin_path).expect("copy the binary");
    std::fs::write(
        bin_dir.join("config.yaml"),
        "uservoice_api_key: deployed\nuservoice_base_url: https://d.uservoice.com\n",
    )
    .expect("write the deployed config");

    let mut cmd = Command::new(&bin_path);
    cmd.env("HOME", &home)
        .env_remove("USERVOICE_API_KEY")
        .env_remove("USERVOICE_BASE_URL")
        .env_remove("USERVOICE_STATE_DIR")
        .args(["logout"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Logged out"));

    assert!(
        !legacy_config(&home).exists(),
        "a logout must remove the credential even when a deployed config shadows it"
    );
    assert!(!current_config(&home).exists());
    std::fs::remove_dir_all(&home).ok();
    std::fs::remove_dir_all(&bin_dir).ok();
}

#[test]
fn a_user_named_state_dir_is_never_moved() {
    let home = unique_home("named-state");
    seed_legacy_config(&home);
    let named = home.join("my-own-ledger");
    std::fs::create_dir_all(&named).unwrap();
    std::fs::write(named.join("a.json"), "mine").unwrap();
    let platform_legacy = legacy_state_dir(&home);
    std::fs::create_dir_all(&platform_legacy).unwrap();
    std::fs::write(platform_legacy.join("a.json"), "legacy").unwrap();

    let mut cmd = migrate_cmd(&home);
    cmd.arg("--output")
        .arg("json")
        .arg("--state-dir")
        .arg(&named)
        .args(["workspace", "--include-catalog"]);
    let _ = cmd.output().expect("run the command");

    assert_eq!(
        std::fs::read_to_string(named.join("a.json")).unwrap(),
        "mine",
        "a directory the user named must never be renamed out from under them"
    );
    assert!(
        platform_legacy.join("a.json").exists(),
        "the platform legacy dir must only move when the platform default is in use"
    );
    std::fs::remove_dir_all(&home).ok();
}

/// The current config wins the migration and the legacy file stays behind —
/// it holds a credential the tool must not destroy. The success payload
/// reports it, so a second copy of a secret on disk never stays invisible.
#[test]
fn login_reports_a_legacy_config_the_current_config_left_behind() {
    let home = unique_home("leftover");
    seed_legacy_config(&home);
    let current = current_config(&home);
    std::fs::create_dir_all(current.parent().unwrap()).unwrap();
    std::fs::write(
        &current,
        "uservoice_api_key: current-key\nuservoice_base_url: https://c.uservoice.com\n",
    )
    .unwrap();

    let mut cmd = migrate_cmd(&home);
    cmd.args([
        "--output",
        "json",
        "login",
        "--base-url",
        "https://x.uservoice.com",
        "--api-key-stdin",
        "--force",
        "--no-verify",
    ]);
    cmd.write_stdin("new-key");
    let out = cmd.assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"].as_bool(), Some(true));
    let warning = v["legacy_warning"]
        .as_str()
        .expect("the surviving legacy config must be reported");
    assert!(warning.contains("rusty-user-voice"), "{warning}");
    assert!(
        legacy_config(&home).exists(),
        "the current config wins, so the legacy credential is preserved, not destroyed"
    );
    std::fs::remove_dir_all(&home).ok();
}

/// Both files present: migration keeps the legacy one (current wins), logout
/// removes both, and the payload reports no leftover — the field exists to
/// surface a surviving secret, so a clean logout must leave it null.
#[test]
fn logout_removes_both_configs_and_reports_no_leftover() {
    let home = unique_home("logout-both");
    seed_legacy_config(&home);
    let current = current_config(&home);
    std::fs::create_dir_all(current.parent().unwrap()).unwrap();
    std::fs::write(&current, "uservoice_api_key: current-key\n").unwrap();

    let mut cmd = migrate_cmd(&home);
    cmd.args(["--output", "json", "logout"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"].as_bool(), Some(true));
    assert!(
        v["legacy_warning"].is_null(),
        "a clean logout leaves nothing behind: {}",
        v["legacy_warning"]
    );
    assert!(
        !legacy_config(&home).exists(),
        "the legacy credential must not survive the logout"
    );
    assert!(!current.exists(), "the current credential must be gone too");
    std::fs::remove_dir_all(&home).ok();
}

/// `~/.config/rusty_user_voice` is a plain file, so the migration cannot run
/// and logout takes the "only the legacy file exists" branch. The leftover
/// check must run after the removal, never before it — otherwise a clean
/// logout would report a credential that is already gone.
#[test]
fn logout_of_a_legacy_only_install_reports_no_leftover_in_json() {
    let home = unique_home("logout-legacy-json");
    seed_legacy_config(&home);
    let blocker = home.join(".config/rusty_user_voice");
    std::fs::create_dir_all(blocker.parent().unwrap()).unwrap();
    std::fs::write(&blocker, "not a directory").unwrap();

    let mut cmd = migrate_cmd(&home);
    cmd.args(["--output", "json", "logout"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"].as_bool(), Some(true));
    assert!(
        v["legacy_warning"].is_null(),
        "the leftover check must run after the removal: {}",
        v["legacy_warning"]
    );
    assert!(
        v["removed"]
            .as_str()
            .expect("the removed path is reported")
            .contains("rusty-user-voice"),
        "the legacy file is what was removed: {}",
        v["removed"]
    );
    assert!(!legacy_config(&home).exists());
    std::fs::remove_dir_all(&home).ok();
}
