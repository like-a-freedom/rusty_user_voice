use thiserror::Error;

/// Typed upstream and transport failures.
///
/// `#[non_exhaustive]` because new failure modes may be added without a
/// breaking version bump (`api-non_exhaustive`).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ClientError {
    #[error("invalid UserVoice base URL: {0}")]
    InvalidBaseUrl(String),
    #[error("USERVOICE_API_KEY is empty")]
    EmptyAccessToken,
    #[error("UserVoice authentication failed with HTTP {status}")]
    Authentication { status: u16 },
    #[error("UserVoice resource was not found")]
    NotFound,
    #[error("UserVoice rejected the request: {message}")]
    Validation { message: String },
    #[error("UserVoice rate limit was exceeded")]
    RateLimited {
        rate_limit: Option<crate::RateLimit>,
    },
    #[error("UserVoice returned HTTP {status}")]
    HttpStatus { status: u16 },
    #[error("UserVoice transport failed")]
    Transport(#[source] reqwest::Error),
    #[error("UserVoice response could not be decoded")]
    Decode(#[source] reqwest::Error),
    #[error("UserVoice returned no current user")]
    MissingCurrentUser,
    #[error("UserVoice {section} pagination exceeded the 100-page safety ceiling")]
    PaginationLimit { section: String },
}
