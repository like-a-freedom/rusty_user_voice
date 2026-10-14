use anyhow::Context;
use clap::Parser;
use rusty_user_voice::{
    CheckpointStore, Cli, Command, LoginParams, NEXT_COMMAND, OutputMode, Settings,
    bootstrap::build_app, cli_runner::run_cli, init_tracing, machine_mode_now, run_login,
    run_logout,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Auth commands run before `Settings::from_sources`: settings load fails
    // exactly when login is needed (no key/URL yet). Borrow + clone so the
    // `cli` value stays usable for the normal path below.
    if let Command::Login {
        base_url,
        api_key_stdin,
        ca_cert,
        force,
        no_verify,
    } = &cli.command
    {
        let machine = machine_mode_now(cli.output);
        match run_login(LoginParams {
            base_url: base_url.clone(),
            global_base_url: cli.base_url.clone(),
            key_stdin: *api_key_stdin,
            ca_cert: ca_cert.clone(),
            force: *force,
            no_verify: *no_verify,
        })
        .await
        {
            Ok(result) => {
                if machine {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "config_path": result.config_path,
                            "base_url": result.base_url,
                            "validated": result.validated,
                            "user_id": result.user_id,
                            "shadow_warning": result.shadow_warning,
                            "next": NEXT_COMMAND,
                        })
                    );
                } else {
                    println!("Saved to {} (mode 0600).", result.config_path.display());
                    if result.validated {
                        println!(
                            "Validated against {} (user id: {}).",
                            result.base_url,
                            result.user_id.unwrap_or_default()
                        );
                    } else {
                        println!("Saved without a server call (--no-verify).");
                    }
                    if let Some(warning) = result.shadow_warning {
                        println!("{warning}");
                    }
                    println!("Next: {NEXT_COMMAND}");
                }
                return Ok(());
            }
            Err(error) => error.report(machine),
        }
    }

    if matches!(cli.command, Command::Logout {}) {
        let machine = machine_mode_now(cli.output);
        match run_logout() {
            Ok(result) => {
                if machine {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "removed": result.removed,
                            "shadow_warning": result.shadow_warning,
                        })
                    );
                } else {
                    println!("Logged out (removed {}).", result.removed.display());
                    if let Some(warning) = result.shadow_warning {
                        println!("{warning}");
                    }
                }
                return Ok(());
            }
            Err(error) => error.report(machine),
        }
    }

    let mut cli = cli;
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
