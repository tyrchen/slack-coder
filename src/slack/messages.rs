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
            "💬 Processing message from user {} in channel {}",
            message.user.as_str(),
            message.channel.as_str()
        );
        tracing::debug!("Message text: '{}'", message.text);

        // Check if channel has configured agent
        let has_agent = self.agent_manager.has_agent(&message.channel);
        tracing::info!(
            "🔍 Channel {} has agent: {}",
            message.channel.as_str(),
            has_agent
        );

        if !has_agent {
            tracing::info!("⚠️  No agent configured for this channel, prompting for setup");
            self.slack_client
                .send_message(
                    &message.channel,
                    "👋 *This channel is not configured yet.*\n\nPlease mention me with a repository name in the format `owner/repo-name` to get started.\n\n*Example:*\n```\n@slack-coder tyrchen/rust-lib-template\n```",
                    message.thread_ts.as_ref(),
                )
                .await?;
            return Ok(());
        }

        // Forward to agent
        tracing::info!("✅ Agent found! Forwarding message to repository agent...");
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
        tracing::debug!("Getting agent for channel {}...", channel.as_str());
        // Get agent from manager (returns Arc<Mutex<RepoAgent>>)
        let agent_mutex = self.agent_manager.get_repo_agent(channel).await?;
        tracing::debug!("✅ Got agent, acquiring lock...");

        // Lock agent for this request
        let mut agent = agent_mutex.lock().await;
        tracing::info!("🔒 Agent locked, sending query to Claude...");

        // Send query to agent
        agent.query(text).await?;
        tracing::info!("✅ Query sent, streaming response...");

        // Stream response
        let mut stream = agent.receive_response();
        let mut final_result = String::new();
        let mut message_count = 0;

        while let Some(message) = stream.next().await {
            message_count += 1;
            tracing::debug!("Received message #{} from Claude", message_count);

            let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

            if let ClaudeMessage::Result(res) = message {
                final_result = res.result.unwrap_or_default();
                tracing::info!("✅ Received final result ({} chars)", final_result.len());
                break;
            }
        }

        // Send response to Slack
        if !final_result.is_empty() {
            tracing::info!(
                "📤 Sending response to Slack ({} chars)...",
                final_result.len()
            );

            // Split into chunks if response is too large (Slack has 40KB limit)
            const MAX_SLACK_MESSAGE_SIZE: usize = 39000; // Leave some margin

            if final_result.len() > MAX_SLACK_MESSAGE_SIZE {
                tracing::warn!("Response is large, splitting into chunks");
                for (i, chunk) in final_result
                    .as_bytes()
                    .chunks(MAX_SLACK_MESSAGE_SIZE)
                    .enumerate()
                {
                    let chunk_text = String::from_utf8_lossy(chunk).to_string();
                    let prefix = if i == 0 {
                        String::new()
                    } else {
                        format!("*(continued {}/...)*\n\n", i + 1)
                    };

                    self.slack_client
                        .send_message(channel, &format!("{}{}", prefix, chunk_text), thread_ts)
                        .await?;
                }
            } else {
                self.slack_client
                    .send_message(channel, &final_result, thread_ts)
                    .await?;
            }

            tracing::info!("✅ Response sent to Slack");
        } else {
            tracing::warn!("⚠️  No response from agent");
        }

        Ok(())
    }
}
