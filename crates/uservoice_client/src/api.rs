use std::future::Future;

use crate::{
    ActivityEntry, ClientError, Comment, ContextPage, Continuation, CreateFeedbackRecordRequest,
    CreateStatusUpdateRequest, FeedbackRecord, IdeaId, LabelId, Note, RateLimit,
    SearchSuggestionsRequest, StatusUpdate, Suggestion, SuggestionPage, Supporter,
    UpdateSuggestionRequest, User, WorkspaceCatalog,
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

    /// Fetches all comments for a suggestion, following pagination.
    fn comments(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> impl Future<Output = Result<ApiResponse<ContextPage<Comment>>, ClientError>> + Send;

    /// Fetches all notes for a suggestion, following pagination.
    fn notes(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> impl Future<Output = Result<ApiResponse<ContextPage<Note>>, ClientError>> + Send;

    /// Fetches all supporters for a suggestion, following pagination.
    fn supporters(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> impl Future<Output = Result<ApiResponse<ContextPage<Supporter>>, ClientError>> + Send;

    /// Fetches all feedback records for a suggestion, following pagination.
    fn feedback_records(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> impl Future<Output = Result<ApiResponse<ContextPage<FeedbackRecord>>, ClientError>> + Send;

    /// Fetches all status updates for a suggestion, following pagination.
    fn status_updates(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> impl Future<Output = Result<ApiResponse<ContextPage<StatusUpdate>>, ClientError>> + Send;

    /// Fetches all activity entries for a suggestion, following pagination.
    fn activity(
        &self,
        idea_id: IdeaId,
        per_page: u32,
    ) -> impl Future<Output = Result<ApiResponse<ContextPage<ActivityEntry>>, ClientError>> + Send;

    fn update_suggestion(
        &self,
        idea_id: IdeaId,
        request: UpdateSuggestionRequest,
    ) -> impl Future<Output = Result<ApiResponse<Suggestion>, ClientError>> + Send;

    fn attach_label(
        &self,
        idea_id: IdeaId,
        label_id: LabelId,
    ) -> impl Future<Output = Result<ApiResponse<()>, ClientError>> + Send;

    fn detach_label(
        &self,
        idea_id: IdeaId,
        label_id: LabelId,
    ) -> impl Future<Output = Result<ApiResponse<()>, ClientError>> + Send;

    fn create_note(
        &self,
        idea_id: IdeaId,
        body: String,
    ) -> impl Future<Output = Result<ApiResponse<Note>, ClientError>> + Send;

    fn create_status_update(
        &self,
        request: CreateStatusUpdateRequest,
    ) -> impl Future<Output = Result<ApiResponse<StatusUpdate>, ClientError>> + Send;

    fn create_feedback_record(
        &self,
        request: CreateFeedbackRecordRequest,
    ) -> impl Future<Output = Result<ApiResponse<FeedbackRecord>, ClientError>> + Send;
}
