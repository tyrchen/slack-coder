use crate::agent::AgentManager;
use crate::error::{Result, SlackCoderError};
use crate::slack::{
    ChannelId, SlackClient, SlackCommandHandler, SlackMessage, ThreadTs, markdown_to_slack,
};
use claude_agent_sdk_rs::Message as ClaudeMessage;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

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
            "💬 Processing message {} user={}",
            message.channel.log_format(),
            message.user.as_str()
        );
        tracing::debug!("  Text: '{}'", message.text);

        // Check if message is a command
        if message.text.starts_with('/') {
            tracing::info!("🎯 Detected command: {}", message.text);
            let command_handler = SlackCommandHandler::new(self.slack_client.clone());
            return command_handler
                .handle_command(&message.text, &message.channel, &self.agent_manager)
                .await;
        }

        // Check if channel has configured agent
        let has_agent = self.agent_manager.has_agent(&message.channel);
        tracing::info!(
            "🔍 Agent check {} has_agent={}",
            message.channel.log_format(),
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
        tracing::debug!("Getting agent {}...", channel.log_format());
        // Get agent from manager (returns Arc<Mutex<RepoAgent>>)
        let agent_mutex = self.agent_manager.get_repo_agent(channel).await?;

        tracing::debug!("  Got agent, attempting to acquire lock with timeout...");

        // Try to acquire lock with timeout to avoid blocking forever
        let agent_lock = timeout(Duration::from_secs(3), agent_mutex.lock()).await;

        let mut agent = match agent_lock {
            Ok(agent) => {
                tracing::info!(
                    "🔒 Agent locked {}, sending query to Claude...",
                    channel.log_format()
                );
                agent
            }
            Err(_) => {
                tracing::warn!(
                    "⏳ Agent busy {}, lock acquisition timed out after 3s",
                    channel.log_format()
                );

                // Send user-friendly message
                self.slack_client
                    .send_message(
                        channel,
                        "⏳ *Agent is currently processing another request*\n\n\
                         Your message has been received, but the agent is busy with a previous task. \
                         Please wait for the current task to complete and try again in a moment.\n\n\
                         *Tip*: Long-running tasks (like comprehensive code analysis or documentation) \
                         can take several minutes. You can check the latest progress update above.",
                        thread_ts,
                    )
                    .await?;

                return Ok(());
            }
        };

        // Send query to agent
        agent.query(text).await?;
        tracing::info!(
            "✅ Query sent {}, streaming response...",
            channel.log_format()
        );

        // Stream response - lock is held during entire streaming
        let mut stream = agent.receive_response();
        let mut final_result = String::new();
        let mut message_count = 0;

        while let Some(message) = stream.next().await {
            message_count += 1;
            tracing::debug!("Received message #{} from Claude", message_count);

            let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

            if let ClaudeMessage::Result(res) = message {
                final_result = res.result.unwrap_or_default();
                tracing::info!(
                    "✅ Received final result {} ({} chars)",
                    channel.log_format(),
                    final_result.len()
                );
                break;
            }
        }

        // Send response to Slack
        if !final_result.is_empty() {
            tracing::info!(
                "📤 Sending response {} ({} chars)...",
                channel.log_format(),
                final_result.len()
            );

            // Convert markdown to Slack format
            let slack_formatted = markdown_to_slack(&final_result);
            tracing::debug!("Converted markdown to Slack format");

            // Split into chunks if response is too large (Slack has 40KB limit)
            const MAX_SLACK_MESSAGE_SIZE: usize = 39000; // Leave some margin

            if slack_formatted.len() > MAX_SLACK_MESSAGE_SIZE {
                tracing::warn!("Response is large, splitting into chunks");
                for (i, chunk) in slack_formatted
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
                    .send_message(channel, &slack_formatted, thread_ts)
                    .await?;
            }

            tracing::info!("✅ Response sent {}", channel.log_format());
        } else {
            tracing::warn!("⚠️  No response from agent {}", channel.log_format());
        }

        Ok(())
    }
}
