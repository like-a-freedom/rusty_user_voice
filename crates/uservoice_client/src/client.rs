use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::{
    AccessToken, ApiEndpoint, ApiResponse, ClientError, RateLimit, User, UserVoiceApi,
    continuation_from_link, models::UsersResponse,
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
}
