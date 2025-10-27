use slack_coder::agent::AgentManager;
use slack_coder::config::load_settings;
use slack_coder::error::Result;
use slack_coder::slack::{EventHandler, ProgressTracker, SlackClient};
use slack_coder::storage::Workspace;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::Notify;
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

    // Setup shutdown signal handler
    let shutdown_notify = Arc::new(Notify::new());
    let shutdown_signal = setup_shutdown_handler(shutdown_notify.clone());

    // Clone references for shutdown handler
    let shutdown_agent_manager = agent_manager.clone();
    let shutdown_slack_client = slack_client.clone();

    // Run application with shutdown handling
    tokio::select! {
        result = event_handler.start() => {
            tracing::info!("Event handler completed normally");
            result?;
        }
        signal_name = shutdown_signal => {
            tracing::info!("🛑 Received {} signal, initiating graceful shutdown...", signal_name);

            // Send shutdown notifications and wait for completion
            send_shutdown_notifications(&shutdown_agent_manager, &shutdown_slack_client).await;

            tracing::info!("👋 Graceful shutdown complete");
        }
    }

    Ok(())
}

/// Setup signal handlers for graceful shutdown
/// Handles SIGINT (Ctrl+C), SIGTERM, and SIGQUIT on Unix systems
async fn setup_shutdown_handler(_shutdown_notify: Arc<Notify>) -> String {
    #[cfg(unix)]
    {
        use signal::unix::{SignalKind, signal};

        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to setup SIGINT handler");
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");
        let mut sigquit = signal(SignalKind::quit()).expect("Failed to setup SIGQUIT handler");

        tokio::select! {
            _ = sigint.recv() => {
                tracing::debug!("Caught SIGINT signal");
                "SIGINT (Ctrl+C)".to_string()
            }
            _ = sigterm.recv() => {
                tracing::debug!("Caught SIGTERM signal");
                "SIGTERM".to_string()
            }
            _ = sigquit.recv() => {
                tracing::debug!("Caught SIGQUIT signal");
                "SIGQUIT".to_string()
            }
        }
    }

    #[cfg(not(unix))]
    {
        // On Windows, only handle Ctrl+C
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        tracing::debug!("Caught Ctrl+C signal");
        "Ctrl+C".to_string()
    }
}

/// Send shutdown notifications to all active channels
/// This function will block until all notifications are sent or timeout (10s)
async fn send_shutdown_notifications(
    agent_manager: &Arc<AgentManager>,
    slack_client: &Arc<SlackClient>,
) {
    tracing::info!("📢 Sending shutdown notifications to all active channels...");

    // Get all active agents
    let agents = agent_manager.get_all_active_agents().await;
    tracing::info!("Found {} active agent(s) to notify", agents.len());

    if agents.is_empty() {
        tracing::info!("No active agents to notify");
        return;
    }

    // Send notifications sequentially to ensure delivery
    let mut success_count = 0;
    let mut failure_count = 0;
    let total_agents = agents.len();

    for (channel_id, session_id) in agents {
        tracing::info!(
            "📤 Sending shutdown notice {} session={} ({}/{})",
            channel_id.log_format(),
            session_id,
            success_count + failure_count + 1,
            total_agents
        );

        // Try to send with 3-second timeout per message
        match tokio::time::timeout(
            Duration::from_secs(3),
            slack_client.send_shutdown_notice(&channel_id, &session_id),
        )
        .await
        {
            Ok(Ok(_)) => {
                success_count += 1;
                tracing::info!("✅ Shutdown notice sent {}", channel_id.log_format());
            }
            Ok(Err(e)) => {
                failure_count += 1;
                tracing::warn!(
                    "❌ Failed to send shutdown notice {}: {}",
                    channel_id.log_format(),
                    e
                );
            }
            Err(_) => {
                failure_count += 1;
                tracing::warn!(
                    "⏱️  Timeout sending shutdown notice {} (3s exceeded)",
                    channel_id.log_format()
                );
            }
        }
    }

    tracing::info!(
        "📊 Shutdown notification summary: {} succeeded, {} failed out of {} total",
        success_count,
        failure_count,
        total_agents
    );

    if success_count == total_agents {
        tracing::info!("✅ All shutdown notifications delivered successfully");
    } else if success_count > 0 {
        tracing::warn!(
            "⚠️  Partial delivery: {}/{} notifications sent",
            success_count,
            total_agents
        );
    } else {
        tracing::error!("❌ Failed to send any shutdown notifications");
    }
}
