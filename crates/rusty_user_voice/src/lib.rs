#![deny(unsafe_code)]
//! UserVoice agent workflows with MCP and CLI adapters.

pub mod app;
pub mod bootstrap;
pub mod checkpoint;
pub mod cli;
pub mod cli_runner;
pub mod config;
pub mod contracts;
pub mod error;

pub use app::App;
pub use checkpoint::{Checkpoint, CheckpointStore};
pub use cli::{Cli, Command, IdeasCommand, OutputMode};
pub use config::Settings;
pub use contracts::{
    CollectIdeasParams, DetailLevel, Envelope, GetIdeaContextParams, IdeaContext, IdeaPage,
    NextAction, OutcomeStatus, SearchIdeasParams, WorkspaceContext, WorkspaceContextParams,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
