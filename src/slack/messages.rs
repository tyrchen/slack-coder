use crate::agent::AgentManager;
use crate::error::{Result, SlackCoderError};
use crate::slack::{
    ChannelId, SlackClient, SlackCommandHandler, SlackMessage, ThreadTs, markdown_to_slack,
};
use claude_agent_sdk_rs::Message as ClaudeMessage;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Instant;

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
        let start = Instant::now();
        tracing::info!(
            "💬 Processing message {} user={} text_length={} in_thread={}",
            message.channel.log_format(),
            message.user.as_str(),
            message.text.len(),
            message.thread_ts.is_some()
        );
        tracing::debug!("  Text: '{}'", message.text);

        // Check if message is a command
        if message.text.starts_with('/') {
            tracing::info!("🎯 Detected command: '{}'", message.text);
            let command_handler = SlackCommandHandler::new(self.slack_client.clone());
            let result = command_handler
                .handle_command(&message.text, &message.channel, &self.agent_manager)
                .await;
            tracing::info!(
                "✅ Command processed {} duration={:?}",
                message.channel.log_format(),
                start.elapsed()
            );
            return result;
        }

        // Check if channel has configured agent
        let has_agent = self.agent_manager.has_agent(&message.channel);
        tracing::debug!(
            "  🔍 Agent check {} has_agent={}",
            message.channel.log_format(),
            has_agent
        );

        if !has_agent {
            tracing::info!(
                "⚠️  No agent configured {}, prompting for setup",
                message.channel.log_format()
            );
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
        tracing::info!(
            "✅ Agent found {}, forwarding to repository agent...",
            message.channel.log_format()
        );
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
        let total_start = Instant::now();

        tracing::debug!("  Getting agent {}...", channel.log_format());
        // Get agent from manager (returns Arc<Mutex<RepoAgent>>)
        let agent_mutex = self.agent_manager.get_repo_agent(channel).await?;
        tracing::debug!("  ✅ Got agent, acquiring lock...");

        // Lock agent for this request
        let lock_start = Instant::now();
        let mut agent = agent_mutex.lock().await;
        let lock_duration = lock_start.elapsed();

        if lock_duration.as_millis() > 100 {
            tracing::warn!(
                "⚠️  Agent lock acquired {} (wait_time={:?} - possible contention)",
                channel.log_format(),
                lock_duration
            );
        } else {
            tracing::debug!(
                "  🔒 Agent lock acquired {} (wait_time={:?})",
                channel.log_format(),
                lock_duration
            );
        }

        // Send query to agent
        let query_start = Instant::now();
        let text_preview = text.chars().take(60).collect::<String>();
        tracing::info!(
            "📤 Sending query to Claude {} text_preview='{}'...",
            channel.log_format(),
            text_preview
        );

        agent.query(text).await?;
        tracing::debug!(
            "  ✅ Query sent {} (send_duration={:?})",
            channel.log_format(),
            query_start.elapsed()
        );

        // Stream response
        tracing::info!(
            "📡 Streaming response from Claude {}...",
            channel.log_format()
        );
        let stream_start = Instant::now();
        let mut stream = agent.receive_response();
        let mut final_result = String::new();
        let mut chunk_count = 0;

        while let Some(message) = stream.next().await {
            chunk_count += 1;

            let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

            if let ClaudeMessage::Result(res) = message {
                final_result = res.result.unwrap_or_default();
                let total_bytes = final_result.len();
                tracing::info!(
                    "✅ Received final result {} (chunks={}, bytes={}, stream_duration={:?})",
                    channel.log_format(),
                    chunk_count,
                    total_bytes,
                    stream_start.elapsed()
                );
                break;
            } else {
                // Log progress every 5 chunks
                if chunk_count % 5 == 0 {
                    tracing::debug!(
                        "  📦 Received chunk #{} from Claude {}",
                        chunk_count,
                        channel.log_format()
                    );
                }
            }
        }

        // Send response to Slack
        if !final_result.is_empty() {
            let format_start = Instant::now();

            // Convert markdown to Slack format
            let slack_formatted = markdown_to_slack(&final_result);
            let format_duration = format_start.elapsed();
            tracing::debug!(
                "  🔄 Converted markdown to Slack format {} (before={} chars, after={} chars, format_duration={:?})",
                channel.log_format(),
                final_result.len(),
                slack_formatted.len(),
                format_duration
            );

            // Split into chunks if response is too large (Slack has 40KB limit)
            const MAX_SLACK_MESSAGE_SIZE: usize = 39000; // Leave some margin

            let send_start = Instant::now();
            if slack_formatted.len() > MAX_SLACK_MESSAGE_SIZE {
                let chunk_count = slack_formatted.len().div_ceil(MAX_SLACK_MESSAGE_SIZE);
                tracing::warn!(
                    "⚠️  Response is large {} ({} chars), splitting into {} chunks",
                    channel.log_format(),
                    slack_formatted.len(),
                    chunk_count
                );

                for (i, chunk) in slack_formatted
                    .as_bytes()
                    .chunks(MAX_SLACK_MESSAGE_SIZE)
                    .enumerate()
                {
                    let chunk_text = String::from_utf8_lossy(chunk).to_string();
                    let prefix = if i == 0 {
                        String::new()
                    } else {
                        format!("*(continued {}/{})*\n\n", i + 1, chunk_count)
                    };

                    self.slack_client
                        .send_message(channel, &format!("{}{}", prefix, chunk_text), thread_ts)
                        .await?;

                    tracing::debug!(
                        "  ✅ Sent chunk {}/{} {}",
                        i + 1,
                        chunk_count,
                        channel.log_format()
                    );
                }
            } else {
                tracing::info!(
                    "📤 Sending response {} ({} chars)...",
                    channel.log_format(),
                    slack_formatted.len()
                );
                self.slack_client
                    .send_message(channel, &slack_formatted, thread_ts)
                    .await?;
            }

            let send_duration = send_start.elapsed();
            let total_duration = total_start.elapsed();

            tracing::info!(
                "✅ Response delivered {} (send_duration={:?}, total_duration={:?})",
                channel.log_format(),
                send_duration,
                total_duration
            );
        } else {
            tracing::warn!(
                "⚠️  No response from agent {} (total_duration={:?})",
                channel.log_format(),
                total_start.elapsed()
            );
        }

        Ok(())
    }
}
