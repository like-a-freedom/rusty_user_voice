use reqwest::header::HeaderMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parsed rate-limit state from response headers. Mirrors the four headers
/// UserVoice documents: `X-Rate-Limit-Limit`, `X-Rate-Limit-Remaining`,
/// `X-Rate-Limit-Reset`, and `Retry-After` (only on 429).
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct RateLimit {
    pub limit: u64,
    pub remaining: u64,
    pub reset_epoch_seconds: u64,
    pub retry_after_epoch_seconds: Option<u64>,
}

impl RateLimit {
    /// Builds a `RateLimit` from headers. Returns `None` when any of the three
    /// `X-Rate-Limit-*` headers is missing or unparseable, because a partial
    /// rate-limit view is worse than none — callers cannot pace correctly.
    /// `Retry-After` is optional and only expected on 429.
    pub fn from_headers(headers: &HeaderMap) -> Option<Self> {
        fn number(headers: &HeaderMap, name: &str) -> Option<u64> {
            headers.get(name)?.to_str().ok()?.parse().ok()
        }
        Some(Self {
            limit: number(headers, "x-rate-limit-limit")?,
            remaining: number(headers, "x-rate-limit-remaining")?,
            reset_epoch_seconds: number(headers, "x-rate-limit-reset")?,
            retry_after_epoch_seconds: number(headers, "retry-after"),
        })
    }

    pub fn retry_delay(headers: &HeaderMap, attempt: u32) -> std::time::Duration {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if let Some(raw) = headers
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
        {
            let seconds = if raw > now { raw - now } else { raw };
            return std::time::Duration::from_secs(seconds);
        }
        let exponential_ms = 250_u64.saturating_mul(1_u64 << attempt.saturating_sub(1));
        let jitter_ms = rand::random_range(0..=100_u64);
        std::time::Duration::from_millis(exponential_ms.saturating_add(jitter_ms))
    }
}

#[cfg(test)]
mod tests {
    use reqwest::header::{HeaderMap, HeaderValue};

    use super::*;

    #[test]
    fn parses_all_documented_rate_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-rate-limit-limit", HeaderValue::from_static("60"));
        headers.insert("x-rate-limit-remaining", HeaderValue::from_static("0"));
        headers.insert("x-rate-limit-reset", HeaderValue::from_static("1730000000"));
        headers.insert("retry-after", HeaderValue::from_static("1730000000"));
        let rate = RateLimit::from_headers(&headers).expect("complete headers");
        assert_eq!(rate.limit, 60);
        assert_eq!(rate.remaining, 0);
        assert_eq!(rate.retry_after_epoch_seconds, Some(1_730_000_000));
    }

    #[test]
    fn returns_none_when_any_limit_header_missing() {
        let mut headers = HeaderMap::new();
        headers.insert("x-rate-limit-limit", HeaderValue::from_static("60"));
        // remaining + reset deliberately omitted
        assert!(RateLimit::from_headers(&headers).is_none());
    }

    #[test]
    fn retry_after_is_optional() {
        let mut headers = HeaderMap::new();
        headers.insert("x-rate-limit-limit", HeaderValue::from_static("60"));
        headers.insert("x-rate-limit-remaining", HeaderValue::from_static("30"));
        headers.insert("x-rate-limit-reset", HeaderValue::from_static("1730000000"));
        let rate = RateLimit::from_headers(&headers).expect("complete headers");
        assert_eq!(rate.retry_after_epoch_seconds, None);
    }
}
