use anyhow::Context;
use clap::Parser;
use rusty_user_voice::{
    CheckpointStore, Cli, OutputMode, Settings, bootstrap::build_app, cli_runner::run_cli,
    init_tracing,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut cli = Cli::parse();
    let settings = Settings::from_sources(
        cli.base_url.as_deref(),
        cli.state_dir.clone(),
        cli.max_in_flight,
    )
    .context("invalid UserVoice configuration")?;
    init_tracing(settings.log_level.as_deref()).ok();

    // Resolve output mode: CLI flag > env var (via clap) > config.yaml > default
    if cli.output.is_none() {
        cli.output = settings
            .default_output
            .as_deref()
            .and_then(|s| match s {
                "human" => Some(OutputMode::Human),
                "json" => Some(OutputMode::Json),
                "ndjson" => Some(OutputMode::Ndjson),
                _ => None,
            })
            .or(Some(OutputMode::Human));
    }

    let app = build_app(&settings).context("failed to construct UserVoice client")?;
    let checkpoints = CheckpointStore::new(settings.state_dir.join("checkpoints"));
    let mut stdout = std::io::stdout().lock();
    run_cli(&app, &checkpoints, cli, &mut stdout)
        .await
        .context("UserVoice command failed")
}
