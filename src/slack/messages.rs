use crate::error::Result;
use crate::slack::{ChannelId, SlackClient, SlackMessage};
use std::sync::Arc;

pub struct MessageProcessor {
    slack_client: Arc<SlackClient>,
}

impl MessageProcessor {
    pub fn new(slack_client: Arc<SlackClient>) -> Self {
        Self { slack_client }
    }

    /// Process user message - forward to appropriate agent
    pub async fn process_message(&self, message: SlackMessage) -> Result<()> {
        tracing::info!(
            "Processing message from {} in channel {}",
            message.user.as_str(),
            message.channel.as_str()
        );

        // TODO: Check if channel has configured agent
        // TODO: If no agent, show setup form
        // TODO: If agent exists, forward message to repo agent

        self.slack_client
            .send_message(
                &message.channel,
                "Message received (processing not yet implemented)",
                None,
            )
            .await?;

        Ok(())
    }

    /// Forward message to repository agent and stream response
    async fn forward_to_agent(&self, _text: &str, _channel: &ChannelId) -> Result<()> {
        // TODO: Get agent from manager
        // TODO: Call agent.query()
        // TODO: Stream response
        Ok(())
    }
}
