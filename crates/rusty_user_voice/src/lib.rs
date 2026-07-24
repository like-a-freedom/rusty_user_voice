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
pub mod ledger;
pub mod mcp;

pub use app::App;
pub use checkpoint::{Checkpoint, CheckpointStore};
pub use cli::{Cli, Command, IdeasCommand, OutputMode};
pub use config::Settings;
pub use contracts::{
    AppliedOperation, ChangeId, CollectIdeasParams, DetailLevel, Envelope, FeedbackChannel,
    FieldChange, GetIdeaContextParams, IdeaContext, IdeaPage, MutationMode, MutationResult,
    NextAction, OutcomeStatus, PublishIdeaUpdateParams, RecordFeedbackParams, SearchIdeasParams,
    ToolName, UpdateIdeaParams, WorkspaceContext, WorkspaceContextParams,
};
pub use ledger::{DiskChangeLedger, FixedClock, PreviewKind, PreviewRecord};
pub use mcp::UserVoiceMcpServer;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
