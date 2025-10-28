use crate::agent::AgentManager;
use crate::error::{Result, SlackCoderError};
use crate::logging::Timer;
use crate::metadata::MetadataCache;
use crate::slack::{
    ChannelId, MessageTs, SlackClient, SlackCommandHandler, SlackMessage, ThreadTs, UsageMetrics,
    markdown_to_slack,
};
use claude_agent_sdk_rs::Message as ClaudeMessage;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

pub struct MessageProcessor {
    slack_client: Arc<SlackClient>,
    agent_manager: Arc<AgentManager>,
    metadata_cache: Arc<MetadataCache>,
}

impl MessageProcessor {
    pub fn new(
        slack_client: Arc<SlackClient>,
        agent_manager: Arc<AgentManager>,
        metadata_cache: Arc<MetadataCache>,
    ) -> Self {
        Self {
            slack_client,
            agent_manager,
            metadata_cache,
        }
    }

    /// Process user message - forward to appropriate agent
    pub async fn process_message(&self, message: SlackMessage) -> Result<()> {
        let _timer = Timer::new("process_message");

        // Get enriched context
        let ctx = self
            .metadata_cache
            .log_context(message.channel.as_str(), message.user.as_str())
            .await;

        let span = tracing::info_span!(
            "process_message",
            channel_id = %ctx.channel_id,
            channel = %ctx.channel_name,
            user_id = %ctx.user_id,
            user = %ctx.user_name,
            has_thread = message.thread_ts.is_some(),
        );
        let _guard = span.enter();

        // Show message preview for context
        let message_preview = if message.text.len() > 150 {
            format!("{}...", message.text.chars().take(150).collect::<String>())
        } else {
            message.text.clone()
        };

        tracing::info!(
            channel_id = %ctx.channel_id,
            channel = %ctx.channel_display,
            user_id = %ctx.user_id,
            user = %ctx.user_display,
            message = %message_preview,
            "Message from {} in {}: \"{}\"",
            ctx.user_display,
            ctx.channel_display,
            message_preview
        );

        // Check if message is a command
        if message.text.starts_with('/') {
            tracing::info!(command = %message.text, "Processing command");
            let command_handler = SlackCommandHandler::new(self.slack_client.clone());
            return command_handler
                .handle_command(&message.text, &message.channel, &self.agent_manager)
                .await;
        }

        // Check if channel has configured agent
        let has_agent = self.agent_manager.has_agent(&message.channel);
        tracing::debug!(has_agent = has_agent, "Agent availability check");

        if !has_agent {
            tracing::info!("No agent configured, prompting for setup");
            self.slack_client
                .send_message(
                    &message.channel,
                    "*This channel is not configured yet.*\n\nPlease mention me with a repository name in the format `owner/repo-name` to get started.\n\n*Example:*\n```\n@slack-coder tyrchen/rust-lib-template\n```",
                    message.thread_ts.as_ref(),
                )
                .await?;
            return Ok(());
        }

        // Forward to agent
        tracing::debug!("Forwarding to repository agent");
        // Use existing thread_ts if in thread, otherwise use message ts to create thread
        let reply_thread_ts = message
            .thread_ts
            .as_ref()
            .map(|t| t.clone())
            .unwrap_or_else(|| ThreadTs::new(message.ts.as_str()));

        self.forward_to_agent(
            &message.text,
            &message.channel,
            &reply_thread_ts,
            &message.ts,
        )
        .await
    }

