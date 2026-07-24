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
    #[arg(long, env = "USERVOICE_MAX_IN_FLIGHT")]
    pub max_in_flight: Option<usize>,
    #[arg(long, value_enum, default_value = "human")]
    pub output: OutputMode,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Workspace {
        #[arg(long, default_value_t = true)]
        include_catalog: bool,
    },
    Ideas {
        #[command(subcommand)]
        command: Box<IdeasCommand>,
    },
}

#[derive(Debug, Subcommand)]
pub enum IdeasCommand {
    Search(SearchArgs),
    Get(GetArgs),
    Collect(CollectArgs),
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
