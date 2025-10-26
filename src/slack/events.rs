use crate::agent::AgentManager;
use crate::error::Result;
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
    processed_events: Arc<DashMap<String, Instant>>,
}

pub struct EventHandler {
    slack_client: Arc<SlackClient>,
    agent_manager: Arc<AgentManager>,
}

impl EventHandler {
    pub fn new(slack_client: Arc<SlackClient>, agent_manager: Arc<AgentManager>) -> Self {
        Self {
            slack_client,
            agent_manager,
        }
    }

    /// Start listening for Slack events using Socket Mode
    pub async fn start(self) -> Result<()> {
        tracing::info!("🔧 Initializing event handler components...");

        // Create SHARED processed_events cache (same instance across all event callbacks)
        let processed_events = Arc::new(DashMap::new());
        tracing::debug!("Created shared event deduplication cache");

        // Create state with our components
        let message_processor = Arc::new(MessageProcessor::new(
            self.slack_client.clone(),
            self.agent_manager.clone(),
        ));
        let form_handler = Arc::new(FormHandler::new(
            self.slack_client.clone(),
            self.agent_manager.clone(),
        ));

        let bot_state = BotState {
            message_processor,
            form_handler,
            slack_client: self.slack_client.clone(),
            processed_events,
        };

        tracing::debug!("Creating listener environment with bot state");
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
        tracing::info!("🔌 Connecting to Slack via Socket Mode...");

        socket_mode_listener
            .listen_for(&app_token)
            .await
            .map_err(|e| crate::error::SlackCoderError::SlackApi(e.to_string()))?;

        tracing::info!("✅ Connected! Listening for Slack events...");
        tracing::info!(
            "📱 Bot is ready to receive messages. Invite it to a channel and @mention it!"
        );

        socket_mode_listener.serve().await;

        Ok(())
    }

    async fn handle_push_event(
        event: SlackPushEventCallback,
        _client: Arc<SlackHyperClient>,
        user_state: SlackClientEventsUserState,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("📨 Received push event: {:?}", event.event);

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
                tracing::error!("Error processing event: {}", e);
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
                        "🔄 Duplicate event detected (processed {} ago), skipping: {}",
                        format_duration(last_seen.elapsed()),
                        event_key
                    );
                    return Ok(());
                }
                state
                    .processed_events
                    .insert(event_key.clone(), Instant::now());
                tracing::debug!("✅ New event, processing: {}", event_key);

                let channel_id = ChannelId::new(mention.channel.to_string());

                tracing::info!(
                    "🔔 App mentioned {} by user: {}",
                    channel_id.log_format(),
                    mention.user
                );
                tracing::debug!("Full mention event: {:?}", mention);
                let user_id = UserId::new(mention.user.to_string());
                let text = mention.content.text.clone().unwrap_or_default();
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

                tracing::info!("📝 Original text: '{}'", text);
                tracing::info!("🧹 Cleaned text: '{}'", clean_text);

                // Check if this looks like a repository name (owner/repo pattern)
                if clean_text.contains('/') && clean_text.split_whitespace().count() == 1 {
                    tracing::info!("🔧 Detected setup request: {}", clean_text);
                    if let Err(e) = state
                        .form_handler
                        .handle_repo_setup(channel_id.clone(), clean_text)
                        .await
                    {
                        tracing::error!("❌ Setup failed: {}", e);
                        let _ = state
                            .slack_client
                            .send_message(
                                &channel_id,
                                &format!("❌ Setup failed: {}", e),
                                thread_ts.as_ref(),
                            )
                            .await;
                    }
                } else {
                    tracing::info!("💬 Processing regular message");
                    // Regular message - process it
                    let slack_message = SlackMessage {
                        channel: channel_id,
                        user: user_id,
                        text: clean_text,
                        thread_ts,
                        ts,
                    };

                    if let Err(e) = state.message_processor.process_message(slack_message).await {
                        tracing::error!("❌ Message processing failed: {}", e);
                    }
                }
            }
            SlackEventCallbackBody::Message(message) => {
                tracing::info!("📬 Message event received");
                tracing::debug!("Full message: {:?}", message);

                // Ignore bot's own messages to prevent loops
                if message.sender.bot_id.is_some() {
                    tracing::debug!("🤖 Ignoring bot's own message");
                    return Ok(());
                }

                // Ignore message updates/edits
                if message.subtype == Some(SlackMessageEventType::MessageChanged) {
                    tracing::debug!("✏️  Ignoring message edit event");
                    return Ok(());
                }

                // Check if this is a channel_join event (bot was invited)
                if message.subtype == Some(SlackMessageEventType::ChannelJoin) {
                    if let Some(channel_id) = message.origin.channel {
                        tracing::info!("🎉 Bot joined channel: {}", channel_id);

                        let channel = ChannelId::new(channel_id.to_string());

                        // Check if already setup
                        if state.form_handler.agent_manager.has_agent(&channel) {
                            tracing::info!("Channel already has an agent configured");
                        } else {
                            tracing::info!("Showing welcome message and setup instructions");
                            if let Err(e) = state.form_handler.show_repo_setup_form(&channel).await
                            {
                                tracing::error!("Failed to show setup form: {}", e);
                            }
                        }
                    }
                } else {
                    // Handle regular messages in threads where bot participated
                    tracing::debug!(
                        "Regular message (subtype: {:?}), skipping for now",
                        message.subtype
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
        tracing::error!("Slack event error: {:#?}", err);
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
            tracing::debug!("🧹 Cleaned up {} old events from cache", removed);
        }
    }
}
