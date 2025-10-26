use crate::agent::AgentManager;
use crate::error::Result;
use crate::slack::{ChannelId, FormHandler, MessageProcessor, MessageTs, SlackClient, SlackMessage, ThreadTs, UserId};
use slack_morphism::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
struct BotState {
    message_processor: Arc<MessageProcessor>,
    form_handler: Arc<FormHandler>,
    slack_client: Arc<SlackClient>,
}

pub struct EventHandler {
    slack_client: Arc<SlackClient>,
    agent_manager: Arc<AgentManager>,
}

impl EventHandler {
    pub fn new(
        slack_client: Arc<SlackClient>,
        agent_manager: Arc<AgentManager>,
    ) -> Self {
        Self {
            slack_client,
            agent_manager,
        }
    }

    /// Start listening for Slack events using Socket Mode
    pub async fn start(self) -> Result<()> {
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
        };

        let listener_environment = Arc::new(
            SlackClientEventsListenerEnvironment::new(self.slack_client.get_client())
                .with_error_handler(Self::error_handler)
                .with_user_state(bot_state),
        );

        let callbacks =
            SlackSocketModeListenerCallbacks::new().with_push_events(Self::handle_push_event);

        let socket_mode_listener = SlackClientSocketModeListener::new(
            &SlackClientSocketModeConfig::new(),
            listener_environment,
            callbacks,
        );

        // Get app token from client
        let app_token = self.slack_client.get_app_token();

        socket_mode_listener
            .listen_for(&app_token)
            .await
            .map_err(|e| crate::error::SlackCoderError::SlackApi(e.to_string()))?;

        socket_mode_listener.serve().await;

        Ok(())
    }

    async fn handle_push_event(
        event: SlackPushEventCallback,
        _client: Arc<SlackHyperClient>,
        user_state: SlackClientEventsUserState,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Received push event: {:?}", event.event);

        // Extract state
        let state: BotState = {
            let storage = user_state.read().await;
            storage
                .get_user_state::<BotState>()
                .expect("BotState should be set")
                .clone()
        };

        match event.event {
            SlackEventCallbackBody::AppMention(mention) => {
                tracing::info!(
                    "App mentioned in channel: {:?} by user: {:?}",
                    mention.channel,
                    mention.user
                );

                let channel_id = ChannelId::new(mention.channel.to_string());
                let user_id = UserId::new(mention.user.to_string());
                let text = mention.content.text.clone().unwrap_or_default();
                let ts = MessageTs::new(mention.origin.ts.to_string());
                let thread_ts = mention.origin.thread_ts.map(|t| ThreadTs::new(t.to_string()));

                // Strip bot mention from text
                let clean_text = text
                    .split_whitespace()
                    .filter(|w| !w.starts_with("<@"))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string();

                // Check if this looks like a repository name (owner/repo pattern)
                if clean_text.contains('/') && clean_text.split_whitespace().count() == 1 {
                    // Likely a setup request
                    if let Err(e) = state.form_handler.handle_repo_setup(channel_id.clone(), clean_text).await {
                        tracing::error!("Setup failed: {}", e);
                        let _ = state.slack_client
                            .send_message(
                                &channel_id,
                                &format!("Setup failed: {}", e),
                                thread_ts.as_ref(),
                            )
                            .await;
                    }
                } else {
                    // Regular message - process it
                    let slack_message = SlackMessage {
                        channel: channel_id,
                        user: user_id,
                        text: clean_text,
                        thread_ts,
                        ts,
                    };

                    if let Err(e) = state.message_processor.process_message(slack_message).await {
                        tracing::error!("Message processing failed: {}", e);
                    }
                }
            }
            SlackEventCallbackBody::Message(message) => {
                tracing::info!("Message received: {:?}", message);
                // Handle regular messages in threads where bot participated
                // For now, we'll focus on app_mention as primary interaction
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
}
