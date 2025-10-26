use crate::agent::AgentManager;
use crate::error::{Result, SlackCoderError};
use crate::slack::{ChannelId, SlackClient};
use std::sync::Arc;

pub struct FormHandler {
    slack_client: Arc<SlackClient>,
    agent_manager: Arc<AgentManager>,
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
        // Validate repo name format
        let (owner, repo) = Self::validate_repo_name_format(&repo_name)?;

        tracing::info!(
            "Starting setup for channel {} with repo {}/{}",
            channel.as_str(),
            owner,
            repo
        );

        // Send acknowledgment
        self.slack_client
            .send_message(
                &channel,
                &format!("Setting up repository `{}`...\nThis may take a minute.", repo_name),
                None,
            )
            .await?;

        // Trigger setup via agent manager
        self.agent_manager
            .setup_channel(channel.clone(), repo_name.clone())
            .await?;

        // Send completion message
        self.slack_client
            .send_message(
                &channel,
                &format!(
                    "✅ Repository `{}` is now ready!\n\nYou can now ask me to generate code, write documentation, or use commands like `/help`.",
                    repo_name
                ),
                None,
            )
            .await?;

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
