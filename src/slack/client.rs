use crate::config::SlackConfig;
use crate::error::{Result, SlackCoderError};
use crate::metadata::{ChannelInfo, ChannelType, UserInfo};
use crate::slack::{ChannelId, MessageTs, ThreadTs, UsageMetrics};
use slack_morphism::prelude::*;
use std::sync::Arc;
use std::time::Instant;

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

    /// Send a message to a channel with Slack markdown formatting
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

        // Unfurl links to show previews
        request.unfurl_links = Some(false);
        request.unfurl_media = Some(false);

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
        tracing::debug!("📋 Fetching channel list from Slack API...");
        let session = self.client.open_session(&self.token);

        let request = SlackApiConversationsListRequest::new().with_types(vec![
            SlackConversationType::Public,
            SlackConversationType::Private,
        ]);

        let response = session
            .conversations_list(&request)
            .await
            .map_err(|e| SlackCoderError::SlackApi(e.to_string()))?;

        tracing::debug!("Received {} total channels", response.channels.len());

        let channels: Vec<ChannelId> = response
            .channels
            .iter()
            .filter(|c| c.flags.is_member.unwrap_or(false))
            .map(|c| {
                tracing::debug!(
                    "  Channel: {} (member: {})",
                    c.id,
                    c.flags.is_member.unwrap_or(false)
                );
                ChannelId::new(c.id.to_string())
            })
            .collect();

        tracing::info!("Found {} channels where bot is a member", channels.len());
        for ch in &channels {
            tracing::debug!("  - {}", ch.as_str());
        }

        Ok(channels)
    }

    /// Send usage metrics as a formatted message
    pub async fn send_metrics(
        &self,
        channel: &ChannelId,
        thread_ts: Option<&ThreadTs>,
        metrics: &UsageMetrics,
    ) -> Result<MessageTs> {
        let text = metrics.format_slack_message();
        self.send_message(channel, &text, thread_ts).await
    }

    /// Send completion notification
    pub async fn send_completion_alert(
        &self,
        channel: &ChannelId,
        thread_ts: Option<&ThreadTs>,
    ) -> Result<MessageTs> {
        let text = "✅ *Task Complete* - All operations finished!";
        self.send_message(channel, text, thread_ts).await
    }

    /// Send shutdown notification
    pub async fn send_shutdown_notice(
        &self,
        channel: &ChannelId,
        session_id: &str,
    ) -> Result<MessageTs> {
        let text = format!("🔴 *Agent Gone*\n\nSession ID: `{}` ended", session_id);
        self.send_message(channel, &text, None).await
    }

    /// Get channel information from Slack API
    pub async fn get_channel_info(&self, channel_id: &str) -> Result<ChannelInfo> {
        let session = self.client.open_session(&self.token);

        let request = SlackApiConversationsInfoRequest::new(SlackChannelId(channel_id.to_string()));

        let response = session
            .conversations_info(&request)
            .await
            .map_err(|e| SlackCoderError::SlackApi(e.to_string()))?;

        let channel = response.channel;

        // Determine channel type
        let channel_type = if channel.flags.is_channel.unwrap_or(false) {
            if channel.flags.is_private.unwrap_or(false) {
                ChannelType::PrivateChannel
            } else {
                ChannelType::PublicChannel
            }
        } else if channel.flags.is_im.unwrap_or(false) {
            ChannelType::DirectMessage
        } else if channel.flags.is_mpim.unwrap_or(false) {
            ChannelType::MultiPartyDirectMessage
        } else {
            ChannelType::PublicChannel // Default fallback
        };

        Ok(ChannelInfo {
            id: channel.id.to_string(),
            name: channel.name.unwrap_or_else(|| channel_id.to_string()),
            channel_type,
            is_private: channel.flags.is_private.unwrap_or(false),
            member_count: channel.num_members.map(|n| n as u32),
            fetched_at: Instant::now(),
            topic: channel.topic.map(|t| t.value),
        })
    }

    /// Get user information from Slack API
    pub async fn get_user_info(&self, user_id: &str) -> Result<UserInfo> {
        let session = self.client.open_session(&self.token);

        let request = SlackApiUsersInfoRequest::new(SlackUserId(user_id.to_string()));

        let response = session
            .users_info(&request)
            .await
            .map_err(|e| SlackCoderError::SlackApi(e.to_string()))?;

        let user = response.user;

        Ok(UserInfo {
            id: user.id.to_string(),
            name: user.name.unwrap_or_else(|| user_id.to_string()),
            real_name: user.real_name,
            display_name: user.profile.as_ref().and_then(|p| p.display_name.clone()),
            email: user
                .profile
                .as_ref()
                .and_then(|p| p.email.as_ref().map(|e| e.to_string())),
            is_bot: user.flags.is_bot.unwrap_or(false),
            fetched_at: Instant::now(),
        })
    }
}
