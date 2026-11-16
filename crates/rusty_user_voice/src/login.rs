//! First-time authentication for the CLI: resolve bearer key + base URL,
//! validate, and persist to the user config file. CLI-only (like `gcx login` /
//! `pangolin login` / `keenable login`) — no MCP tool. No clap types here, so
//! the flow is testable without a CLI.
//!
//! Agent-UX contract (ADR-0007): prompts fire only on a TTY and go to stderr;
//! headless runs with missing values fail fast with a classified
//! `CommandError`. Secrets and PII never appear in output, and there is
//! deliberately NO `--api-key` flag (Credential Boundary: argv leaks via
//! `ps`/history) — automation uses env or `--api-key-stdin`.

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::config::binary_config_path;
use crate::{
    cli::OutputMode,
    cli_error::{
        CommandError, REASON_CONFIRMATION_REQUIRED, REASON_IO_ERROR, REASON_MISSING_VALUE,
        REASON_NOT_LOGGED_IN, REASON_VALIDATION_FAILED,
    },
};

/// Pure merge for one login run. Env values are parameters (the caller reads
/// env), so this stays side-effect free and unit-testable.
///
/// URL: login `--base-url` > top-level `--base-url` — which clap has already
/// merged with `USERVOICE_BASE_URL` (`Cli.base_url` carries `env = …`), so no
/// separate env step is needed or wanted here.
/// Key: `--api-key-stdin` content > `USERVOICE_API_KEY` env (there is no
/// `--api-key` flag; missing values are prompted by the caller on a TTY).
pub fn resolve_login_inputs(
    login_url: Option<String>,
    global_url: Option<String>,
    stdin_token: Option<String>,
    env_token: Option<String>,
) -> (Option<String>, Option<String>) {
    let url = login_url.or(global_url);
    let token = stdin_token.or(env_token);
    (url, token)
}

/// IO wrapper around the output-mode resolution: reads the environment and the
/// effective config file, exactly like [`OutputMode::resolve_now`]. Kept
/// separate from the pure resolver so the precedence stays unit-tested.
pub fn machine_mode_now(flag: Option<OutputMode>) -> bool {
    OutputMode::resolve_now(flag).is_machine()
}

/// Trim an input and reject blanks — the single place where "empty means
/// missing" is decided, shared by the stdin and prompt sources.
pub fn required(raw: String, label: &str) -> Result<String, String> {
    let value = raw.trim().to_owned();
    if value.is_empty() {
        return Err(format!("{label} is required (empty input)"));
    }
    Ok(value)
}

/// Read the whole stdin as an API key (for `--api-key-stdin`, the
/// `gh --with-token` pattern). Refuses on a TTY (nothing piped) and on empty
/// input — it never falls through to a prompt, because a piped stdin has no
/// terminal to prompt on.
pub(crate) fn read_key_stdin() -> Result<String, String> {
    use std::io::Read;
    if io::stdin().is_terminal() {
        return Err(
            "--api-key-stdin needs a pipe (e.g. echo \"$USERVOICE_API_KEY\" | uservoice-cli login --api-key-stdin ...)"
                .to_owned(),
        );
    }
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|e| format!("cannot read API key from stdin: {e}"))?;
    required(buffer, "API key")
}

pub(crate) fn prompt_line(label: &str) -> Result<String, String> {
    // Prompts go to stderr: stdout stays a clean pipe for payloads.
    eprint!("{label}: ");
    io::stderr()
        .flush()
        .map_err(|e| format!("cannot write prompt: {e}"))?;
    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .map_err(|e| format!("cannot read input: {e}"))?;
    required(buffer, label)
}

pub(crate) fn prompt_password(label: &str) -> Result<String, String> {
    rpassword::prompt_password(format!("{label}: "))
        .map_err(|e| format!("cannot read API key: {e}"))
        .and_then(|value| required(value, "API key"))
}

/// Note when a binary-dir config exists: `load_yaml` tries it *first*, so it
/// shadows the user file — and after `logout` a credential may remain there.
///
/// `config_path` is the user file the caller knows about; it is `None` when
/// there is no current file to point at (a failed migration left only the
/// legacy one), in which case the note does not name a path that does not
/// exist.
pub fn shadow_note(config_path: Option<&Path>) -> Option<String> {
    let deployed = binary_config_path()?;
    if !deployed.exists() {
        return None;
    }
    Some(match config_path {
        Some(config_path) => format!(
            "note: {} exists and takes precedence over {} on load — remove it if login seems ignored (or if logout should clear the credential)",
            deployed.display(),
            config_path.display()
        ),
        None => format!(
            "note: {} holds a credential of its own — logout cannot remove it",
            deployed.display()
        ),
    })
}

