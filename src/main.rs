use slack_coder::agent::AgentManager;
use slack_coder::config::load_settings;
use slack_coder::error::Result;
use slack_coder::metadata::MetadataCache;
use slack_coder::slack::{EventHandler, ProgressTracker, SlackClient};
use slack_coder::storage::Workspace;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize rustls crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("slack_coder=debug,slack_morphism=debug")),
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
    tracing::info!("Slack client created");

    // Create metadata cache for enriched logging
    let metadata_cache = Arc::new(MetadataCache::new(slack_client.clone()));
    tracing::info!("Metadata cache initialized");

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
    tracing::info!("Agent manager created");

    // Scan Slack channels and restore agents
    tracing::info!("Scanning Slack channels");
    agent_manager
        .scan_and_restore_channels(&slack_client)
        .await?;
    tracing::info!("Channels scanned and agents restored");

    // Start event handler
    tracing::info!("Starting event handler (Socket Mode)");
    let event_handler = EventHandler::new(
        slack_client.clone(),
        agent_manager.clone(),
        metadata_cache.clone(),
    );

    // Clone references for shutdown handler
    let shutdown_agent_manager = agent_manager.clone();
    let shutdown_slack_client = slack_client.clone();

    // Setup shutdown signal handler in background
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<String>(1);
    tokio::spawn(async move {
        let signal_name = setup_shutdown_handler().await;
        let _ = shutdown_tx.send(signal_name).await;
    });

    // Run application with shutdown handling
    let shutdown_result = tokio::select! {
        result = event_handler.start() => {
            tracing::info!("Event handler completed normally");
            result
        }
        Some(signal_name) = shutdown_rx.recv() => {
            tracing::info!(
                signal = %signal_name,
                "Received shutdown signal, initiating graceful shutdown"
            );

            // Send shutdown notifications and cleanup agents
            shutdown_gracefully(&shutdown_agent_manager, &shutdown_slack_client).await;

            tracing::info!("Graceful shutdown complete");
            Ok(())
        }
    };

    // Ensure we wait for everything to complete
    tracing::info!("Application shutdown sequence complete");
    shutdown_result
}

/// Setup signal handlers for graceful shutdown
/// Handles SIGINT (Ctrl+C), SIGTERM, and SIGQUIT on Unix systems
async fn setup_shutdown_handler() -> String {
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

/// Gracefully shutdown the application
/// 1. Send shutdown notifications to all channels
/// 2. Disconnect all agents properly
async fn shutdown_gracefully(agent_manager: &Arc<AgentManager>, slack_client: &Arc<SlackClient>) {
    tracing::info!("Starting graceful shutdown sequence");

    // Step 1: Send shutdown notifications
    send_shutdown_notifications(agent_manager, slack_client).await;

    // Step 2: Disconnect all agents to cleanup resources
    disconnect_all_agents(agent_manager).await;

    tracing::info!("All cleanup tasks completed");
}

/// Send shutdown notifications to all active channels (in parallel)
/// This function will send all notifications concurrently with 5s total timeout
async fn send_shutdown_notifications(
    agent_manager: &Arc<AgentManager>,
    slack_client: &Arc<SlackClient>,
) {
    tracing::info!("Sending shutdown notifications to all channels");

    // Get all active agents
    let agents = agent_manager.get_all_active_agents().await;
    tracing::info!(agent_count = agents.len(), "Found active agents");

    if agents.is_empty() {
        tracing::info!("No active agents to notify");
        return;
    }

    // Send all shutdown notices in parallel
    let notification_futures: Vec<_> = agents
        .into_iter()
        .map(|(channel_id, session_id)| {
            let client = slack_client.clone();
            async move {
                let result = tokio::time::timeout(
                    Duration::from_secs(3),
                    client.send_shutdown_notice(&channel_id, &session_id),
                )
                .await;

                (channel_id, session_id, result)
            }
        })
        .collect();

    let total = notification_futures.len();
    tracing::info!(total = total, "Sending shutdown notices in parallel");

    // Execute all in parallel with overall 10s timeout (generous to ensure delivery)
    let results = tokio::time::timeout(
        Duration::from_secs(10),
        futures::future::join_all(notification_futures),
    )
    .await;

    // Count successes/failures
    let mut success_count = 0;
    let mut failure_count = 0;

    match results {
        Ok(results) => {
            for (channel_id, _session_id, result) in results {
                match result {
                    Ok(Ok(_)) => {
                        success_count += 1;
                        tracing::debug!(
                            channel_id = %channel_id.as_str(),
                            "Shutdown notice sent"
                        );
                    }
                    Ok(Err(e)) => {
                        failure_count += 1;
                        tracing::warn!(
                            channel_id = %channel_id.as_str(),
                            error = %e,
                            "Failed to send shutdown notice"
                        );
                    }
                    Err(_) => {
                        failure_count += 1;
                        tracing::warn!(
                            channel_id = %channel_id.as_str(),
                            "Timeout sending shutdown notice"
                        );
                    }
                }
            }
        }
        Err(_) => {
            tracing::warn!(
                timeout_secs = 10,
                "Overall shutdown notification timeout - messages may not have been delivered"
            );
            failure_count = total;
        }
    }

    let success_rate = if total > 0 {
        (success_count as f32 / total as f32 * 100.0) as u32
    } else {
        0
    };

    tracing::info!(
        succeeded = success_count,
        failed = failure_count,
        total = total,
        success_rate = success_rate,
        "Shutdown notification summary"
    );
}

/// Disconnect all agents to cleanup resources properly
async fn disconnect_all_agents(agent_manager: &Arc<AgentManager>) {
    tracing::info!("Disconnecting all agents");

    // We need to get the channels and remove agents one by one
    // because disconnect() consumes the agent (takes ownership)
    let agent_channels: Vec<_> = agent_manager
        .get_all_active_agents()
        .await
        .into_iter()
        .map(|(channel, _)| channel)
        .collect();

    let total = agent_channels.len();
    let mut disconnected = 0;

    for channel in agent_channels {
        tracing::debug!(
            channel_id = %channel.as_str(),
            "Disconnecting agent"
        );

        if let Err(e) = agent_manager.remove_agent(&channel).await {
            tracing::warn!(
                channel_id = %channel.as_str(),
                error = %e,
                "Failed to disconnect agent"
            );
        } else {
            disconnected += 1;
        }
    }

    tracing::info!(
        disconnected = disconnected,
        total = total,
        "Agent cleanup complete"
    );
}
