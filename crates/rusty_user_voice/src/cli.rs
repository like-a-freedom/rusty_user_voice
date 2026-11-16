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

impl OutputMode {
    /// Resolve the effective output mode: explicit flag, then the
    /// `USERVOICE_DEFAULT_OUTPUT` environment variable, then `default_output`
    /// in `config.yaml`, then human.
    ///
    /// Every command that runs before [`crate::Settings::from_sources`] needs
    /// this: `login`, `logout`, and `skills` all work without credentials, so
    /// they cannot load the settings first.
    pub fn resolve(
        flag: Option<OutputMode>,
        env_default: Option<String>,
        config_default: Option<String>,
    ) -> OutputMode {
        if let Some(mode) = flag {
            return mode;
        }
        let named = |value: &str| match value {
            "json" => Some(OutputMode::Json),
            "ndjson" => Some(OutputMode::Ndjson),
            _ => Some(OutputMode::Human),
        };
        env_default
            .as_deref()
            .and_then(named)
            .or_else(|| config_default.as_deref().and_then(named))
            .unwrap_or(OutputMode::Human)
    }

    /// Read the environment and the config file, then resolve the mode.
    pub fn resolve_now(flag: Option<OutputMode>) -> OutputMode {
        OutputMode::resolve(
            flag,
            std::env::var("USERVOICE_DEFAULT_OUTPUT")
                .ok()
                .filter(|value| !value.is_empty()),
            crate::config::config_default_output(),
        )
    }

    /// Whether the mode produces machine-readable output.
    pub fn is_machine(self) -> bool {
        matches!(self, OutputMode::Json | OutputMode::Ndjson)
    }
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
    /// Log in: save API key + base URL to ~/.config/rusty_user_voice/config.yaml.
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
    ///
    /// ```text
    /// uservoice-cli login
    /// uservoice-cli login --base-url https://acme.uservoice.com
    /// echo "$USERVOICE_API_KEY" | uservoice-cli --output json login --base-url https://acme.uservoice.com --api-key-stdin --force
    /// USERVOICE_API_KEY=key123 uservoice-cli login --base-url https://acme.uservoice.com --no-verify
    /// ```
    Login {
        /// Base URL (e.g. `https://acme.uservoice.com`).
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

    #[doc = "Log out: delete ~/.config/rusty_user_voice/config.yaml."]
    Logout {},

    /// Workspace context: the principal, the taxonomy, and the rate-limit envelope.
    ///
    /// Start here. Every Forum, Category, Status, and Label ID used later comes
    /// from this call.
    Workspace {
        /// Also load Forums, Categories, Statuses, and Labels.
        #[arg(long, default_value_t = true)]
        include_catalog: bool,
    },
    /// Idea workflows: search, read, collect, update, publish.
    Ideas {
        #[command(subcommand)]
        command: Box<IdeasCommand>,
    },
    /// Inbound feedback: record it with the channel and provenance it came from.
    Feedback {
        #[command(subcommand)]
        command: Box<FeedbackCommand>,
    },

    /// Agent-facing instructions compiled into this binary.
    ///
    /// The text always matches the installed version. Needs no credentials, so
    /// it works before `login`.
    Skills {
        #[command(subcommand)]
        command: Box<SkillsCommand>,
    },
}

#[derive(Debug, Subcommand)]
pub enum IdeasCommand {
    /// Find Ideas by text, time window, or taxonomy.
    Search(SearchArgs),
    /// Load one Idea with its full discussion.
    Get(GetArgs),
    /// Collect one resumable page of Ideas updated in a time window.
    Collect(CollectArgs),
    /// Change an Idea. Preview first, then apply.
    Update(UpdateArgs),
    /// Publish a Status Update to the Supporters. Preview first, then apply.
    Publish(PublishArgs),
}

#[derive(Debug, Subcommand)]
pub enum FeedbackCommand {
    /// Record one inbound Feedback Record with its provenance.
    Record(RecordFeedbackArgs),
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    /// List the bundled skills with a one-line description of each.
    ///
    /// Use the global --output json for a machine-readable list.
    List {},
    /// Print a skill, one of its references, or the whole corpus.
    Get {
        /// Skill name. Omit it together with --all.
        name: Option<String>,

        /// Print the skill and all of its references.
        #[arg(long, conflicts_with = "reference")]
        full: bool,

        /// Print one reference of the skill, for example `cli.md`.
        #[arg(long = "ref", value_name = "PATH")]
        reference: Option<String>,

        /// Print every bundled skill.
        #[arg(long, conflicts_with_all = ["name", "full", "reference"])]
        all: bool,
    },
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

#[cfg(test)]
mod tests {
    use super::OutputMode;

    #[test]
    fn the_explicit_flag_wins_over_the_environment_and_the_config() {
        let mode = OutputMode::resolve(
            Some(OutputMode::Ndjson),
            Some("human".to_owned()),
            Some("human".to_owned()),
        );
        assert!(mode.is_machine());
    }

    #[test]
    fn the_environment_wins_over_the_config() {
        let mode = OutputMode::resolve(None, Some("json".to_owned()), Some("human".to_owned()));
        assert_eq!(mode, OutputMode::Json);
        let mode = OutputMode::resolve(None, None, Some("json".to_owned()));
        assert_eq!(mode, OutputMode::Json);
    }

    #[test]
    fn human_and_unknown_values_are_prose() {
        assert_eq!(
            OutputMode::resolve(Some(OutputMode::Human), None, None),
            OutputMode::Human
        );
        assert_eq!(
            OutputMode::resolve(None, Some("human".to_owned()), None),
            OutputMode::Human
        );
        assert_eq!(
            OutputMode::resolve(None, None, Some("nonsense".to_owned())),
            OutputMode::Human
        );
        assert_eq!(OutputMode::resolve(None, None, None), OutputMode::Human);
    }

    #[test]
    fn only_json_and_ndjson_are_machine_modes() {
        assert!(OutputMode::Json.is_machine());
        assert!(OutputMode::Ndjson.is_machine());
        assert!(!OutputMode::Human.is_machine());
    }
}
