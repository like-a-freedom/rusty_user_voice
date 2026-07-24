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

// Task 1 lands the foundation types; the methods that exercise them
// (`AccessToken::expose`, `ApiEndpoint::admin_url`, and the `0` field of
// `AccessToken`) are consumed by the HTTP transport in Task 2. Allow dead code
// until then so the workspace clippy gate passes without forcing Task 2 in.
#![allow(dead_code)]

mod auth;
mod endpoint;
mod error;
mod ids;

pub use auth::AccessToken;
pub use endpoint::ApiEndpoint;
pub use error::ClientError;
pub use ids::{
    CategoryId, CommentId, FeedbackRecordId, ForumId, IdeaId, LabelId, NoteId, StatusId,
    StatusUpdateId, SupporterId, UserId,
};