/// Note when the pre-rename config still exists after the command ran.
///
/// `login` leaves it deliberately when the current config wins the path, and
/// the move can fail on permissions; `logout` rechecks after removing both
/// files, so `Some` there means a secret survived a logout that must not read
/// as clean. The load prefers the current file, so without this note the
/// leftover copy would be invisible.
fn legacy_note() -> Option<String> {
    use crate::config::legacy_config_leftover;
    let path = legacy_config_leftover()?;
    Some(format!(
        "note: {} still exists and holds a credential — remove it manually",
        path.display()
    ))
}

/// Resolved input for one `login` run. Built from clap flags in `bin/cli.rs`;
/// missing required values are prompted interactively (TTY only). There is
/// deliberately no `api_key` field (Credential Boundary).
#[derive(Debug, Clone, Default)]
pub struct LoginParams {
    /// `login --base-url`.
    pub base_url: Option<String>,
    /// Top-level `--base-url`, which clap has already merged with the
    /// `USERVOICE_BASE_URL` env var.
    pub global_base_url: Option<String>,
    /// Read the API key from stdin instead of env/prompt (keeps the key out
    /// of shell history and `ps`).
    pub key_stdin: bool,
    pub ca_cert: Option<String>,
    /// Overwrite an existing config, preserving its non-auth keys.
    pub force: bool,
    /// Skip only the network preflight; local validation always runs.
    pub no_verify: bool,
}

/// Outcome of a successful login. Carries no key and no PII (only the numeric
/// `user_id`), per the Credential Boundary and `AccessToken([REDACTED])`.
#[derive(Debug, Clone)]
pub struct LoginResult {
    pub config_path: PathBuf,
    pub base_url: String,
    pub validated: bool,
    pub user_id: Option<i64>,
    pub shadow_warning: Option<String>,
    /// The pre-rename config, when it survived migration — a second copy of a
    /// credential on disk, reported rather than silently ignored.
    pub legacy_warning: Option<String>,
}

/// Outcome of a successful logout.
#[derive(Debug, Clone)]
pub struct LogoutResult {
    pub removed: PathBuf,
    pub shadow_warning: Option<String>,
    /// Post-removal recheck: `None` after a clean logout, `Some` only when the
    /// pre-rename file still exists — a surviving secret must never read as
    /// success.
    pub legacy_warning: Option<String>,
}

const URL_HINT: &str = "re-run with --base-url URL (or set USERVOICE_BASE_URL)";
const KEY_HINT: &str = "re-run with --api-key-stdin (pipe) or set USERVOICE_API_KEY";
const NON_TTY_HINT: &str = "no TTY for interactive prompts — pass flags, --api-key-stdin, or env";
/// The verify command every success payload points at. `pub` so `bin/cli.rs`
/// formats from the same constant (DRY) instead of repeating the string.
pub const NEXT_COMMAND: &str = "uservoice-cli workspace --include-catalog";

