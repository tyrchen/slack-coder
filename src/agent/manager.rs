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

    /// Scan Slack channels and restore existing agents from disk (in parallel)
    pub async fn scan_and_restore_channels(&self, slack_client: &SlackClient) -> Result<()> {
        let span = tracing::info_span!("scan_and_restore_channels");
        let _guard = span.enter();

        let start = std::time::Instant::now();
        let channels = slack_client.list_channels().await?;

        tracing::info!(
            total_channels = channels.len(),
            "Scanning for existing setups"
        );

        // Filter to channels that are setup
        let mut setup_channels = Vec::new();
        for channel_id in channels {
            if self.workspace.is_channel_setup(&channel_id).await {
                setup_channels.push(channel_id);
            }
        }

        tracing::info!(
            setup_count = setup_channels.len(),
            "Found channels with existing setup"
        );

        if setup_channels.is_empty() {
            tracing::info!("No channels to restore");
            return Ok(());
        }

        // Restore all agents in parallel
        let restore_futures: Vec<_> = setup_channels
            .into_iter()
            .map(|channel_id| {
                let workspace = self.workspace.clone();
                let settings = self.settings.clone();
                let progress_tracker = self.progress_tracker.clone();

                async move {
                    Self::create_repo_agent_static(
                        channel_id.clone(),
                        workspace,
                        settings,
                        progress_tracker,
                    )
                    .await
                    .map(|agent| (channel_id.clone(), agent))
                    .map_err(|e| (channel_id, e))
                }
            })
            .collect();

        tracing::info!(
            agent_count = restore_futures.len(),
            "Restoring agents in parallel"
        );

        let results = futures::future::join_all(restore_futures).await;

        // Process results
        let mut restored_count = 0;
        let mut failed_count = 0;

        for result in results {
            match result {
                Ok((channel_id, agent)) => {
                    self.repo_agents
                        .insert(channel_id.clone(), Arc::new(Mutex::new(agent)));
                    restored_count += 1;
                    tracing::debug!(
                        channel_id = %channel_id.as_str(),
                        "Agent restored"
                    );
                }
                Err((channel_id, e)) => {
                    failed_count += 1;
                    tracing::warn!(
                        channel_id = %channel_id.as_str(),
                        error = %e,
                        "Failed to restore agent"
                    );
                }
            }
        }

        let duration = start.elapsed();
        tracing::info!(
            restored = restored_count,
            failed = failed_count,
            duration_ms = duration.as_millis() as u64,
            "Agent restoration complete"
        );

        // Send startup notification to all restored channels in parallel
        if restored_count > 0 {
            self.send_startup_notifications().await;
        }

        Ok(())
    }

    /// Send startup notifications to all channels with restored agents (in parallel)
    async fn send_startup_notifications(&self) {
        tracing::info!("Sending startup notifications to restored channels");

        // Collect channel IDs and session IDs
        let mut channel_sessions = Vec::new();
        for entry in self.repo_agents.iter() {
            let channel_id = entry.key().clone();

            // Try to get session ID
            if let Ok(agent) =
                tokio::time::timeout(Duration::from_millis(100), entry.value().lock()).await
            {
                channel_sessions.push((channel_id, agent.get_session_id()));
            }
        }

        tracing::debug!(
            channel_count = channel_sessions.len(),
            "Prepared startup notifications"
        );

        // Send notifications in parallel
        let slack_client = self.progress_tracker.slack_client_ref();
        let notification_futures: Vec<_> = channel_sessions
            .into_iter()
            .map(|(channel_id, session_id)| {
                let client = slack_client.clone();
                async move {
                    let notification = format!(
                        "🤖 *Agent Ready*\n\nSession ID: `{}`\n\nI'm ready to help with this repository! Type `/help` for available commands.",
                        session_id
                    );

                    match client.send_message(&channel_id, &notification, None).await {
                        Ok(_) => {
                            tracing::debug!(
                                channel_id = %channel_id.as_str(),
                                session_id = %session_id,
                                "Startup notification sent"
                            );
                            Ok(())
                        }
                        Err(e) => {
                            tracing::warn!(
                                channel_id = %channel_id.as_str(),
                                error = %e,
                                "Failed to send startup notification"
                            );
                            Err(e)
                        }
                    }
                }
            })
            .collect();

        let results = futures::future::join_all(notification_futures).await;

        let success_count = results.iter().filter(|r| r.is_ok()).count();
        tracing::info!(
            sent = success_count,
            total = results.len(),
            "Startup notifications sent"
        );
    }

    /// Setup a new channel - invokes main agent to validate, clone, analyze, generate prompt
    pub async fn setup_channel(&self, channel_id: ChannelId, repo_name: String) -> Result<()> {
        tracing::info!(
            "🎬 Setting up {} repo={}",
            channel_id.log_format(),
            repo_name
        );

        // Create and run main agent
        tracing::debug!("Creating main agent...");
        let mut main_agent = MainAgent::new(
            self.settings.clone(),
            self.workspace.clone(),
            self.progress_tracker.clone(),
            channel_id.clone(),
        )
        .await?;
        tracing::info!("✅ Main agent created");

        tracing::info!("🔗 Connecting main agent to Claude...");
        main_agent.connect().await?;
        tracing::info!("✅ Connected to Claude");

        tracing::info!("🚀 Running repository setup (this may take 1-2 minutes)...");
        main_agent.setup_repository(&repo_name, &channel_id).await?;
        tracing::info!("✅ Repository setup completed");

        tracing::debug!("Disconnecting main agent...");
        main_agent.disconnect().await?;

        // Create repository agent
        tracing::info!(
            "🤖 Creating repository-specific agent {}...",
            channel_id.log_format()
        );
        let repo_agent = self.create_repo_agent(channel_id.clone()).await?;
        self.repo_agents
            .insert(channel_id.clone(), Arc::new(Mutex::new(repo_agent)));
        tracing::info!(
            "✅ Repository agent created and cached {}",
            channel_id.log_format()
        );

        Ok(())
    }

    /// Create a new repository agent (instance method)
    async fn create_repo_agent(&self, channel_id: ChannelId) -> Result<RepoAgent> {
        Self::create_repo_agent_static(
            channel_id,
            self.workspace.clone(),
            self.settings.clone(),
            self.progress_tracker.clone(),
        )
        .await
    }

    /// Create a new repository agent (static method for parallel execution)
    async fn create_repo_agent_static(
        channel_id: ChannelId,
        workspace: Arc<Workspace>,
        settings: Arc<Settings>,
        progress_tracker: Arc<ProgressTracker>,
    ) -> Result<RepoAgent> {
        tracing::debug!(
            channel_id = %channel_id.as_str(),
            "Creating repo agent"
        );

        let mut agent =
            RepoAgent::new(channel_id.clone(), workspace, settings, progress_tracker).await?;

        tracing::debug!(
            channel_id = %channel_id.as_str(),
            "Connecting agent to Claude"
        );
        agent.connect().await?;

        tracing::debug!(
            channel_id = %channel_id.as_str(),
            session_id = %agent.get_session_id(),
            "Agent connected"
        );

        // NO per-channel startup notification sent here

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

    /// Get all active agents and their session IDs
    /// Returns a list of (channel_id, session_id) tuples
    pub async fn get_all_active_agents(&self) -> Vec<(ChannelId, String)> {
        let mut result = Vec::new();

        for entry in self.repo_agents.iter() {
            let channel_id = entry.key().clone();

            // Try to lock with short timeout
            if let Ok(agent) =
                tokio::time::timeout(Duration::from_millis(100), entry.value().lock()).await
            {
                let session_id = agent.get_session_id();
                result.push((channel_id, session_id));
            }
        }

        result
    }
}
