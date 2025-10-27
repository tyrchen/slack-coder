use crate::agent::{MainAgent, RepoAgent};
use crate::config::Settings;
use crate::error::{Result, SlackCoderError};
use crate::slack::{ChannelId, ProgressTracker, SlackClient};
use crate::storage::Workspace;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct AgentManager {
    repo_agents: Arc<DashMap<ChannelId, Arc<Mutex<RepoAgent>>>>,
    workspace: Arc<Workspace>,
    settings: Arc<Settings>,
    progress_tracker: Arc<ProgressTracker>,
}

impl AgentManager {
    /// Create new agent manager with empty repo agent pool
    pub async fn new(
        settings: Arc<Settings>,
        workspace: Arc<Workspace>,
        progress_tracker: Arc<ProgressTracker>,
    ) -> Result<Self> {
        // Ensure workspace directories exist
        workspace.ensure_workspace().await?;

        Ok(Self {
            repo_agents: Arc::new(DashMap::new()),
            workspace,
            settings,
            progress_tracker,
        })
    }

    /// Scan Slack channels and restore existing agents from disk
    pub async fn scan_and_restore_channels(&self, slack_client: &SlackClient) -> Result<()> {
        tracing::info!("🔍 Scanning Slack channels for existing setups...");

        let channels = slack_client.list_channels().await?;
        tracing::info!("📊 Total channels to scan: {}", channels.len());

        let mut restored_count = 0;
        let mut skipped_count = 0;
        let mut failed_count = 0;

        for (idx, channel_id) in channels.iter().enumerate() {
            tracing::debug!(
                "  [{}/{}] Checking {}",
                idx + 1,
                channels.len(),
                channel_id.log_format()
            );

            if self.workspace.is_channel_setup(channel_id).await {
                tracing::info!("  ♻️  Found existing setup {}", channel_id.log_format());

                match self.create_repo_agent(channel_id.clone()).await {
                    Ok(agent) => {
                        self.repo_agents
                            .insert(channel_id.clone(), Arc::new(Mutex::new(agent)));
                        tracing::info!("  ✅ Agent restored {}", channel_id.log_format());
                        restored_count += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "  ⚠️  Failed to restore agent {}: {}",
                            channel_id.log_format(),
                            e
                        );
                        failed_count += 1;
                    }
                }
            } else {
                tracing::debug!("    {} not setup yet (skipping)", channel_id.log_format());
                skipped_count += 1;
            }
        }