/// Run the full login flow: guard → resolve → offline validation → optional
/// network preflight → secure save.
pub async fn run_login(params: LoginParams) -> Result<LoginResult, CommandError> {
    use crate::bootstrap::build_http_client;
    use crate::config::{
        YamlConfig, load_user_config_raw, migrate_legacy_user_config, save_user_config,
        user_config_path,
    };
    use uservoice_client::{AccessToken, ApiEndpoint, ReqwestUserVoiceClient, UserVoiceApi};

    // 0. Migrate a config left at the legacy path, so the guard below sees
    //    it and the user is not asked to log in again.
    migrate_legacy_user_config();

    // 1. Guard FIRST — before reading any secret from stdin, so a refused
    //    overwrite never consumes credentials or prompts.
    if let Some(path) = user_config_path()
        && path.exists()
        && !params.force
    {
        return Err(CommandError::new(
            REASON_CONFIRMATION_REQUIRED,
            format!(
                "config already exists at {} — not overwritten",
                path.display()
            ),
            "re-run with --force to overwrite the existing config".to_owned(),
        ));
    }

    // 2. Resolve: key from stdin > env > (TTY) prompt;
    //    URL from login flag > top-level flag > (TTY) prompt.
    let stdin_token = if params.key_stdin {
        Some(
            read_key_stdin()
                .map_err(|d| CommandError::new(REASON_MISSING_VALUE, d, NON_TTY_HINT.to_owned()))?,
        )
    } else {
        None
    };
    let (url, token) = resolve_login_inputs(
        params.base_url.clone(),
        params.global_base_url.clone(),
        stdin_token,
        std::env::var("USERVOICE_API_KEY").ok(),
    );
    let interactive = io::stdin().is_terminal();
    let url = match url {
        Some(value) if !value.trim().is_empty() => value.trim().to_owned(),
        _ if !interactive => {
            return Err(CommandError::new(
                REASON_MISSING_VALUE,
                "base URL is required".to_owned(),
                URL_HINT.to_owned(),
            ));
        }
        _ => prompt_line("UserVoice base URL (e.g. https://acme.uservoice.com)")
            .map_err(|d| CommandError::new(REASON_IO_ERROR, d, NON_TTY_HINT.to_owned()))?,
    };
    let key = match token {
        Some(value) if !value.trim().is_empty() => value.trim().to_owned(),
        _ if !interactive => {
            return Err(CommandError::new(
                REASON_MISSING_VALUE,
                "API key is required".to_owned(),
                KEY_HINT.to_owned(),
            ));
        }
        _ => prompt_password("UserVoice API key (input hidden)")
            .map_err(|d| CommandError::new(REASON_IO_ERROR, d, NON_TTY_HINT.to_owned()))?,
    };

    // 3. Offline validation ALWAYS: endpoint shape, key, and CA cert must be
    //    usable, so `--no-verify` can never persist a config that fails on the
    //    first real command.
    let endpoint = ApiEndpoint::parse(&url).map_err(|e| {
        CommandError::new(
            REASON_VALIDATION_FAILED,
            format!("invalid base URL {url}: {e} (not saved)"),
            "pass a tenant root such as https://acme.uservoice.com, then re-run login".to_owned(),
        )
    })?;
    let access = AccessToken::parse(key.clone()).map_err(|e| {
        CommandError::new(
            REASON_VALIDATION_FAILED,
            format!("invalid API key (not saved): {e}"),
            "check the key, then re-run login".to_owned(),
        )
    })?;
    let http = build_http_client(params.ca_cert.as_deref()).map_err(|e| {
        CommandError::new(
            REASON_VALIDATION_FAILED,
            format!("cannot build HTTP client (not saved): {e}"),
            "check the --ca-cert path, then re-run login".to_owned(),
        )
    })?;
    let client = ReqwestUserVoiceClient::from_http_client(http, endpoint, access).map_err(|e| {
        CommandError::new(
            REASON_VALIDATION_FAILED,
            format!("cannot build client for {url} (not saved): {e}"),
            "check the base URL, then re-run login".to_owned(),
        )
    })?;

    // 4. Network preflight unless --no-verify. `current_user` is the
    //    documented access preflight (uservoice_client/src/api.rs:28).
    let mut validated = false;
    let mut user_id = None;
    if !params.no_verify {
        let me = client.current_user().await.map_err(|e| {
            CommandError::new(
                REASON_VALIDATION_FAILED,
                format!("validation failed for {url}: {e} (not saved)"),
                "check the base URL and API key, then re-run login (or add --no-verify to save without the server call)"
                    .to_owned(),
            )
        })?;
        user_id = Some(me.data.id.get());
        validated = true;
    }

    // 5. Save, carrying over the keys login does not own (decision 13), then
    //    report a deployed config that would shadow what we just wrote.
    let preserved = if params.force {
        load_user_config_raw().unwrap_or_default()
    } else {
        YamlConfig::default()
    };
    let yaml = YamlConfig {
        uservoice_api_key: Some(key),
        uservoice_base_url: Some(url.clone()),
        uservoice_state_dir: preserved.uservoice_state_dir,
        uservoice_max_in_flight: preserved.uservoice_max_in_flight,
        default_output: preserved.default_output,
        ca_cert: params.ca_cert.clone().or(preserved.ca_cert),
        log_level: preserved.log_level,
    };
    let config_path = save_user_config(&yaml).map_err(|d| {
        CommandError::new(
            REASON_IO_ERROR,
            d,
            "check permissions on ~/.config/rusty_user_voice".to_owned(),
        )
    })?;
    let shadow_warning = shadow_note(Some(&config_path));
    let legacy_warning = legacy_note();
    Ok(LoginResult {
        config_path,
        base_url: url,
        validated,
        user_id,
        shadow_warning,
        legacy_warning,
    })
}

