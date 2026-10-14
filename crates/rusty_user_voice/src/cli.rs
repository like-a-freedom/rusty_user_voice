use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use uservoice_client::{CategoryId, ForumId, IdeaId, LabelId, StatusId};

use crate::DetailLevel;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputMode {
    Human,
    Json,
    Ndjson,
}

#[derive(Debug, Parser)]
#[command(name = "uservoice-cli", version, about = "Local UserVoice PM CLI")]
pub struct Cli {
    #[arg(long, env = "USERVOICE_BASE_URL")]
    pub base_url: Option<String>,
    #[arg(long, env = "USERVOICE_STATE_DIR")]
    pub state_dir: Option<PathBuf>,
    #[arg(
        long,
        env = "USERVOICE_MAX_IN_FLIGHT",
        value_parser = clap::value_parser!(u32).range(1..=8),
    )]
    pub max_in_flight: Option<u32>,
    #[arg(long, value_enum)]
    pub output: Option<OutputMode>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Log in: save API key + base URL to ~/.config/rusty-user-voice/config.yaml.
    ///
    /// Prompts for a missing base URL / API key (TTY only, key input hidden).
    /// Headless and agents: pass --base-url with --api-key-stdin (pipe) or the
    /// key env var. There is NO --api-key flag (Credential Boundary: argv is
    /// visible in ps/history). Validates offline always, and via users/current
    /// unless --no-verify. Refuses to overwrite an existing config without
    /// --force (exit 3). Exit codes: 0 ok, 2 usage, 3 needs --force, 1 other
    /// failures.
    ///
    /// Examples:
    ///   uservoice-cli login
    ///   uservoice-cli login --base-url https://acme.uservoice.com
    ///   echo "$USERVOICE_API_KEY" | uservoice-cli --output json login --base-url https://acme.uservoice.com --api-key-stdin --force
    ///   USERVOICE_API_KEY=key123 uservoice-cli login --base-url https://acme.uservoice.com --no-verify
    Login {
        /// Base URL (e.g. https://acme.uservoice.com).
        /// Falls back to the top-level --base-url, then USERVOICE_BASE_URL env, then prompts (TTY only).
        #[arg(long)]
        base_url: Option<String>,

        /// Read the API key from stdin (e.g. `echo "$USERVOICE_API_KEY" | uservoice-cli login --api-key-stdin ...`).
        /// Preferred over env when env is unavailable. There is no --api-key flag by design.
        #[arg(long)]
        api_key_stdin: bool,

        /// Path to a custom CA certificate (PEM) for corporate TLS (saved as ca_cert).
        #[arg(long)]
        ca_cert: Option<String>,

        /// Overwrite an existing config without error (its non-auth keys are preserved).
        #[arg(long)]
        force: bool,

        /// Skip the server call. Local validation (URL, key, ca_cert) still runs.
        #[arg(long)]
        no_verify: bool,
    },

    #[doc = "Log out: delete ~/.config/rusty-user-voice/config.yaml."]
    Logout {},

    Workspace {
        #[arg(long, default_value_t = true)]
        include_catalog: bool,
    },
    Ideas {
        #[command(subcommand)]
        command: Box<IdeasCommand>,
    },
    Feedback {
        #[command(subcommand)]
        command: Box<FeedbackCommand>,
    },
}

#[derive(Debug, Subcommand)]
pub enum IdeasCommand {
    Search(SearchArgs),
    Get(GetArgs),
    Collect(CollectArgs),
    Update(UpdateArgs),
    Publish(PublishArgs),
}

#[derive(Debug, Subcommand)]
pub enum FeedbackCommand {
    Record(RecordFeedbackArgs),
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[arg(long)]
    pub query: Option<String>,
    #[arg(long)]
    pub updated_after: Option<DateTime<Utc>>,
    #[arg(long)]
    pub updated_before: Option<DateTime<Utc>>,
    #[arg(long, value_delimiter = ',')]
    pub forum_ids: Vec<i64>,
    #[arg(long, value_delimiter = ',')]
    pub category_ids: Vec<i64>,
    #[arg(long, value_delimiter = ',')]
    pub status_ids: Vec<i64>,
    #[arg(long, value_delimiter = ',')]
    pub label_ids: Vec<i64>,
    #[arg(long, value_delimiter = ',')]
    pub states: Vec<String>,
    #[arg(long)]
    pub sort: Option<String>,
    #[arg(
        long,
        default_value_t = 100,
        value_parser = clap::value_parser!(u32).range(1..=100)
    )]
    pub per_page: u32,
    #[arg(long, conflicts_with = "page")]
    pub cursor: Option<String>,
    #[arg(long, conflicts_with = "cursor")]
    pub page: Option<u32>,
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    pub idea_id: i64,
    #[arg(long, value_enum, default_value = "full")]
    pub detail_level: CliDetailLevel,
    #[arg(long, default_value_t = false)]
    pub include_supporters: bool,
    #[arg(
        long,
        default_value_t = 100,
        value_parser = clap::value_parser!(u32).range(1..=100)
    )]
    pub per_page: u32,
}

