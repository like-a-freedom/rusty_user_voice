use std::future::Future;

use crate::{
    ClientError, Continuation, IdeaId, RateLimit, SearchSuggestionsRequest, Suggestion,
    SuggestionPage, User, WorkspaceCatalog,
};

/// Successful API response envelope: parsed body plus optional rate-limit
/// state and `Link: rel="next"` continuation extracted from headers.
#[derive(Clone, Debug)]
pub struct ApiResponse<T> {
    pub data: T,
    pub rate_limit: Option<RateLimit>,
    pub continuation: Option<Continuation>,
}

/// Native-async UserVoice API seam. Implementations must be
/// `Clone + Send + Sync + 'static`; `ReqwestUserVoiceClient` is the production
/// impl, `FakeUserVoiceApi` (in the `rusty_user_voice` test suite) is the
/// in-memory test impl.
///
/// Methods return `impl Future` rather than using `async_trait` so the trait
/// stays zero-overhead (`async-fn-in-trait`, stable since Rust 1.75).
pub trait UserVoiceApi: Clone + Send + Sync + 'static {
    /// Returns the authenticated principal. Acts as the access preflight.
    fn current_user(&self) -> impl Future<Output = Result<ApiResponse<User>, ClientError>> + Send;

    /// Loads the full workspace taxonomy (forums, categories, statuses, labels)
    /// across all pages.
    fn workspace_catalog(
        &self,
    ) -> impl Future<Output = Result<ApiResponse<WorkspaceCatalog>, ClientError>> + Send;

    /// Searches suggestions with UserVoice-native filter parameters.
    fn search_suggestions(
        &self,
        request: SearchSuggestionsRequest,
    ) -> impl Future<Output = Result<ApiResponse<SuggestionPage>, ClientError>> + Send;

    /// Fetches a single suggestion with sideloaded associations.
    fn suggestion(
        &self,
        idea_id: IdeaId,
    ) -> impl Future<Output = Result<ApiResponse<Suggestion>, ClientError>> + Send;
}
