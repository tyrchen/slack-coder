use crate::config::SlackConfig;
use crate::error::{Result, SlackCoderError};
use crate::slack::{ChannelId, MessageTs, ThreadTs};
use slack_morphism::prelude::*;
use std::sync::Arc;

pub struct SlackClient {
    client: Arc<SlackHyperClient>,
    token: SlackApiToken,
}

impl SlackClient {
    pub fn new(config: SlackConfig) -> Result<Self> {
        let connector = SlackClientHyperConnector::new()
            .map_err(|e| SlackCoderError::SlackApi(e.to_string()))?;

        let client = Arc::new(slack_morphism::SlackClient::new(connector));
        let token = SlackApiToken::new(config.bot_token.into());

        Ok(Self { client, token })
    }

    pub fn get_client(&self) -> Arc<SlackHyperClient> {
        self.client.clone()
    }

    pub fn get_app_token(&self) -> SlackApiToken {
        // This will be replaced with proper app token from config
        SlackApiToken::new(
            std::env::var("SLACK_APP_TOKEN")
                .expect("SLACK_APP_TOKEN must be set")
                .into(),
        )
    }

    pub fn get_token(&self) -> &SlackApiToken {
        &self.token
    }

    /// Send a message to a channel
    pub async fn send_message(
        &self,
        channel: &ChannelId,
        text: &str,
        thread_ts: Option<&ThreadTs>,
    ) -> Result<MessageTs> {
        let session = self.client.open_session(&self.token);

        let mut request = SlackApiChatPostMessageRequest::new(
            channel.as_str().into(),
            SlackMessageContent::new().with_text(text.into()),
        );

        if let Some(ts) = thread_ts {
            request.thread_ts = Some(ts.as_str().into());
        }

        let response = session
            .chat_post_message(&request)
            .await
            .map_err(|e| SlackCoderError::SlackApi(e.to_string()))?;

        Ok(MessageTs::new(response.ts.to_string()))
    }

    /// Update an existing message
    pub async fn update_message(
        &self,
        channel: &ChannelId,
        ts: &MessageTs,
        text: &str,
    ) -> Result<()> {
        let session = self.client.open_session(&self.token);

        let request = SlackApiChatUpdateRequest::new(
            channel.as_str().into(),
            SlackMessageContent::new().with_text(text.into()),
            ts.as_str().into(),
        );

        session
            .chat_update(&request)
            .await
            .map_err(|e| SlackCoderError::SlackApi(e.to_string()))?;

        Ok(())
    }

    /// Send a code block with syntax highlighting
    pub async fn send_code_block(
        &self,
        channel: &ChannelId,
        code: &str,
        language: &str,
        thread_ts: Option<&ThreadTs>,
    ) -> Result<MessageTs> {
        let formatted_code = format!("```{}\n{}\n```", language, code);
        self.send_message(channel, &formatted_code, thread_ts).await
    }

    /// Get list of channels where bot is a member
    pub async fn list_channels(&self) -> Result<Vec<ChannelId>> {
        let session = self.client.open_session(&self.token);

        let request = SlackApiConversationsListRequest::new().with_types(vec![
            SlackConversationType::Public,
            SlackConversationType::Private,
        ]);

        let response = session
            .conversations_list(&request)
            .await
            .map_err(|e| SlackCoderError::SlackApi(e.to_string()))?;

        let channels = response
            .channels
            .into_iter()
            .filter(|c| c.flags.is_member.unwrap_or(false))
            .map(|c| ChannelId::new(c.id.to_string()))
            .collect();

        Ok(channels)
    }
}