    /// Forward message to repository agent and stream response
    async fn forward_to_agent(
        &self,
        text: &str,
        channel: &ChannelId,
        thread_ts: &ThreadTs,
        _message_ts: &MessageTs,
    ) -> Result<()> {
        tracing::debug!("Acquiring agent lock");
        // Get agent from manager (returns Arc<Mutex<RepoAgent>>)
        let agent_mutex = self.agent_manager.get_repo_agent(channel).await?;

        // Try to acquire lock with timeout to avoid blocking forever
        let agent_lock = timeout(Duration::from_secs(3), agent_mutex.lock()).await;

        let mut agent = match agent_lock {
            Ok(agent) => {
                tracing::info!("Agent lock acquired, sending query to Claude");
                agent
            }
            Err(_) => {
                tracing::warn!(timeout_secs = 3, "Agent lock timeout - agent busy");

                // Send user-friendly message as reply in the same thread
                self.slack_client
                    .send_message(
                        channel,
                        "⏳ *Agent is currently processing another request*\n\n\
                         Your message has been received, but the agent is busy with a previous task. \
                         Please wait for the current task to complete and try again in a moment.\n\n\
                         *Tip*: Long-running tasks (like comprehensive code analysis or documentation) \
                         can take several minutes. You can check the latest progress update above.",
                        Some(thread_ts), // This ensures it's a reply in the thread
                    )
                    .await?;

                return Ok(());
            }
        };

        // Send query to agent
        agent.query(text).await?;
        tracing::debug!("Query sent, streaming response");

        // Stream response - lock is held during entire streaming
        let mut stream = agent.receive_response();
        let mut final_result = String::new();
        let mut result_message = None;
        let mut message_count = 0;

        while let Some(message) = stream.next().await {
            message_count += 1;
            tracing::debug!(message_num = message_count, "Received message from Claude");

            let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

            if let ClaudeMessage::Result(res) = message {
                final_result = res.result.clone().unwrap_or_default();
                result_message = Some(res);
                tracing::info!(result_len = final_result.len(), "Received final result");
                break;
            }
        }

        // Send response to Slack
        if !final_result.is_empty() {
            // Convert markdown to Slack format
            let slack_formatted = markdown_to_slack(&final_result);

            // Append detailed metrics footer if available (consolidated - single message!)
            let final_message = if let Some(result_msg) = &result_message {
                let metrics = UsageMetrics::from_result_message(result_msg);

                let cost_str = if let Some(cost) = metrics.cost_usd {
                    format!("${:.4} USD", cost)
                } else {
                    "N/A".to_string()
                };

                let duration_sec = metrics.duration_ms as f64 / 1000.0;
                let api_duration_sec = metrics.duration_api_ms as f64 / 1000.0;

                // Build detailed metrics section
                let mut metrics_footer = format!(
                    "\n\n---\n📊 *Query Metrics*\n\
                     • Tokens: {} input + {} output = *{} total*\n\
                     • Cost: {}\n\
                     • Duration: {:.2}s (API: {:.2}s)\n\
                     • Turns: {}\n\
                     • Session: `{}`",
                    metrics.input_tokens,
                    metrics.output_tokens,
                    metrics.total_tokens,
                    cost_str,
                    duration_sec,
                    api_duration_sec,
                    metrics.num_turns,
                    metrics.session_id
                );

                // Add cache info if present
                if metrics.cache_creation_input_tokens > 0 || metrics.cache_read_input_tokens > 0 {
                    metrics_footer.push_str(&format!(
                        "\n• Cache: {} created, {} read",
                        metrics.cache_creation_input_tokens, metrics.cache_read_input_tokens
                    ));
                }

                // Add task complete indicator
                metrics_footer.push_str("\n\n✅ *Task Complete* - All operations finished!");

                tracing::debug!(
                    tokens = metrics.total_tokens,
                    cost_usd = metrics.cost_usd.unwrap_or(0.0),
                    duration_ms = metrics.duration_ms,
                    "Appending detailed metrics to result"
                );

                format!("{}{}", slack_formatted, metrics_footer)
            } else {
                slack_formatted
            };

            tracing::debug!(
                original_len = final_result.len(),
                final_len = final_message.len(),
                "Prepared message with metrics"
            );

            // Split into chunks if response is too large (Slack has 40KB limit)
            const MAX_SLACK_MESSAGE_SIZE: usize = 39000; // Leave some margin

            if final_message.len() > MAX_SLACK_MESSAGE_SIZE {
                let chunk_count = final_message.len().div_ceil(MAX_SLACK_MESSAGE_SIZE);
                tracing::warn!(
                    message_len = final_message.len(),
                    chunk_count = chunk_count,
                    "Message exceeds size limit, splitting into chunks"
                );

                for (i, chunk) in final_message
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
                        .send_message(
                            channel,
                            &format!("{}{}", prefix, chunk_text),
                            Some(thread_ts),
                        )
                        .await?;
                }
            } else {
                self.slack_client
                    .send_message(channel, &final_message, Some(thread_ts))
                    .await?;
            }

            tracing::info!(
                message_len = final_message.len(),
                has_metrics = result_message.is_some(),
                "Response sent with metrics"
            );
        } else {
            tracing::warn!("No response received from agent");
        }

        Ok(())
    }
}
