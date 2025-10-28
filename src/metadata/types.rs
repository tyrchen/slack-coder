//! Metadata types for enriched logging

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Channel metadata information
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    /// Channel ID (e.g., C09NU1KFXHT)
    pub id: String,

    /// Channel name without # (e.g., "engineering", "general")
    pub name: String,

    /// Channel type
    pub channel_type: ChannelType,

    /// Is this a private channel?
    pub is_private: bool,

    /// Number of members (if available)
    pub member_count: Option<u32>,

    /// When this info was last fetched
    pub fetched_at: Instant,

    /// Topic/description (optional)
    pub topic: Option<String>,
}

/// Channel type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChannelType {
    PublicChannel,
    PrivateChannel,
    DirectMessage,
    MultiPartyDirectMessage,
}

impl ChannelInfo {
    /// Check if this cache entry is stale (older than TTL)
    pub fn is_stale(&self, ttl: Duration) -> bool {
        self.fetched_at.elapsed() > ttl
    }

    /// Get display name with # prefix for channels
    pub fn display_name(&self) -> String {
        match self.channel_type {
            ChannelType::PublicChannel | ChannelType::PrivateChannel => {
                format!("#{}", self.name)
            }
            _ => self.name.clone(),
        }
    }
}

/// User metadata information
#[derive(Debug, Clone)]
pub struct UserInfo {
    /// User ID (e.g., U09JDBT2MCM)
    pub id: String,

    /// Username/handle (e.g., "john.doe")
    pub name: String,

    /// Real name (e.g., "John Doe")
    pub real_name: Option<String>,

    /// Display name (what shows in Slack)
    pub display_name: Option<String>,

    /// Email (if available)
    pub email: Option<String>,

    /// Is this a bot?
    pub is_bot: bool,

    /// When this info was last fetched
    pub fetched_at: Instant,
}

impl UserInfo {
    /// Check if this cache entry is stale
    pub fn is_stale(&self, ttl: Duration) -> bool {
        self.fetched_at.elapsed() > ttl
    }

    /// Get best available name for display
    pub fn best_name(&self) -> &str {
        self.display_name
            .as_deref()
            .or(self.real_name.as_deref())
            .unwrap_or(&self.name)
    }

    /// Get display name with @ prefix
    pub fn display_name_with_at(&self) -> String {
        format!("@{}", self.best_name())
    }
}

/// Context for enriched logging with both IDs and names
#[derive(Debug, Clone)]
pub struct LogContext {
    /// Raw channel ID (e.g., "C09NU1KFXHT")
    pub channel_id: String,

    /// Channel name without prefix (e.g., "engineering")
    pub channel_name: String,

    /// Channel display name with prefix (e.g., "#engineering")
    pub channel_display: String,

    /// Raw user ID (e.g., "U09JDBT2MCM")
    pub user_id: String,

    /// Username/handle (e.g., "john.doe")
    pub user_name: String,

    /// User display name with @ (e.g., "@John Doe")
    pub user_display: String,
}

impl LogContext {
    /// Create a context with ID-only fallback (when metadata not available)
    pub fn from_ids(channel_id: String, user_id: String) -> Self {
        Self {
            channel_id: channel_id.clone(),
            channel_name: channel_id.clone(),
            channel_display: channel_id.clone(),
            user_id: user_id.clone(),
            user_name: user_id.clone(),
            user_display: format!("@{}", user_id),
        }
    }

    /// Create a context from fetched metadata
    pub fn from_metadata(
        channel_id: String,
        channel_info: Option<&ChannelInfo>,
        user_id: String,
        user_info: Option<&UserInfo>,
    ) -> Self {
        Self {
            channel_id: channel_id.clone(),
            channel_name: channel_info
                .map(|c| c.name.clone())
                .unwrap_or_else(|| channel_id.clone()),
            channel_display: channel_info
                .map(|c| c.display_name())
                .unwrap_or_else(|| channel_id.clone()),
            user_id: user_id.clone(),
            user_name: user_info
                .map(|u| u.name.clone())
                .unwrap_or_else(|| user_id.clone()),
            user_display: user_info
                .map(|u| u.display_name_with_at())
                .unwrap_or_else(|| format!("@{}", user_id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_info_display_name() {
        let channel = ChannelInfo {
            id: "C123".to_string(),
            name: "engineering".to_string(),
            channel_type: ChannelType::PublicChannel,
            is_private: false,
            member_count: Some(42),
            fetched_at: Instant::now(),
            topic: None,
        };

        assert_eq!(channel.display_name(), "#engineering");
    }

    #[test]
    fn test_channel_info_dm_display_name() {
        let channel = ChannelInfo {
            id: "D123".to_string(),
            name: "john.doe".to_string(),
            channel_type: ChannelType::DirectMessage,
            is_private: true,
            member_count: Some(2),
            fetched_at: Instant::now(),
            topic: None,
        };

        assert_eq!(channel.display_name(), "john.doe");
    }

    #[test]
    fn test_user_info_best_name() {
        let user = UserInfo {
            id: "U123".to_string(),
            name: "john.doe".to_string(),
            real_name: Some("John Doe".to_string()),
            display_name: Some("Johnny".to_string()),
            email: None,
            is_bot: false,
            fetched_at: Instant::now(),
        };

        assert_eq!(user.best_name(), "Johnny");
        assert_eq!(user.display_name_with_at(), "@Johnny");
    }

    #[test]
    fn test_user_info_fallback_name() {
        let user = UserInfo {
            id: "U123".to_string(),
            name: "john.doe".to_string(),
            real_name: None,
            display_name: None,
            email: None,
            is_bot: false,
            fetched_at: Instant::now(),
        };

        assert_eq!(user.best_name(), "john.doe");
        assert_eq!(user.display_name_with_at(), "@john.doe");
    }

    #[test]
    fn test_log_context_from_ids() {
        let ctx = LogContext::from_ids("C123".to_string(), "U456".to_string());

        assert_eq!(ctx.channel_id, "C123");
        assert_eq!(ctx.channel_name, "C123");
        assert_eq!(ctx.user_id, "U456");
        assert_eq!(ctx.user_name, "U456");
    }

    #[test]
    fn test_log_context_from_metadata() {
        let channel = ChannelInfo {
            id: "C123".to_string(),
            name: "engineering".to_string(),
            channel_type: ChannelType::PublicChannel,
            is_private: false,
            member_count: None,
            fetched_at: Instant::now(),
            topic: None,
        };

        let user = UserInfo {
            id: "U456".to_string(),
            name: "john.doe".to_string(),
            real_name: Some("John Doe".to_string()),
            display_name: None,
            email: None,
            is_bot: false,
            fetched_at: Instant::now(),
        };

        let ctx = LogContext::from_metadata(
            "C123".to_string(),
            Some(&channel),
            "U456".to_string(),
            Some(&user),
        );

        assert_eq!(ctx.channel_id, "C123");
        assert_eq!(ctx.channel_name, "engineering");
        assert_eq!(ctx.channel_display, "#engineering");
        assert_eq!(ctx.user_id, "U456");
        assert_eq!(ctx.user_name, "john.doe");
        assert_eq!(ctx.user_display, "@John Doe");
    }
}
