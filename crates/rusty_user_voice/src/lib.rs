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
pub use app::feedback::FeedbackRecordResult;
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

/// Initialises the process-wide `tracing_subscriber::fmt` layer exactly once,
/// routing logging to stderr (so stdout stays protocol-safe for both the MCP
/// and CLI binaries) and reading the `RUST_LOG` env filter when present.
///
/// Idempotent — safe to call from both `main` and tests; subsequent calls are
/// no-ops and return `Ok(())`. The first call's outcome (success or the
/// `try_init` error if a global subscriber was already installed elsewhere)
/// is reported back so the caller can decide whether to log a warning.
pub fn init_tracing(
    log_level: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    static INIT: std::sync::Once = std::sync::Once::new();
    let mut result: Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> = Ok(());
    INIT.call_once(|| {
        let filter = std::env::var("RUST_LOG")
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| log_level.map(String::from))
            .unwrap_or_else(|| "warn".to_owned());
        result = tracing_subscriber::fmt()
            .compact()
            .with_ansi(false)
            .with_target(false)
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .try_init();
    });
    result
}
