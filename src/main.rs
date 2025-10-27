use slack_coder::agent::AgentManager;
use slack_coder::config::load_settings;
use slack_coder::error::Result;
use slack_coder::slack::{EventHandler, ProgressTracker, SlackClient};
use slack_coder::storage::Workspace;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize rustls crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("slack_coder=info,slack_morphism=info")),
        )
        .with_target(true)
        .with_line_number(true)
        .init();

    tracing::info!("🚀 Starting Slack Coder Bot");

    // Load configuration
    let settings = Arc::new(load_settings()?);
    tracing::info!("✅ Configuration loaded");
    tracing::debug!(
        "Config: model={}, workspace={:?}",
        settings.claude.model,
        settings.workspace.base_path
    );

    // Create workspace
    let workspace = Arc::new(Workspace::new(settings.workspace.base_path.clone()));
    workspace.ensure_workspace().await?;
    tracing::info!(
        "✅ Workspace initialized at {:?}",
        settings.workspace.base_path
    );

    // Create Slack client
    let slack_client = Arc::new(SlackClient::new(settings.slack.clone())?);
    tracing::info!("✅ Slack client created");

    // Create progress tracker
    let progress_tracker = Arc::new(ProgressTracker::new(slack_client.clone()));
    tracing::debug!("Progress tracker initialized");

    // Create agent manager
    let agent_manager = Arc::new(
        AgentManager::new(
            settings.clone(),
            workspace.clone(),
            progress_tracker.clone(),
        )
        .await?,
    );
    tracing::info!("✅ Agent manager created");

    // Scan Slack channels and restore agents
    tracing::info!("🔍 Scanning Slack channels...");
    agent_manager
        .scan_and_restore_channels(&slack_client)
        .await?;
    tracing::info!("✅ Channels scanned and agents restored");

    // Start event handler
    tracing::info!("🎧 Starting event handler (Socket Mode)...");
    let event_handler = EventHandler::new(slack_client.clone(), agent_manager.clone());

    event_handler.start().await?;

    Ok(())
}
