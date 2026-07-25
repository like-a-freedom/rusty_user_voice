use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::{
    AccessToken, ActivityEntry, ApiEndpoint, ApiResponse, ClientError, Comment, ContextPage,
    Continuation, CreateFeedbackRecordRequest, CreateNoteRequest, CreateStatusUpdateRequest,
    FeedbackRecord, IdeaId, LabelId, Note, Pagination, RateLimit, SearchSuggestionsRequest,
    StatusUpdate, Suggestion, SuggestionPage, Supporter, UpdateSuggestionRequest,
    UpstreamPagination, User, UserVoiceApi, WorkspaceCatalog, continuation_from_link,
    models::{
        ActivityResponse, CategoriesResponse, CommentsResponse, FeedbackRecordsResponse,
        ForumsResponse, LabelsResponse, MutationResponse, NotesResponse, StatusUpdatesResponse,
        StatusesResponse, SuggestionsResponse, SupportersResponse, UsersResponse,
    },
    validate_per_page,
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
    pub(crate) const MAX_READ_ATTEMPTS: u32 = 3;
    pub(crate) const MAX_RATE_LIMIT_SLEEP: std::time::Duration = std::time::Duration::from_secs(30);

    pub(crate) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<ApiResponse<T>, ClientError> {
        let url = self.endpoint.admin_url(path)?;
        for attempt in 1..=Self::MAX_READ_ATTEMPTS {
            let response = self
                .http
                .get(url.clone())
                .bearer_auth(self.token.expose())
                .query(query)
                .send()
                .await
                .map_err(ClientError::Transport)?;
            let status = response.status();
            if attempt < Self::MAX_READ_ATTEMPTS
                && (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
            {
                let delay = RateLimit::retry_delay(response.headers(), attempt);
                if delay <= Self::MAX_RATE_LIMIT_SLEEP {
                    tracing::warn!(
                        operation = "uservoice_read_retry",
                        attempt,
                        status = status.as_u16(),
                        delay_ms = delay.as_millis() as u64,
                        "retrying transient UserVoice read"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
            }
            return self.decode(response).await;
        }
        Err(ClientError::HttpStatus { status: 503 })
    }

    pub(crate) async fn retry_with_rate_limit_sleep<F, Fut, T>(mut op: F) -> Result<T, ClientError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, ClientError>>,
    {
        match op().await {
            Err(ClientError::RateLimited { rate_limit }) => {
                let delay = rate_limit
                    .and_then(|rl| rl.retry_after_epoch_seconds)
                    .map(std::time::Duration::from_secs)
                    .unwrap_or_else(|| std::time::Duration::from_secs(1))
                    .min(Self::MAX_RATE_LIMIT_SLEEP);
                tracing::warn!(
                    operation = "uservoice_section_retry",
                    delay_ms = delay.as_millis() as u64,
                    "section-level 429; retrying once (ADR-0006)"
                );
                tokio::time::sleep(delay).await;
                op().await
            }
            other => other,
        }
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
            limit_hit: false,
        })
    }

    const MAX_CONTEXT_PAGES: usize = 100;

    fn idea_filter(
        idea_id: IdeaId,
        per_page: u32,
        continuation: Option<&Continuation>,
    ) -> Result<Vec<(String, String)>, ClientError> {
        validate_per_page(per_page)?;
        let mut query = vec![
            ("suggestion".to_owned(), idea_id.to_string()),
            ("per_page".to_owned(), per_page.to_string()),
        ];
        match continuation {
            Some(Continuation::Cursor { cursor }) => {
                query.push(("cursor".to_owned(), cursor.clone()));
            }
            Some(Continuation::Page { page }) => {
                query.push(("page".to_owned(), page.to_string()));
            }
            None => {}
        }
        Ok(query)
    }

    async fn collect_context<T, R>(
        &self,
        path: &str,
        idea_id: IdeaId,
        per_page: u32,
        extract: impl Fn(R) -> (Vec<T>, UpstreamPagination),
    ) -> Result<ApiResponse<ContextPage<T>>, ClientError>
    where
        R: serde::de::DeserializeOwned,
    {
        let mut items = Vec::new();
        let mut continuation = None;
        let mut total_records = None;
        let mut rate_limit = None;

        for _ in 0..Self::MAX_CONTEXT_PAGES {
            let query = Self::idea_filter(idea_id, per_page, continuation.as_ref())?;
            let path_owned = path.to_owned();
            let http = self.http.clone();
            let endpoint = self.endpoint.clone();
            let token = self.token.clone();
            let response: ApiResponse<R> = Self::retry_with_rate_limit_sleep(|| {
                let query = query.clone();
                let path_owned = path_owned.clone();
                let http = http.clone();
                let endpoint = endpoint.clone();
                let token = token.clone();
                async move {
                    let url = endpoint.admin_url(&path_owned)?;
                    let resp = http
                        .get(url)
                        .bearer_auth(token.expose())
                        .query(&query)
                        .send()
                        .await
                        .map_err(ClientError::Transport)?;
                    // Decode inline to avoid needing &self
                    let status = resp.status();
                    let rl = RateLimit::from_headers(resp.headers());
                    let cont = continuation_from_link(resp.headers());
                    match status {
                        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                            return Err(ClientError::Authentication {
                                status: status.as_u16(),
                            });
                        }
                        StatusCode::NOT_FOUND => return Err(ClientError::NotFound),
                        StatusCode::TOO_MANY_REQUESTS => {
                            return Err(ClientError::RateLimited { rate_limit: rl });
                        }
                        _ if !status.is_success() => {
                            return Err(ClientError::HttpStatus {
                                status: status.as_u16(),
                            });
                        }
                        _ => {}
                    }
                    let data = resp.json::<R>().await.map_err(ClientError::Decode)?;
                    Ok(ApiResponse {
                        data,
                        rate_limit: rl,
                        continuation: cont,
                    })
                }
            })
            .await?;
            let (mut page_items, upstream) = extract(response.data);
            let pagination = Pagination::from_upstream(upstream, response.continuation);
            total_records = pagination.total_records.or(total_records);
            rate_limit = response.rate_limit.or(rate_limit);
            items.append(&mut page_items);
            match pagination.next {
                Some(next) => continuation = Some(next),
                None => {
                    return Ok(ApiResponse {
                        data: ContextPage {
                            items,
                            pagination: Pagination {
                                per_page,
                                total_records,
                                next: None,
                                has_more: false,
                                limit_hit: false,
                            },
                        },
                        rate_limit,
                        continuation: None,
                    });
                }
            }
        }

        Err(ClientError::PaginationLimit {
            section: path.to_owned(),
            limit_hit: true,
        })
    }

    async fn put_json<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<ApiResponse<T>, ClientError> {
        let url = self.endpoint.admin_url(path)?;
        let response = self
            .http
            .put(url)
            .bearer_auth(self.token.expose())
            .json(body)
            .send()
            .await
            .map_err(ClientError::Transport)?;
        self.decode(response).await
    }

    async fn post_json<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<ApiResponse<T>, ClientError> {
        let url = self.endpoint.admin_url(path)?;
        let response = self
            .http
            .post(url)
            .bearer_auth(self.token.expose())
            .json(body)
            .send()
            .await
            .map_err(ClientError::Transport)?;
        self.decode(response).await
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

    async fn comments(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> Result<ApiResponse<ContextPage<Comment>>, ClientError> {
        self.collect_context(
            "comments",
            idea_id,
            per_page,
            |response: CommentsResponse| (response.comments, response.pagination),
        )
        .await
    }

    async fn notes(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> Result<ApiResponse<ContextPage<Note>>, ClientError> {
        self.collect_context("notes", idea_id, per_page, |response: NotesResponse| {
            (response.notes, response.pagination)
        })
        .await
    }

    async fn supporters(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> Result<ApiResponse<ContextPage<Supporter>>, ClientError> {
        self.collect_context(
            "supporters",
            idea_id,
            per_page,
            |response: SupportersResponse| (response.supporters, response.pagination),
        )
        .await
    }

    async fn feedback_records(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> Result<ApiResponse<ContextPage<FeedbackRecord>>, ClientError> {
        self.collect_context(
            "feedback_records",
            idea_id,
            per_page,
            |response: FeedbackRecordsResponse| (response.feedback_records, response.pagination),
        )
        .await
    }

    async fn status_updates(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> Result<ApiResponse<ContextPage<StatusUpdate>>, ClientError> {
        self.collect_context(
            "status_updates",
            idea_id,
            per_page,
            |response: StatusUpdatesResponse| (response.status_updates, response.pagination),
        )
        .await
    }

    async fn activity(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> Result<ApiResponse<ContextPage<ActivityEntry>>, ClientError> {
        self.collect_context(
            "suggestion_activity_entries",
            idea_id,
            per_page,
            |response: ActivityResponse| {
                (response.suggestion_activity_entries, response.pagination)
            },
        )
        .await
    }

    async fn update_suggestion(
        &self,
        idea_id: IdeaId,
        request: UpdateSuggestionRequest,
    ) -> Result<ApiResponse<Suggestion>, ClientError> {
        let response: ApiResponse<MutationResponse> = self
            .put_json(&format!("suggestions/{idea_id}"), &request)
            .await?;
        let suggestion = response
            .data
            .suggestions
            .into_iter()
            .next()
            .ok_or(ClientError::NotFound)?;
        Ok(ApiResponse {
            data: suggestion,
            rate_limit: response.rate_limit,
            continuation: response.continuation,
        })
    }

    async fn attach_label(
        &self,
        idea_id: IdeaId,
        label_id: LabelId,
    ) -> Result<ApiResponse<()>, ClientError> {
        let response: ApiResponse<serde_json::Value> = self
            .post_json(
                "labels/attach",
                &crate::LabelMutationRequest {
                    links: crate::LabelMutationLinks {
                        label: label_id,
                        suggestion: idea_id,
                    },
                },
            )
            .await?;
        Ok(ApiResponse {
            data: (),
            rate_limit: response.rate_limit,
            continuation: response.continuation,
        })
    }

    async fn detach_label(
        &self,
        idea_id: IdeaId,
        label_id: LabelId,
    ) -> Result<ApiResponse<()>, ClientError> {
        let response: ApiResponse<serde_json::Value> = self
            .post_json(
                "labels/detach",
                &crate::LabelMutationRequest {
                    links: crate::LabelMutationLinks {
                        label: label_id,
                        suggestion: idea_id,
                    },
                },
            )
            .await?;
        Ok(ApiResponse {
            data: (),
            rate_limit: response.rate_limit,
            continuation: response.continuation,
        })
    }

    async fn create_note(
        &self,
        idea_id: IdeaId,
        body: String,
    ) -> Result<ApiResponse<Note>, ClientError> {
        let response: ApiResponse<MutationResponse> = self
            .post_json(
                "notes",
                &CreateNoteRequest {
                    body,
                    body_mime_type: "text/plain",
                    links: crate::CreateNoteLinks {
                        suggestion: idea_id,
                    },
                },
            )
            .await?;
        let note = response
            .data
            .notes
            .into_iter()
            .next()
            .ok_or(ClientError::NotFound)?;
        Ok(ApiResponse {
            data: note,
            rate_limit: response.rate_limit,
            continuation: response.continuation,
        })
    }

    async fn create_status_update(
        &self,
        request: CreateStatusUpdateRequest,
    ) -> Result<ApiResponse<StatusUpdate>, ClientError> {
        let response: ApiResponse<MutationResponse> =
            self.post_json("status_updates", &request).await?;
        let update = response
            .data
            .status_updates
            .into_iter()
            .next()
            .ok_or(ClientError::NotFound)?;
        Ok(ApiResponse {
            data: update,
            rate_limit: response.rate_limit,
            continuation: response.continuation,
        })
    }

    async fn create_feedback_record(
        &self,
        request: CreateFeedbackRecordRequest,
    ) -> Result<ApiResponse<FeedbackRecord>, ClientError> {
        let response: ApiResponse<MutationResponse> =
            self.post_json("feedback_records", &request).await?;
        let feedback = response
            .data
            .feedback_records
            .into_iter()
            .next()
            .ok_or(ClientError::NotFound)?;
        Ok(ApiResponse {
            data: feedback,
            rate_limit: response.rate_limit,
            continuation: response.continuation,
        })
    }
}