/// Delete the user config file, reporting any deployed config that still holds
/// a credential (login's mirror image).
pub fn run_logout() -> Result<LogoutResult, CommandError> {
    use crate::config::{
        CONFIG_FILENAME, legacy_user_config_dir, migrate_legacy_user_config, user_config_path,
    };

    // Migrate first, unconditionally: a config at the legacy path is a
    // logged-in install, and the settings load can skip it when a deployed
    // config next to the binary shadows the user file.
    migrate_legacy_user_config();

    let path = user_config_path().ok_or_else(|| {
        CommandError::new(
            REASON_IO_ERROR,
            "cannot resolve home directory".to_owned(),
            "set HOME and re-run logout".to_owned(),
        )
    })?;
    if !path.exists() {
        // The move can fail (permissions, a concurrent writer). A legacy
        // config still holds a credential, so it is removed in place rather
        // than left behind by a logout that reports "not logged in".
        let legacy_path = legacy_user_config_dir().map(|dir| dir.join(CONFIG_FILENAME));
        if let Some(legacy_path) = legacy_path.filter(|path| path.exists()) {
            std::fs::remove_file(&legacy_path).map_err(|d| {
                CommandError::new(
                    REASON_IO_ERROR,
                    format!("cannot remove {}: {d}", legacy_path.display()),
                    "check permissions on ~/.config/rusty-user-voice".to_owned(),
                )
            })?;
            return Ok(LogoutResult {
                removed: legacy_path,
                // No current file to point at — the note must not name a
                // path that does not exist.
                shadow_warning: shadow_note(None),
                // Recheck after the removal: `Some` only if the file survived
                // it (a concurrent writer), never after a clean logout.
                legacy_warning: legacy_note(),
            });
        }
        return Err(CommandError::new(
            REASON_NOT_LOGGED_IN,
            "not logged in (no config file found)".to_owned(),
            "run `uservoice-cli login` first".to_owned(),
        ));
    }
    std::fs::remove_file(&path).map_err(|d| {
        CommandError::new(
            REASON_IO_ERROR,
            format!("cannot remove {}: {d}", path.display()),
            "check permissions on ~/.config/rusty_user_voice".to_owned(),
        )
    })?;
    // A config left at the legacy path holds a credential too. Logout removes
    // it, or the secret would survive the logout — and it must not report
    // success when the removal failed.
    if let Some(legacy) = legacy_user_config_dir() {
        let legacy_path = legacy.join(CONFIG_FILENAME);
        if legacy_path.exists() {
            std::fs::remove_file(&legacy_path).map_err(|d| {
                CommandError::new(
                    REASON_IO_ERROR,
                    format!("cannot remove {}: {d}", legacy_path.display()),
                    "check permissions on ~/.config/rusty-user-voice".to_owned(),
                )
            })?;
        }
    }
    let shadow_warning = shadow_note(Some(&path));
    let legacy_warning = legacy_note();
    Ok(LogoutResult {
        removed: path,
        shadow_warning,
        legacy_warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_login_inputs_prefers_login_flag_over_global_flag() {
        let (url, token) = resolve_login_inputs(
            Some("https://flag.uservoice.com".to_owned()),
            Some("https://global.uservoice.com".to_owned()),
            None,
            Some("env-key".to_owned()),
        );
        assert_eq!(url.as_deref(), Some("https://flag.uservoice.com"));
        assert_eq!(token.as_deref(), Some("env-key"));
    }

    #[test]
    fn resolve_login_inputs_prefers_stdin_key_over_env_key() {
        let (_, token) = resolve_login_inputs(
            None,
            None,
            Some("stdin-key".to_owned()),
            Some("env-key".to_owned()),
        );
        assert_eq!(token.as_deref(), Some("stdin-key"));
    }

    #[test]
    fn resolve_login_inputs_all_missing_stays_missing() {
        let (url, token) = resolve_login_inputs(None, None, None, None);
        assert!(url.is_none());
        assert!(token.is_none());
    }

    #[test]
    fn required_trims_and_rejects_blank() {
        assert_eq!(
            required("  https://x  ".to_owned(), "base URL").unwrap(),
            "https://x"
        );
        let err = required("   ".to_owned(), "API key").unwrap_err();
        assert!(err.contains("API key"));
        assert!(err.contains("required"));
    }
}
