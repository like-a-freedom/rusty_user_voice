use anyhow::Context;
use clap::Parser;
use rusty_user_voice::{
    CheckpointStore, Cli, Settings, bootstrap::build_app, cli_runner::run_cli, init_tracing,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing().ok();
    let cli = Cli::parse();
    let settings = Settings::from_sources(
        cli.base_url.as_deref(),
        cli.state_dir.clone(),
        cli.max_in_flight,
    )
    .context("invalid UserVoice configuration")?;
    let app = build_app(&settings).context("failed to construct UserVoice client")?;
    let checkpoints = CheckpointStore::new(settings.state_dir.join("checkpoints"));
    let mut stdout = std::io::stdout().lock();
    run_cli(&app, &checkpoints, cli, &mut stdout)
        .await
        .context("UserVoice command failed")
}
