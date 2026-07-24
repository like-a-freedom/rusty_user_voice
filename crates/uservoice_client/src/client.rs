use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::{
    AccessToken, ApiEndpoint, ApiResponse, ClientError, Continuation, IdeaId, Pagination,
    RateLimit, SearchSuggestionsRequest, Suggestion, SuggestionPage, UpstreamPagination, User,
    UserVoiceApi, WorkspaceCatalog, continuation_from_link,
    models::{
        CategoriesResponse, ForumsResponse, LabelsResponse, StatusesResponse, SuggestionsResponse,
        UsersResponse,
    },
};

/// Production HTTP adapter satisfying [`UserVoiceApi`].
///
/// Holds a reqwest client, the validated API endpoint, and the redacted access
/// token. The token is only exposed to reqwest's `bearer_auth` call; no other
/// code path reads its value (`obs-no-sensitive-data`).
#[derive(Clone)]
pub struct ReqwestUserVoiceClient {
    http: Client,
    endpoint: ApiEndpoint,
    token: AccessToken,
}

impl ReqwestUserVoiceClient {
    /// Constructs the production HTTP adapter.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Transport`] if the reqwest client cannot be
    /// built (e.g. TLS backend initialization failure).
    pub fn new(endpoint: ApiEndpoint, token: AccessToken) -> Result<Self, ClientError> {
        let http = Client::builder()
            .user_agent(concat!("rusty-user-voice/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(ClientError::Transport)?;
        Ok(Self {
            http,
            endpoint,
            token,
        })
    }

    /// Decodes a reqwest response into an [`ApiResponse<T>`], mapping every
    /// documented UserVoice failure status to its typed [`ClientError`]
    /// variant and extracting rate-limit + continuation headers on success.
    async fn decode<T: DeserializeOwned>(
        &self,
        response: Response,
    ) -> Result<ApiResponse<T>, ClientError> {
        let status = response.status();
        let rate_limit = RateLimit::from_headers(response.headers());
        let continuation = continuation_from_link(response.headers());
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                return Err(ClientError::Authentication {
                    status: status.as_u16(),
                });
            }
            StatusCode::NOT_FOUND => return Err(ClientError::NotFound),
            StatusCode::UNPROCESSABLE_ENTITY => {
                return Err(ClientError::Validation {
                    message: "request failed UserVoice validation".to_owned(),
                });
            }
            StatusCode::TOO_MANY_REQUESTS => {
                return Err(ClientError::RateLimited { rate_limit });
            }
            _ if !status.is_success() => {
                return Err(ClientError::HttpStatus {
                    status: status.as_u16(),
                });
            }
            _ => {}
        }
        let data = response.json::<T>().await.map_err(ClientError::Decode)?;
        Ok(ApiResponse {
            data,
            rate_limit,
            continuation,
        })
    }

    /// GET helper used by every read method. Sends the bearer token, applies
    /// the query pairs, and decodes the response.
    ///
    /// `pub(crate)` so the read-path retry wrapper added in Task 10 can call
    /// this without exposing it on the public API.
    pub(crate) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<ApiResponse<T>, ClientError> {
        let url = self.endpoint.admin_url(path)?;
        let response = self
            .http
            .get(url)
            .bearer_auth(self.token.expose())
            .query(query)
            .send()
            .await
            .map_err(ClientError::Transport)?;
        self.decode(response).await
    }

    const MAX_CATALOG_PAGES: usize = 100;

    async fn collect_catalog<T, R>(
        &self,
        path: &str,
        extract: impl Fn(R) -> (Vec<T>, UpstreamPagination),
    ) -> Result<ApiResponse<Vec<T>>, ClientError>
    where
        R: serde::de::DeserializeOwned,
    {
        let mut items = Vec::new();
        let mut continuation = None;
        let mut rate_limit = None;
        for _ in 0..Self::MAX_CATALOG_PAGES {
            let mut query = vec![("per_page".to_owned(), "100".to_owned())];
            match &continuation {
                Some(Continuation::Cursor { cursor }) => {
                    query.push(("cursor".to_owned(), cursor.clone()));
                }
                Some(Continuation::Page { page }) => {
                    query.push(("page".to_owned(), page.to_string()));
                }
                None => {}
            }
            let response: ApiResponse<R> = self.get(path, &query).await?;
            let (mut page_items, upstream) = extract(response.data);
            let pagination = Pagination::from_upstream(upstream, response.continuation);
            rate_limit = response.rate_limit.or(rate_limit);
            items.append(&mut page_items);
            match pagination.next {
                Some(next) => continuation = Some(next),
                None => {
                    return Ok(ApiResponse {
                        data: items,
                        rate_limit,
                        continuation: None,
                    });
                }
            }
        }
        Err(ClientError::PaginationLimit {
            section: path.to_owned(),
        })
    }
}

impl UserVoiceApi for ReqwestUserVoiceClient {
    async fn current_user(&self) -> Result<ApiResponse<User>, ClientError> {
        let response: ApiResponse<UsersResponse> = self.get("users/current", &[]).await?;
        let user = response
            .data
            .users
            .into_iter()
            .next()
            .ok_or(ClientError::MissingCurrentUser)?;
        Ok(ApiResponse {
            data: user,
            rate_limit: response.rate_limit,
            continuation: response.continuation,
        })
    }

    async fn workspace_catalog(&self) -> Result<ApiResponse<WorkspaceCatalog>, ClientError> {
        let forums = self
            .collect_catalog("forums", |value: ForumsResponse| {
                (value.forums, value.pagination)
            })
            .await?;
        let categories = self
            .collect_catalog("categories", |value: CategoriesResponse| {
                (value.categories, value.pagination)
            })
            .await?;
        let statuses = self
            .collect_catalog("statuses", |value: StatusesResponse| {
                (value.statuses, value.pagination)
            })
            .await?;
        let labels = self
            .collect_catalog("labels", |value: LabelsResponse| {
                (value.labels, value.pagination)
            })
            .await?;
        Ok(ApiResponse {
            data: WorkspaceCatalog {
                forums: forums.data,
                categories: categories.data,
                statuses: statuses.data,
                labels: labels.data,
            },
            rate_limit: labels
                .rate_limit
                .or(statuses.rate_limit)
                .or(categories.rate_limit)
                .or(forums.rate_limit),
            continuation: None,
        })
    }

    async fn search_suggestions(
        &self,
        request: SearchSuggestionsRequest,
    ) -> Result<ApiResponse<SuggestionPage>, ClientError> {
        let query = request.query_pairs()?;
        let response: ApiResponse<SuggestionsResponse> = self.get("suggestions", &query).await?;
        Ok(ApiResponse {
            data: SuggestionPage {
                items: response.data.suggestions,
                pagination: Pagination::from_upstream(
                    response.data.pagination,
                    response.continuation.clone(),
                ),
            },
            rate_limit: response.rate_limit,
            continuation: response.continuation,
        })
    }

    async fn suggestion(&self, idea_id: IdeaId) -> Result<ApiResponse<Suggestion>, ClientError> {
        let query = vec![(
            "includes".to_owned(),
            "labels,forums,status_updates,users,statuses,categories,attachments".to_owned(),
        )];
        let response: ApiResponse<SuggestionsResponse> =
            self.get(&format!("suggestions/{idea_id}"), &query).await?;
        let mut suggestion = response
            .data
            .suggestions
            .into_iter()
            .next()
            .ok_or(ClientError::NotFound)?;
        suggestion.included = response.data.included;
        Ok(ApiResponse {
            data: suggestion,
            rate_limit: response.rate_limit,
            continuation: response.continuation,
        })
    }
}
