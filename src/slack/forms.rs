use crate::agent::AgentManager;
use crate::error::{Result, SlackCoderError};
use crate::slack::{ChannelId, SlackClient};
use std::sync::Arc;

pub struct FormHandler {
    slack_client: Arc<SlackClient>,
    pub agent_manager: Arc<AgentManager>,
}

impl FormHandler {
    pub fn new(slack_client: Arc<SlackClient>, agent_manager: Arc<AgentManager>) -> Self {
        Self {
            slack_client,
            agent_manager,
        }
    }

    /// Show repository setup prompt (simple text for now, can be upgraded to Block Kit form)
    pub async fn show_repo_setup_form(&self, channel: &ChannelId) -> Result<()> {
        let message = r#"Welcome to Slack Coder Bot! 👋

To get started, please provide your GitHub repository in the format: `owner/repo-name`

For example: `tyrchen/rust-lib-template`

Reply with your repository name to begin setup."#;

        self.slack_client
            .send_message(channel, message, None)
            .await?;

        Ok(())
    }

    /// Handle repository setup from user message
    pub async fn handle_repo_setup(&self, channel: ChannelId, repo_name: String) -> Result<()> {
        tracing::info!("🔧 Starting repository setup");
        tracing::info!("  Channel: {}", channel.as_str());
        tracing::info!("  Repository: {}", repo_name);

        // Validate repo name format
        let (owner, repo) = Self::validate_repo_name_format(&repo_name)?;
        tracing::debug!("✅ Validated format: owner={}, repo={}", owner, repo);

        // Send acknowledgment
        tracing::debug!("Sending acknowledgment to Slack...");
        self.slack_client
            .send_message(
                &channel,
                &format!("🔧 Setting up repository `{}`...\nThis may take a minute. I'll update you on progress.", repo_name),
                None,
            )
            .await?;
        tracing::info!("✅ Acknowledgment sent");

        // Trigger setup via agent manager
        tracing::info!("🚀 Invoking agent manager to setup channel...");
        self.agent_manager
            .setup_channel(channel.clone(), repo_name.clone())
            .await?;
        tracing::info!("✅ Agent setup completed");

        // Send completion message with proper formatting
        tracing::debug!("Sending completion message...");
        let completion_msg = format!(
            ":white_check_mark: *Repository `{}` is now ready!*\n\n\
            You can now ask me to:\n\
            • Generate code\n\
            • Write documentation\n\
            • Refactor existing code\n\
            • Review and commit changes\n\
            • Create pull requests\n\n\
            Try: `@slack-coder /help` for more information",
            repo_name
        );

        self.slack_client
            .send_message(&channel, &completion_msg, None)
            .await?;
        tracing::info!("🎉 Setup workflow completed successfully");

        Ok(())
    }

    /// Validate repository name format (owner/repo)
    fn validate_repo_name_format(name: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = name.split('/').collect();

        if parts.len() != 2 {
            return Err(SlackCoderError::Config(format!(
                "Invalid repository format: '{}'. Expected format: owner/repo-name",
                name
            )));
        }

        let owner = parts[0].trim();
        let repo = parts[1].trim();

        if owner.is_empty() || repo.is_empty() {
            return Err(SlackCoderError::Config(
                "Owner and repository name cannot be empty".to_string(),
            ));
        }

        Ok((owner.to_string(), repo.to_string()))
    }
}
