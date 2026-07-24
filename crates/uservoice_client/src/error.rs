use thiserror::Error;

/// Typed upstream and transport failures.
///
/// `#[non_exhaustive]` because new failure modes may be added without a
/// breaking version bump (`api-non_exhaustive`). Future variants will land
/// here as the adapter grows (HTTP transport, rate limits, decoding, etc.).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ClientError {
    #[error("invalid UserVoice base URL: {0}")]
    InvalidBaseUrl(String),
    #[error("USERVOICE_API_KEY is empty")]
    EmptyAccessToken,
}
