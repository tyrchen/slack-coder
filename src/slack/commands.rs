use crate::agent::AgentManager;
use crate::error::Result;
use crate::slack::{ChannelId, SlackClient};
use std::sync::Arc;

pub struct SlackCommandHandler {
    slack_client: Arc<SlackClient>,
}

impl SlackCommandHandler {
    pub fn new(slack_client: Arc<SlackClient>) -> Self {
        Self { slack_client }
    }

    /// Handle a slash command
    pub async fn handle_command(
        &self,
        command: &str,
        channel: &ChannelId,
        agent_manager: &AgentManager,
    ) -> Result<()> {
        tracing::info!("Handling command: {} in {}", command, channel.log_format());

        match command.trim() {
            "/help" => self.handle_help(channel).await,
            "/new-session" => self.handle_new_session(channel, agent_manager).await,
            _ => {
                self.slack_client
                    .send_message(
                        channel,
                        &format!(
                            "❓ Unknown command: `{}`\n\nType `/help` for available commands.",
                            command
                        ),
                        None,
                    )
                    .await?;
                Ok(())
            }
        }
    }

    /// Handle /help command
    async fn handle_help(&self, channel: &ChannelId) -> Result<()> {
        let help_text = r#"📚 *Available Commands*

`/help` - Show this help message
`/new-session` - Start a fresh conversation (clears context)

*Examples:*
• Type `/new-session` to start over with a clean slate
• Type `/help` anytime to see available commands

*Note:* Commands must be sent as a message to the bot (mention me or DM), not as Slack's built-in slash commands."#;

        tracing::info!("Sending help message to {}", channel.log_format());
        self.slack_client
            .send_message(channel, help_text, None)
            .await?;
        Ok(())
    }

    /// Handle /new-session command
    async fn handle_new_session(
        &self,
        channel: &ChannelId,
        agent_manager: &AgentManager,
    ) -> Result<()> {
        // Check if agent exists for this channel
        if !agent_manager.has_agent(channel) {
            self.slack_client
                .send_message(
                    channel,
                    "⚠️  *No agent configured for this channel.*\n\nPlease mention me with a repository name to set up first.",
                    None,
                )
                .await?;
            return Ok(());
        }

        // Get agent and start new session
        let agent_mutex = agent_manager.get_repo_agent(channel).await?;
        let mut agent = agent_mutex.lock().await;

        let new_session_id = agent.start_new_session().await?;

        // Notify user
        let message = format!(
            r#"🔄 *New Session Started*

Session ID: `{}`

Your conversation context has been cleared. You can now start fresh!

*What does this mean?*
• Previous conversation history is no longer accessible
• The bot won't remember earlier discussions in this channel
• Great for switching to a completely different task

Type `/help` for more commands."#,
            new_session_id
        );

        tracing::info!(
            "New session created: {} for {}",
            new_session_id,
            channel.log_format()
        );

        self.slack_client
            .send_message(channel, &message, None)
            .await?;

        Ok(())
    }
}
