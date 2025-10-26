use crate::error::Result;
use crate::slack::{MessageProcessor, SlackClient};
use slack_morphism::prelude::*;
use std::sync::Arc;

pub struct EventHandler {
    #[allow(dead_code)]
    slack_client: Arc<SlackClient>,
    #[allow(dead_code)]
    message_processor: MessageProcessor,
}

impl EventHandler {
    pub fn new(slack_client: Arc<SlackClient>) -> Self {
        let message_processor = MessageProcessor::new(slack_client.clone());

        Self {
            slack_client,
            message_processor,
        }
    }

    /// Start listening for Slack events using Socket Mode
    pub async fn start(self) -> Result<()> {
        let listener_environment = Arc::new(
            SlackClientEventsListenerEnvironment::new(self.slack_client.get_client())
                .with_error_handler(Self::error_handler),
        );

        let callbacks =
            SlackSocketModeListenerCallbacks::new().with_push_events(|event, client, states| {
                Box::pin(Self::handle_push_event(event, client, states))
            });

        let socket_mode_listener = SlackClientSocketModeListener::new(
            &SlackClientSocketModeConfig::new(),
            listener_environment,
            callbacks,
        );

        // TODO: Get app token from config
        let app_token_value: SlackApiTokenValue = std::env::var("SLACK_APP_TOKEN")
            .expect("SLACK_APP_TOKEN must be set")
            .into();
        let app_token = SlackApiToken::new(app_token_value);

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
        _states: SlackClientEventsUserState,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Received push event: {:?}", event.event);

        match event.event {
            SlackEventCallbackBody::AppMention(mention) => {
                tracing::info!("App mentioned in channel: {:?}", mention.channel);
                // TODO: Handle app mention
            }
            SlackEventCallbackBody::Message(message) => {
                tracing::info!("Message received: {:?}", message);
                // TODO: Handle message
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
