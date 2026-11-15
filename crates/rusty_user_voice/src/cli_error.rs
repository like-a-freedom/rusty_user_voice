//! Classified, machine-readable failures for CLI commands.
//!
//! `CommandError` carries a stable reason token, a detail, a copy-pasteable
//! hint, and an exit code. In machine mode the failure is a JSON envelope on
//! stderr and stdout stays empty.
//!
//! The reason tokens are a public contract: agents branch on them rather than
//! on the message text. They are declared once here and documented in
//! `crates/rusty_user_voice/skills/core/references/errors.md`.

/// A required value was absent.
pub const REASON_MISSING_VALUE: &str = "missing_value";
/// An existing config would be overwritten without confirmation.
pub const REASON_CONFIRMATION_REQUIRED: &str = "confirmation_required";
/// A value has the wrong shape, or a combination of flags is not allowed.
pub const REASON_VALIDATION_FAILED: &str = "validation_failed";
/// No usable credentials are configured.
pub const REASON_NOT_LOGGED_IN: &str = "not_logged_in";
/// Reading or writing failed locally.
pub const REASON_IO_ERROR: &str = "io_error";
/// The requested skill is not in the bundled corpus.
pub const REASON_UNKNOWN_SKILL: &str = "unknown_skill";
/// The requested reference does not belong to the skill.
pub const REASON_UNKNOWN_REFERENCE: &str = "unknown_reference";

/// Every reason token an agent may receive. The corpus contract test asserts
/// that each one is documented in the error reference.
pub const COMMAND_REASONS: &[&str] = &[
    REASON_MISSING_VALUE,
    REASON_CONFIRMATION_REQUIRED,
    REASON_VALIDATION_FAILED,
    REASON_NOT_LOGGED_IN,
    REASON_IO_ERROR,
    REASON_UNKNOWN_SKILL,
    REASON_UNKNOWN_REFERENCE,
];

/// Classified, machine-readable failure for a CLI command. `reason` is a
/// stable token agents branch on (see [`COMMAND_REASONS`]); `hint` is a
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
            REASON_CONFIRMATION_REQUIRED => 3,
            _ => 1,
        };
        Self {
            reason,
            detail,
            hint,
            exit_code,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_error_exit_codes() {
        let guard = CommandError::new(REASON_CONFIRMATION_REQUIRED, "d".to_owned(), "h".to_owned());
        assert_eq!(guard.exit_code, 3);
        for reason in COMMAND_REASONS {
            let error = CommandError::new(reason, "d".to_owned(), "h".to_owned());
            let expected = if *reason == REASON_CONFIRMATION_REQUIRED {
                3
            } else {
                1
            };
            assert_eq!(error.exit_code, expected, "reason {reason}");
        }
    }

    #[test]
    fn command_error_json_envelope_shape() {
        let error = CommandError::new(
            REASON_MISSING_VALUE,
            "API key is required".to_owned(),
            "hint-cmd".to_owned(),
        );
        let value = error.to_json();
        assert_eq!(value["ok"].as_bool(), Some(false));
        assert_eq!(value["reason"], REASON_MISSING_VALUE);
        assert_eq!(value["detail"], "API key is required");
        assert_eq!(value["hint"], "hint-cmd");
    }

    #[test]
    fn reason_tokens_are_unique_and_snake_case() {
        let mut seen = std::collections::BTreeSet::new();
        for reason in COMMAND_REASONS {
            assert!(seen.insert(*reason), "duplicate reason token {reason}");
            assert!(
                reason
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character == '_'),
                "reason token {reason} must be snake_case"
            );
        }
    }
}
