#![deny(unsafe_code)]
//! UserVoice agent workflows with MCP and CLI adapters.

pub mod app;
pub mod checkpoint;
pub mod contracts;
pub mod error;

pub use app::App;
pub use contracts::{
    CollectIdeasParams, DetailLevel, Envelope, GetIdeaContextParams, IdeaContext, IdeaPage,
    NextAction, OutcomeStatus, SearchIdeasParams, WorkspaceContext, WorkspaceContextParams,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