        tracing::info!(
            "📈 Channel scan complete: restored={}, skipped={}, failed={}, total={}",
            restored_count,
            skipped_count,
            failed_count,
            channels.len()
        );
        Ok(())
    }

    /// Setup a new channel - invokes main agent to validate, clone, analyze, generate prompt
    pub async fn setup_channel(&self, channel_id: ChannelId, repo_name: String) -> Result<()> {
        use std::time::Instant;
        let total_start = Instant::now();

        tracing::info!(
            "🎬 Starting channel setup {} repo='{}'",
            channel_id.log_format(),
            repo_name
        );

        // Create and run main agent
        tracing::debug!("  Creating MainAgent...");
        let agent_start = Instant::now();
        let mut main_agent = MainAgent::new(
            self.settings.clone(),
            self.workspace.clone(),
            self.progress_tracker.clone(),
            channel_id.clone(),
        )
        .await?;
        tracing::debug!(
            "  ✅ MainAgent created (duration={:?})",
            agent_start.elapsed()
        );

        tracing::info!("  🔗 Connecting MainAgent to Claude...");
        let connect_start = Instant::now();
        main_agent.connect().await?;
        tracing::debug!(
            "  ✅ Connected to Claude (duration={:?})",
            connect_start.elapsed()
        );

        tracing::info!("  🚀 Running repository setup (this may take 1-2 minutes)...");
        let setup_start = Instant::now();
        main_agent.setup_repository(&repo_name, &channel_id).await?;
        tracing::info!(
            "  ✅ Repository setup completed (duration={:?})",
            setup_start.elapsed()
        );

        tracing::debug!("  🔌 Disconnecting MainAgent...");
        main_agent.disconnect().await?;

        // Create repository agent
        tracing::info!("  🤖 Creating RepoAgent {}...", channel_id.log_format());
        let create_start = Instant::now();
        let repo_agent = self.create_repo_agent(channel_id.clone()).await?;
        self.repo_agents
            .insert(channel_id.clone(), Arc::new(Mutex::new(repo_agent)));
        tracing::debug!(
            "  ✅ RepoAgent created (duration={:?})",
            create_start.elapsed()
        );

        tracing::info!(
            "✅ Channel setup complete {} repo='{}' (total_duration={:?})",
            channel_id.log_format(),
            repo_name,
            total_start.elapsed()
        );

        Ok(())
    }

    /// Create a new repository agent
    async fn create_repo_agent(&self, channel_id: ChannelId) -> Result<RepoAgent> {
        tracing::debug!(
            "    Creating RepoAgent instance {}...",
            channel_id.log_format()
        );

        let mut agent = RepoAgent::new(
            channel_id.clone(),
            self.workspace.clone(),
            self.settings.clone(),
            self.progress_tracker.clone(),
        )
        .await?;
        tracing::debug!("    ✅ RepoAgent instance created");

        tracing::debug!("    🔗 Connecting RepoAgent to Claude...");
        agent.connect().await?;
        tracing::debug!("    ✅ RepoAgent connected");

        // Get session ID and send startup notification
        let session_id = agent.get_session_id();
        tracing::info!(
            "    📋 Generated initial session_id={} for {}",
            session_id,
            channel_id.log_format()
        );

        let notification = format!(
            "🤖 *Agent Ready*\n\nSession ID: `{}`\n\nI'm ready to help with this repository! Type `/help` for available commands.",
            session_id
        );

        // Send startup notification
        let slack_client = self.progress_tracker.slack_client_ref();
        let send_result = slack_client
            .send_message(&channel_id, &notification, None)
            .await;

        if let Err(e) = send_result {
            tracing::warn!(
                "    ⚠️  Failed to send startup notification {}: {}",
                channel_id.log_format(),
                e
            );
        } else {
            tracing::debug!("    ✅ Startup notification sent");
        }

        Ok(agent)
    }

    /// Get repository agent for a channel
    pub async fn get_repo_agent(&self, channel_id: &ChannelId) -> Result<Arc<Mutex<RepoAgent>>> {
        self.repo_agents
            .get(channel_id)
            .map(|r| r.clone())
            .ok_or_else(|| {
                SlackCoderError::AgentNotFound(format!(
                    "No agent found for channel {}",
                    channel_id.as_str()
                ))
            })
    }

    /// Remove agent for a channel
    pub async fn remove_agent(&self, channel_id: &ChannelId) -> Result<()> {
        if let Some((_, agent_mutex)) = self.repo_agents.remove(channel_id) {
            // Try to unwrap and disconnect if we have sole ownership
            if let Ok(mutex) = Arc::try_unwrap(agent_mutex) {
                let agent = mutex.into_inner();
                agent.disconnect().await?;
            }
        }
        Ok(())
    }

    /// Cleanup inactive agents (background task)
    pub async fn cleanup_inactive_agents(&self) -> Result<()> {
        let timeout = Duration::from_secs(self.settings.agent.agent_timeout_secs);
        let mut to_remove = Vec::new();

        for entry in self.repo_agents.iter() {
            let agent = entry.value().lock().await;
            if agent.is_expired(timeout) {
                to_remove.push(entry.key().clone());
            }
        }

        for channel_id in to_remove {
            tracing::info!("Removing expired agent for channel {}", channel_id.as_str());
            self.remove_agent(&channel_id).await?;
        }

        Ok(())
    }

    /// Check if channel has a configured agent
    pub fn has_agent(&self, channel_id: &ChannelId) -> bool {
        self.repo_agents.contains_key(channel_id)
    }
}
