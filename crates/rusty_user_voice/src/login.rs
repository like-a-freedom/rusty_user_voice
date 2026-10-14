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
use std::path::Path;

use crate::cli::OutputMode;
use crate::config::{binary_config_path, load_user_config_raw};

/// Classified, machine-readable failure for login/logout. `reason` is a stable
/// token agents branch on (`missing_value`, `confirmation_required`,
/// `validation_failed`, `not_logged_in`, `io_error`); `hint` is a
/// copy-pasteable fix. Reported on stderr (JSON envelope in machine mode,
/// `Error:`/`Hint:` prose otherwise); stdout stays empty.
#[derive(Debug, Clone)]
pub struct CommandError {
    pub reason: &'static str,
    pub detail: String,
    pub hint: String,
    pub exit_code: i32,
}

impl CommandError {
    /// Exit-code contract: 3 = confirmation_required (re-run with --force),
    /// 1 = everything else. clap owns 2 (usage errors).
    pub fn new(reason: &'static str, detail: String, hint: String) -> Self {
        let exit_code = match reason {
            "confirmation_required" => 3,
            _ => 1,
        };
        Self { reason, detail, hint, exit_code }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": false,
            "reason": self.reason,
            "detail": self.detail,
            "hint": self.hint,
        })
    }

    /// Print to stderr (payloads stay on stdout) and exit. Never returns.
    pub fn report(self, machine: bool) -> ! {
        if machine {
            eprintln!("{}", self.to_json());
        } else {
            eprintln!("Error: {}", self.detail);
            eprintln!("Hint: {}", self.hint);
        }
        std::process::exit(self.exit_code);
    }
}

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

/// Pure output-mode resolution, mirroring the precedence every other command
/// uses (`config.rs`: flag > `USERVOICE_DEFAULT_OUTPUT` env > `config.yaml
/// default_output` > human). Login is intercepted before
/// `Settings::from_sources`, so it must resolve the mode itself — otherwise a
/// `default_output: json` user gets prose errors from an agent-driven command.
pub fn resolve_machine_mode(
    flag: Option<OutputMode>,
    env_default: Option<String>,
    config_default: Option<String>,
) -> bool {
    let named = |value: &str| matches!(value, "json" | "ndjson");
    match flag {
        Some(mode) => mode != OutputMode::Human,
        None => env_default
            .as_deref()
            .or(config_default.as_deref())
            .is_some_and(named),
    }
}

/// IO wrapper around [`resolve_machine_mode`]: reads env + the user config.
/// Kept separate from the pure resolver so the precedence stays unit-tested.
pub fn machine_mode_now(flag: Option<OutputMode>) -> bool {
    resolve_machine_mode(
        flag,
        std::env::var("USERVOICE_DEFAULT_OUTPUT")
            .ok()
            .filter(|value| !value.is_empty()),
        load_user_config_raw().and_then(|cfg| cfg.default_output),
    )
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
// Task 2 scaffolding: first used by Task 4; the allow is removed there.
#[allow(dead_code)]
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

// Task 2 scaffolding: first used by Task 4; the allow is removed there.
#[allow(dead_code)]
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

// Task 2 scaffolding: first used by Task 4; the allow is removed there.
#[allow(dead_code)]
pub(crate) fn prompt_password(label: &str) -> Result<String, String> {
    rpassword::prompt_password(format!("{label}: "))
        .map_err(|e| format!("cannot read API key: {e}"))
        .and_then(|value| required(value, "API key"))
}

/// Note when a binary-dir config exists: `load_yaml` tries it *first*, so it
/// shadows the user file — and after `logout` a credential may remain there.
pub fn shadow_note(config_path: &Path) -> Option<String> {
    let deployed = binary_config_path()?;
    if deployed.exists() {
        Some(format!(
            "note: {} exists and takes precedence over {} on load — remove it if login seems ignored (or if logout should clear the credential)",
            deployed.display(),
            config_path.display()
        ))
    } else {
        None
    }
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
    fn resolve_machine_mode_prefers_the_explicit_flag() {
        assert!(resolve_machine_mode(
            Some(OutputMode::Ndjson),
            Some("human".to_owned()),
            Some("human".to_owned()),
        ));
    }

    #[test]
    fn resolve_machine_mode_falls_back_to_env_then_config() {
        assert!(resolve_machine_mode(None, Some("json".to_owned()), Some("human".to_owned())));
        assert!(resolve_machine_mode(None, None, Some("json".to_owned())));
    }

    #[test]
    fn resolve_machine_mode_human_and_unknown_values_are_prose() {
        assert!(!resolve_machine_mode(Some(OutputMode::Human), None, None));
        assert!(!resolve_machine_mode(None, Some("human".to_owned()), None));
        assert!(!resolve_machine_mode(None, None, Some("nonsense".to_owned())));
        assert!(!resolve_machine_mode(None, None, None));
    }

    #[test]
    fn required_trims_and_rejects_blank() {
        assert_eq!(required("  https://x  ".to_owned(), "base URL").unwrap(), "https://x");
        let err = required("   ".to_owned(), "API key").unwrap_err();
        assert!(err.contains("API key"));
        assert!(err.contains("required"));
    }

    #[test]
    fn command_error_exit_codes() {
        let guard = CommandError::new("confirmation_required", "d".to_owned(), "h".to_owned());
        assert_eq!(guard.exit_code, 3);
        for reason in ["missing_value", "validation_failed", "not_logged_in", "io_error"] {
            let e = CommandError::new(reason, "d".to_owned(), "h".to_owned());
            assert_eq!(e.exit_code, 1, "reason {reason} must exit 1");
        }
    }

    #[test]
    fn command_error_json_envelope_shape() {
        let e = CommandError::new(
            "missing_value",
            "API key is required".to_owned(),
            "hint-cmd".to_owned(),
        );
        let v = e.to_json();
        assert_eq!(v["ok"].as_bool(), Some(false));
        assert_eq!(v["reason"], "missing_value");
        assert_eq!(v["detail"], "API key is required");
        assert_eq!(v["hint"], "hint-cmd");
    }
}
