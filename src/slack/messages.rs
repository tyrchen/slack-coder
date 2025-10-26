use crate::agent::AgentManager;
use crate::error::{Result, SlackCoderError};
use crate::slack::{ChannelId, SlackClient, SlackMessage, ThreadTs};
use claude_agent_sdk_rs::Message as ClaudeMessage;
use futures::StreamExt;
use std::sync::Arc;

pub struct MessageProcessor {
    slack_client: Arc<SlackClient>,
    agent_manager: Arc<AgentManager>,
}

impl MessageProcessor {
    pub fn new(slack_client: Arc<SlackClient>, agent_manager: Arc<AgentManager>) -> Self {
        Self {
            slack_client,
            agent_manager,
        }
    }

    /// Process user message - forward to appropriate agent
    pub async fn process_message(&self, message: SlackMessage) -> Result<()> {
        tracing::info!(
            "Processing message from {} in channel {}",
            message.user.as_str(),
            message.channel.as_str()
        );

        // Check if channel has configured agent
        if !self.agent_manager.has_agent(&message.channel) {
            self.slack_client
                .send_message(
                    &message.channel,
                    "This channel is not configured yet. Please provide a repository name in the format `owner/repo-name` to get started.",
                    message.thread_ts.as_ref(),
                )
                .await?;
            return Ok(());
        }

        // Forward to agent
        self.forward_to_agent(&message.text, &message.channel, message.thread_ts.as_ref())
            .await
    }

    /// Forward message to repository agent and stream response
    async fn forward_to_agent(
        &self,
        text: &str,
        channel: &ChannelId,
        thread_ts: Option<&ThreadTs>,
    ) -> Result<()> {
        // Get agent from manager (returns Arc<Mutex<RepoAgent>>)
        let agent_mutex = self.agent_manager.get_repo_agent(channel).await?;

        // Lock agent for this request
        let mut agent = agent_mutex.lock().await;

        // Send query to agent
        agent.query(text).await?;

        // Stream response
        let mut stream = agent.receive_response();
        let mut final_result = String::new();

        while let Some(message) = stream.next().await {
            let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

            if let ClaudeMessage::Result(res) = message {
                final_result = res.result.unwrap_or_default();
                break;
            }
        }

        // Send response to Slack
        if !final_result.is_empty() {
            self.slack_client
                .send_message(channel, &final_result, thread_ts)
                .await?;
        }

        Ok(())
    }
}