#[derive(Debug, Args)]
pub struct CollectArgs {
    #[arg(long)]
    pub updated_after: DateTime<Utc>,
    #[arg(long)]
    pub updated_before: Option<DateTime<Utc>>,
    #[arg(long)]
    pub query: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub forum_ids: Vec<i64>,
    #[arg(long, value_delimiter = ',')]
    pub category_ids: Vec<i64>,
    #[arg(long, value_delimiter = ',')]
    pub status_ids: Vec<i64>,
    #[arg(long, value_delimiter = ',')]
    pub label_ids: Vec<i64>,
    #[arg(long, value_delimiter = ',')]
    pub states: Vec<String>,
    #[arg(long)]
    pub sort: Option<String>,
    #[arg(long, value_enum, default_value = "summary")]
    pub detail_level: CliDetailLevel,
    #[arg(
        long,
        default_value_t = 20,
        value_parser = clap::value_parser!(u32).range(1..=100)
    )]
    pub per_page: u32,
    #[arg(long, conflicts_with = "page")]
    pub cursor: Option<String>,
    #[arg(long, conflicts_with = "cursor")]
    pub page: Option<u32>,
    #[arg(long)]
    pub checkpoint_name: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliDetailLevel {
    Summary,
    Full,
}

impl From<CliDetailLevel> for DetailLevel {
    fn from(value: CliDetailLevel) -> Self {
        match value {
            CliDetailLevel::Summary => Self::Summary,
            CliDetailLevel::Full => Self::Full,
        }
    }
}

pub fn continuation(
    cursor: Option<String>,
    page: Option<u32>,
) -> Option<uservoice_client::Continuation> {
    cursor
        .map(|cursor| uservoice_client::Continuation::Cursor { cursor })
        .or_else(|| page.map(|page| uservoice_client::Continuation::Page { page }))
}

pub fn forum_ids(values: Vec<i64>) -> Vec<ForumId> {
    values.into_iter().map(ForumId::new).collect()
}

pub fn category_ids(values: Vec<i64>) -> Vec<CategoryId> {
    values.into_iter().map(CategoryId::new).collect()
}

pub fn status_ids(values: Vec<i64>) -> Vec<StatusId> {
    values.into_iter().map(StatusId::new).collect()
}

pub fn label_ids(values: Vec<i64>) -> Vec<LabelId> {
    values.into_iter().map(LabelId::new).collect()
}

pub fn idea_id(value: i64) -> IdeaId {
    IdeaId::new(value)
}

pub fn intent_names() -> Vec<&'static str> {
    crate::ToolName::ALL
        .iter()
        .copied()
        .map(crate::ToolName::as_str)
        .collect()
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    pub idea_id: i64,
    #[arg(long, value_enum)]
    pub mode: CliMutationMode,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub body: Option<String>,
    #[arg(long)]
    pub forum_id: Option<i64>,
    #[arg(long)]
    pub category_id: Option<i64>,
    #[arg(long)]
    pub assigned_owner_id: Option<i64>,
    #[arg(long, value_delimiter = ',')]
    pub add_label_ids: Vec<i64>,
    #[arg(long, value_delimiter = ',')]
    pub remove_label_ids: Vec<i64>,
    #[arg(long)]
    pub internal_note: Option<String>,
    #[arg(long)]
    pub change_id: Option<uuid::Uuid>,
    #[arg(long)]
    pub confirmation: Option<String>,
    #[arg(long)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Args)]
pub struct PublishArgs {
    pub idea_id: i64,
    #[arg(long, value_enum)]
    pub mode: CliMutationMode,
    #[arg(long)]
    pub status_id: i64,
    #[arg(long)]
    pub message: String,
    #[arg(long, default_value_t = false)]
    pub notify_supporters: bool,
    #[arg(long)]
    pub change_id: Option<uuid::Uuid>,
    #[arg(long)]
    pub confirmation: Option<String>,
    #[arg(long)]
    pub idempotency_key: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliMutationMode {
    Preview,
    Apply,
}

impl From<CliMutationMode> for crate::MutationMode {
    fn from(value: CliMutationMode) -> Self {
        match value {
            CliMutationMode::Preview => Self::Preview,
            CliMutationMode::Apply => Self::Apply,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliFeedbackChannel {
    Admin,
    Api,
    Bookmarklet,
    Gainsight,
    Import,
    Portal,
    Roadmap,
    Slack,
    Zendesk,
}

impl From<CliFeedbackChannel> for crate::FeedbackChannel {
    fn from(value: CliFeedbackChannel) -> Self {
        match value {
            CliFeedbackChannel::Admin => Self::Admin,
            CliFeedbackChannel::Api => Self::Api,
            CliFeedbackChannel::Bookmarklet => Self::Bookmarklet,
            CliFeedbackChannel::Gainsight => Self::Gainsight,
            CliFeedbackChannel::Import => Self::Import,
            CliFeedbackChannel::Portal => Self::Portal,
            CliFeedbackChannel::Roadmap => Self::Roadmap,
            CliFeedbackChannel::Slack => Self::Slack,
            CliFeedbackChannel::Zendesk => Self::Zendesk,
        }
    }
}

#[derive(Debug, Args)]
pub struct RecordFeedbackArgs {
    #[arg(long)]
    pub body: String,
    #[arg(long, value_enum, default_value = "api")]
    pub channel: CliFeedbackChannel,
    #[arg(long)]
    pub idea_id: Option<i64>,
    #[arg(long)]
    pub user_id: Option<i64>,
    #[arg(long, conflicts_with = "user_id")]
    pub user_email: Option<String>,
    #[arg(long)]
    pub source_url: Option<String>,
    #[arg(long)]
    pub source_guid: Option<String>,
    #[arg(long)]
    pub created_at: Option<DateTime<Utc>>,
    #[arg(long)]
    pub idempotency_key: String,
}
