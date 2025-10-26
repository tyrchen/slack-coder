use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting Slack Coder Bot");

    // TODO: Load configuration
    // TODO: Create workspace
    // TODO: Create agent manager
    // TODO: Create Slack client
    // TODO: Scan and restore channels
    // TODO: Start event handler

    Ok(())
}
