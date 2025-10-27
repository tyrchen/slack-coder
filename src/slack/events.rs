use crate::agent::AgentManager;
use crate::error::Result;
use crate::metadata::MetadataCache;
use crate::slack::{
    ChannelId, FormHandler, MessageProcessor, MessageTs, SlackClient, SlackMessage, ThreadTs,
    UserId,
};
use dashmap::DashMap;
use slack_morphism::prelude::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

#[derive(Clone)]
struct BotState {
    message_processor: Arc<MessageProcessor>,
    form_handler: Arc<FormHandler>,
    slack_client: Arc<SlackClient>,
    metadata_cache: Arc<MetadataCache>,
    processed_events: Arc<DashMap<String, Instant>>,
}

pub struct EventHandler {
    slack_client: Arc<SlackClient>,
    agent_manager: Arc<AgentManager>,
    metadata_cache: Arc<MetadataCache>,
}

impl EventHandler {
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

    /// Start listening for Slack events using Socket Mode
    pub async fn start(self) -> Result<()> {
        tracing::info!("Initializing event handler components");

        // Create SHARED processed_events cache (same instance across all event callbacks)
        let processed_events = Arc::new(DashMap::new());
        tracing::debug!("Created event deduplication cache");

        // Create state with our components
        let message_processor = Arc::new(MessageProcessor::new(
            self.slack_client.clone(),
            self.agent_manager.clone(),
            self.metadata_cache.clone(),
        ));
        let form_handler = Arc::new(FormHandler::new(
            self.slack_client.clone(),
            self.agent_manager.clone(),
        ));

        let bot_state = BotState {
            message_processor,
            form_handler,
            slack_client: self.slack_client.clone(),
            metadata_cache: self.metadata_cache.clone(),
            processed_events,
        };

        tracing::debug!("Creating listener environment");
        let listener_environment = Arc::new(
            SlackClientEventsListenerEnvironment::new(self.slack_client.get_client())
                .with_error_handler(Self::error_handler)
                .with_user_state(bot_state),
        );

        tracing::debug!("Configuring Socket Mode callbacks");
        let callbacks =
            SlackSocketModeListenerCallbacks::new().with_push_events(Self::handle_push_event);

        tracing::debug!("Creating Socket Mode listener");
        let socket_mode_listener = SlackClientSocketModeListener::new(
            &SlackClientSocketModeConfig::new(),
            listener_environment,
            callbacks,
        );

        // Get app token from client
        let app_token = self.slack_client.get_app_token();
        tracing::info!("Connecting to Slack via Socket Mode");

        socket_mode_listener
            .listen_for(&app_token)
            .await
            .map_err(|e| crate::error::SlackCoderError::SlackApi(e.to_string()))?;

        tracing::info!("Connected to Slack Socket Mode");
        tracing::info!("Bot is ready to receive messages");

        socket_mode_listener.serve().await;

        Ok(())
    }

    async fn handle_push_event(
        event: SlackPushEventCallback,
        _client: Arc<SlackHyperClient>,
        user_state: SlackClientEventsUserState,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Log event type without verbose debug dump
        let event_type = match &event.event {
            SlackEventCallbackBody::AppMention(_) => "app_mention",
            SlackEventCallbackBody::Message(_) => "message",
            _ => "other",
        };
        tracing::debug!(event_type = event_type, "Received push event");

        // Extract state
        let state: BotState = {
            let storage = user_state.read().await;
            storage
                .get_user_state::<BotState>()
                .expect("BotState should be set")
                .clone()
        };

        // Cleanup old events (older than 1 hour) to prevent memory growth
        Self::cleanup_old_events(&state.processed_events);

        // Spawn processing as background task and return immediately
        // This ensures we acknowledge within 3 seconds (Slack's timeout)
        tokio::spawn(async move {
            if let Err(e) = Self::process_event(event, state).await {
                tracing::error!(error = %e, "Event processing failed");
            }
        });

        // Return immediately so Slack gets acknowledgment quickly
        Ok(())
    }

