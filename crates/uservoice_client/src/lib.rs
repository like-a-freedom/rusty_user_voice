#![deny(unsafe_code)]
//! UserVoice Admin API adapter (DDD anti-corruption layer).
//!
//! This crate owns the typed UserVoice Admin API adapter, the redacted access
//! token, the validated/normalized API endpoint, semantic ID newtypes,
//! pagination, rate limits, and the native-async `UserVoiceApi` seam. It is
//! deliberately free of any MCP, Clap, or application-workflow dependency:
//! schema drift and HTTP shape are contained here so the application layer
//! never sees raw upstream payloads.
//!
//! See `CONTEXT.md` at the repository root for the ubiquitous language and
//! `docs/adr/0001` for the rationale of the crate split.

mod api;
mod auth;
mod client;
mod endpoint;
mod error;
mod ids;
mod models;
mod pagination;
mod rate_limit;

pub use api::{ApiResponse, UserVoiceApi};
pub use auth::AccessToken;
pub use client::ReqwestUserVoiceClient;
pub use endpoint::ApiEndpoint;
pub use error::ClientError;
pub use ids::{
    CategoryId, CommentId, FeedbackRecordId, ForumId, IdeaId, LabelId, NoteId, StatusId,
    StatusUpdateId, SupporterId, UserId,
};
pub use models::User;
pub use pagination::{
    Continuation, Pagination, UpstreamPagination, continuation_from_link, validate_per_page,
};
pub use rate_limit::RateLimit;
