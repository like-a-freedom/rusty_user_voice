use anyhow::Context;
use rmcp::ServiceExt;
use rusty_user_voice::{Settings, UserVoiceMcpServer, bootstrap::build_app};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_ansi(false)
        .with_target(false)
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init()
        .ok();
    let settings =
        Settings::from_sources(None, None, None).context("invalid UserVoice configuration")?;
    let app = build_app(&settings).context("failed to construct UserVoice client")?;
    UserVoiceMcpServer::new(app)
        .serve(rmcp::transport::io::stdio())
        .await
        .context("failed to start MCP stdio transport")?
        .waiting()
        .await
        .context("MCP stdio transport failed")?;
    Ok(())
}