    async fn process_event(
        event: SlackPushEventCallback,
        state: BotState,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match event.event {
            SlackEventCallbackBody::AppMention(mention) => {
                // Deduplicate events using timestamp
                // Use message ts as unique key - same message should never be processed twice
                let event_key = format!("mention:{}:{}", mention.channel, mention.origin.ts);
                if let Some(last_seen) = state.processed_events.get(&event_key) {
                    // Event was already processed - skip regardless of how long ago
                    tracing::debug!(
                        event_key = %event_key,
                        last_seen_ago = format_duration(last_seen.elapsed()),
                        "Duplicate event detected, skipping"
                    );
                    return Ok(());
                }
                state
                    .processed_events
                    .insert(event_key.clone(), Instant::now());
                tracing::debug!(event_key = %event_key, "Processing new event");

                let channel_id = ChannelId::new(mention.channel.to_string());

                let text = mention.content.text.clone().unwrap_or_default();
                let user_id = UserId::new(mention.user.to_string());

                // Get enriched context with channel and user names
                let ctx = state
                    .metadata_cache
                    .log_context(channel_id.as_str(), mention.user.as_ref())
                    .await;

                let span = tracing::info_span!(
                    "app_mention",
                    channel_id = %ctx.channel_id,
                    channel = %ctx.channel_name,
                    user_id = %ctx.user_id,
                    user = %ctx.user_name,
                    ts = %mention.origin.ts
                );
                let _guard = span.enter();

                // Show first 150 chars of message for context
                let message_preview = if text.len() > 150 {
                    format!("{}...", text.chars().take(150).collect::<String>())
                } else {
                    text.clone()
                };

                tracing::info!(
                    channel_id = %ctx.channel_id,
                    channel = %ctx.channel_display,
                    user_id = %ctx.user_id,
                    user = %ctx.user_display,
                    message = %message_preview,
                    "App mentioned in {} by {}: \"{}\"",
                    ctx.channel_display,
                    ctx.user_display,
                    message_preview
                );

                // Log selective fields instead of full debug dump
                tracing::debug!(
                    text_len = text.len(),
                    has_blocks = mention.content.blocks.is_some(),
                    thread_ts = ?mention.origin.thread_ts,
                    "App mention details"
                );
                let ts = MessageTs::new(mention.origin.ts.to_string());
                let thread_ts = mention
                    .origin
                    .thread_ts
                    .map(|t| ThreadTs::new(t.to_string()));

                // Strip bot mention from text
                let clean_text = text
                    .split_whitespace()
                    .filter(|w| !w.starts_with("<@"))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string();

                tracing::debug!(
                    original_len = text.len(),
                    cleaned_len = clean_text.len(),
                    "Cleaned mention text"
                );

                // Check if this is a command (starts with /)
                if clean_text.starts_with('/') {
                    tracing::info!(command = %clean_text, "Processing command");
                    // Forward to message processor for command handling
                    let slack_message = SlackMessage {
                        channel: channel_id.clone(),
                        user: user_id.clone(),
                        text: clean_text.clone(),
                        thread_ts: thread_ts.clone(),
                        ts: ts.clone(),
                    };

                    if let Err(e) = state.message_processor.process_message(slack_message).await {
                        tracing::error!(error = %e, "Command processing failed");
                    }
                }
                // Check if this looks like a repository name (owner/repo pattern)
                else if clean_text.contains('/') && clean_text.split_whitespace().count() == 1 {
                    tracing::info!(repo = %clean_text, "Processing setup request");
                    if let Err(e) = state
                        .form_handler
                        .handle_repo_setup(channel_id.clone(), clean_text.clone())
                        .await
                    {
                        tracing::error!(error = %e, repo = %clean_text, "Setup failed");
                        let _ = state
                            .slack_client
                            .send_message(
                                &channel_id,
                                &format!("Setup failed: {}", e),
                                thread_ts.as_ref(),
                            )
                            .await;
                    }
                } else {
                    tracing::info!("Processing regular message");
                    // Regular message - process it
                    let slack_message = SlackMessage {
                        channel: channel_id,
                        user: user_id,
                        text: clean_text,
                        thread_ts,
                        ts,
                    };

                    if let Err(e) = state.message_processor.process_message(slack_message).await {
                        tracing::error!(error = %e, "Message processing failed");
                    }
                }
            }
            SlackEventCallbackBody::Message(message) => {
                let channel = message.origin.channel.as_ref().map(|c| c.to_string());
                let user = message.sender.user.as_ref().map(|u| u.to_string());

                // Log selective fields instead of full debug dump
                tracing::debug!(
                    channel = ?channel,
                    user = ?user,
                    subtype = ?message.subtype,
                    has_bot_id = message.sender.bot_id.is_some(),
                    "Message event received"
                );

                // Ignore bot's own messages to prevent loops
                if message.sender.bot_id.is_some() {
                    tracing::debug!("Ignoring bot message");
                    return Ok(());
                }

                // Ignore message updates/edits
                if message.subtype == Some(SlackMessageEventType::MessageChanged) {
                    tracing::debug!("Ignoring message edit");
                    return Ok(());
                }

                // Check if this is a channel_join event (bot was invited)
                if message.subtype == Some(SlackMessageEventType::ChannelJoin) {
                    if let Some(channel_id) = message.origin.channel {
                        let channel = ChannelId::new(channel_id.to_string());
                        tracing::info!(channel = %channel.as_str(), "Bot joined channel");

                        // Check if already setup
                        if state.form_handler.agent_manager.has_agent(&channel) {
                            tracing::info!("Channel already configured");
                        } else {
                            tracing::info!("Showing setup instructions");
                            if let Err(e) = state.form_handler.show_repo_setup_form(&channel).await
                            {
                                tracing::error!(error = %e, "Failed to show setup form");
                            }
                        }
                    }
                } else {
                    // Handle regular messages in threads where bot participated
                    tracing::debug!(
                        subtype = ?message.subtype,
                        "Skipping regular message"
                    );
                }
            }
            _ => {
                tracing::debug!("Unhandled event type");
            }
        }

        Ok(())
    }

    fn error_handler(
        err: Box<dyn std::error::Error + Send + Sync>,
        _client: Arc<SlackHyperClient>,
        _states: SlackClientEventsUserState,
    ) -> HttpStatusCode {
        tracing::error!(
            error = %err,
            error_kind = std::any::type_name_of_val(&*err),
            "Slack event error"
        );
        HttpStatusCode::OK
    }

    /// Cleanup events older than 1 hour to prevent memory growth
    fn cleanup_old_events(events: &Arc<DashMap<String, Instant>>) {
        let cutoff = Duration::from_secs(3600); // 1 hour
        let mut removed = 0;

        events.retain(|_key, instant| {
            let keep = instant.elapsed() < cutoff;
            if !keep {
                removed += 1;
            }
            keep
        });

        if removed > 0 {
            tracing::debug!(removed_count = removed, "Cleaned up old events from cache");
        }
    }
}
