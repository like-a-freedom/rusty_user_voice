use std::{fmt, sync::Arc};

use crate::ClientError;

/// Redacted bearer access token.
///
/// Wraps the UserVoice API key in an `Arc<str>` so it is cheap to clone
/// across tasks. The token value never appears in `Debug`, `Display`, errors,
/// tracing, or MCP output — see `CONTEXT.md` "Credential Boundary".
#[derive(Clone)]
pub struct AccessToken(Arc<str>);

impl AccessToken {
    /// Parses a raw environment value into a redacted token.
    ///
    /// Leading/trailing whitespace is trimmed. An empty (after trim) value is
    /// rejected as `ClientError::EmptyAccessToken`.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::EmptyAccessToken`] when `value` is empty or
    /// whitespace-only.
    pub fn parse(value: String) -> Result<Self, ClientError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(ClientError::EmptyAccessToken);
        }
        Ok(Self(Arc::from(value)))
    }

    /// Exposes the raw token to trusted adapter code only. `pub(crate)` keeps
    /// this outside the public API surface so the value cannot leak through a
    /// downstream consumer of this crate.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessToken([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_reveals_token() {
        let token = AccessToken::parse("uv-secret-value".to_owned()).expect("valid token");
        let rendered = format!("{token:?}");
        assert_eq!(rendered, "AccessToken([REDACTED])");
        assert!(!rendered.contains("uv-secret-value"));
    }

    #[test]
    fn parse_rejects_empty_value() {
        assert!(matches!(
            AccessToken::parse("".to_owned()),
            Err(ClientError::EmptyAccessToken)
        ));
        assert!(matches!(
            AccessToken::parse("   \t ".to_owned()),
            Err(ClientError::EmptyAccessToken)
        ));
    }

    #[test]
    fn parse_trims_whitespace() {
        let token = AccessToken::parse("  uv-secret  ".to_owned()).expect("valid token");
        assert_eq!(token.expose(), "uv-secret");
    }
}
