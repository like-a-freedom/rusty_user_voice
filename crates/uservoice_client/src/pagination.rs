use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Pagination block as UserVoice returns it in a response body. Every field is
/// optional because the API does not guarantee presence across endpoints.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpstreamPagination {
    pub page: Option<u32>,
    #[serde(alias = "page_size")]
    pub per_page: Option<u32>,
    pub total_pages: Option<u32>,
    pub total_records: Option<u64>,
    pub cursor: Option<String>,
}

/// Opaque continuation token. Passed back to the next call verbatim.
///
/// Tagged internally (`tag = "kind"`) so the wire shape round-trips through
/// serde. Cursor takes precedence over page in `Pagination::from_upstream`
/// because UserVoice documents cursor pagination as preferred.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Continuation {
    Cursor { cursor: String },
    Page { page: u32 },
}

/// Agent-facing pagination payload. Computed from `UpstreamPagination` plus
/// the `Link: rel="next"` header so callers see one shape regardless of
/// whether UserVoice returned a cursor or a page number.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Pagination {
    pub per_page: u32,
    pub total_records: Option<u64>,
    /// Opaque continuation token (cursor or page number, depending on what
    /// UserVoice returned). Pass back verbatim as `continuation`; do not
    /// interpret as a numeric offset (ADR-0005).
    pub next: Option<Continuation>,
    /// `true` when `next` is `Some(_)`. Computed by `from_upstream`. Named
    /// `has_more` rather than `next_offset` because UserVoice cursors are
    /// opaque and cannot be expressed as a numeric offset (ADR-0005).
    pub has_more: bool,
    /// `true` when the request was truncated because the bounded fan-out
    /// (`MAX_CONTEXT_PAGES = 100`) hit its safety ceiling (ADR-0004). Always
    /// `false` in successful responses; surfaced alongside `status =
    /// "conflict"` when the safety cap trips.
    #[serde(default)]
    pub limit_hit: bool,
}

impl Pagination {
    /// Merges the body pagination block with the parsed `Link` header. Cursor
    /// wins; otherwise the `Link` header wins; otherwise the next page number
    /// is computed from `page < total_pages`.
    pub fn from_upstream(
        value: UpstreamPagination,
        link_continuation: Option<Continuation>,
    ) -> Self {
        let next = value
            .cursor
            .map(|cursor| Continuation::Cursor { cursor })
            .or(link_continuation)
            .or_else(|| match (value.page, value.total_pages) {
                (Some(page), Some(total_pages)) if page < total_pages => {
                    Some(Continuation::Page { page: page + 1 })
                }
                _ => None,
            });
        let has_more = next.is_some();
        Self {
            per_page: value.per_page.unwrap_or(20),
            total_records: value.total_records,
            next,
            has_more,
            limit_hit: false,
        }
    }
}

/// Single source of truth for the `1 <= per_page <= 100` invariant (F-005,
/// DRY). Called by the CLI parser's `value_parser`, by
/// `SearchSuggestionsRequest::validate`, and by the collector's `idea_filter`.
///
/// # Errors
///
/// Returns [`crate::ClientError::Validation`] when `per_page` is outside
/// `[1, 100]`.
pub fn validate_per_page(per_page: u32) -> Result<u32, crate::ClientError> {
    if !(1..=100).contains(&per_page) {
        return Err(crate::ClientError::Validation {
            message: format!("per_page must be in 1..=100, got {per_page}"),
        });
    }
    Ok(per_page)
}

/// Parses the RFC 8288 `Link: <...>; rel="next"` header into a `Continuation`.
/// Returns `None` when no `rel="next"` entry is present or when the URL has no
/// `cursor` or `page` query parameter.
pub fn continuation_from_link(headers: &reqwest::header::HeaderMap) -> Option<Continuation> {
    headers
        .get_all("link")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .find(|part| part.contains("rel=\"next\"") || part.contains("rel=next"))
        .and_then(|part| part.split_once('<')?.1.split_once('>').map(|(url, _)| url))
        .and_then(|url| {
            url::Url::parse(url)
                .or_else(|_| url::Url::parse(&format!("https://link.invalid{url}")))
                .ok()
        })
        .and_then(|url| {
            let query = url
                .query_pairs()
                .collect::<std::collections::BTreeMap<_, _>>();
            query
                .get("cursor")
                .map(|cursor| Continuation::Cursor {
                    cursor: cursor.to_string(),
                })
                .or_else(|| {
                    query
                        .get("page")
                        .and_then(|page| page.parse().ok())
                        .map(|page| Continuation::Page { page })
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_wins_over_page_continuation() {
        let pagination = Pagination::from_upstream(
            UpstreamPagination {
                page: Some(1),
                per_page: Some(100),
                total_pages: Some(3),
                total_records: Some(272),
                cursor: Some("next-cursor".to_owned()),
            },
            None,
        );
        assert_eq!(
            pagination.next,
            Some(Continuation::Cursor {
                cursor: "next-cursor".to_owned(),
            })
        );
        assert!(pagination.has_more);
    }

    #[test]
    fn page_continuation_stops_at_last_page() {
        let pagination = Pagination::from_upstream(
            UpstreamPagination {
                page: Some(3),
                per_page: Some(100),
                total_pages: Some(3),
                total_records: Some(272),
                cursor: None,
            },
            None,
        );
        assert_eq!(pagination.next, None);
        assert!(!pagination.has_more);
    }

    #[test]
    fn parses_cursor_from_documented_next_link() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::LINK,
            reqwest::header::HeaderValue::from_static(
                "</api/v2/admin/suggestions?cursor=next-42>; rel=next",
            ),
        );
        assert_eq!(
            continuation_from_link(&headers),
            Some(Continuation::Cursor {
                cursor: "next-42".to_owned(),
            })
        );
    }

    #[test]
    fn parses_page_from_next_link_when_no_cursor() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::LINK,
            reqwest::header::HeaderValue::from_static(
                "</api/v2/admin/suggestions?page=2>; rel=next",
            ),
        );
        assert_eq!(
            continuation_from_link(&headers),
            Some(Continuation::Page { page: 2 })
        );
    }

    #[test]
    fn returns_none_when_no_next_link() {
        let headers = reqwest::header::HeaderMap::new();
        assert_eq!(continuation_from_link(&headers), None);
    }

    #[test]
    fn validate_per_page_accepts_bounds() {
        assert_eq!(validate_per_page(1).expect("min"), 1);
        assert_eq!(validate_per_page(100).expect("max"), 100);
    }

    #[test]
    fn validate_per_page_rejects_out_of_range() {
        assert!(matches!(
            validate_per_page(0),
            Err(crate::ClientError::Validation { .. })
        ));
        assert!(matches!(
            validate_per_page(101),
            Err(crate::ClientError::Validation { .. })
        ));
    }

    #[test]
    fn from_upstream_sets_limit_hit_false() {
        let pagination = Pagination::from_upstream(
            UpstreamPagination {
                page: Some(1),
                per_page: Some(100),
                total_pages: Some(1),
                total_records: Some(42),
                cursor: None,
            },
            None,
        );
        assert!(!pagination.has_more);
        assert!(!pagination.limit_hit);
    }
}
