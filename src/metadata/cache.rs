//! Metadata cache for lazy-loading channel and user information

use crate::error::Result;
use crate::metadata::types::{ChannelInfo, LogContext, UserInfo};
use crate::slack::SlackClient;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Cache statistics for monitoring
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub channel_hits: u64,
    pub channel_misses: u64,
    pub user_hits: u64,
    pub user_misses: u64,
    pub api_calls: u64,
    pub api_errors: u64,
}

/// Metadata cache with lazy-loading from Slack API
///
/// This cache only fetches data on-demand when users/channels interact with the bot.
/// It does NOT pre-fetch all workspace users or channels.
pub struct MetadataCache {
    /// Slack API client
    slack_client: Arc<SlackClient>,

    /// Channel metadata cache (lazy-populated)
    channels: Arc<DashMap<String, ChannelInfo>>,

    /// User metadata cache (lazy-populated)
    users: Arc<DashMap<String, UserInfo>>,

    /// Cache TTL (how long before refresh)
    ttl: Duration,

    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
}

impl MetadataCache {
    /// Create a new metadata cache
    pub fn new(slack_client: Arc<SlackClient>) -> Self {
        Self::with_ttl(slack_client, Duration::from_secs(3600))
    }

    /// Create a new metadata cache with custom TTL
    pub fn with_ttl(slack_client: Arc<SlackClient>, ttl: Duration) -> Self {
        tracing::info!(
            ttl_secs = ttl.as_secs(),
            "Creating metadata cache with lazy-loading"
        );

        Self {
            slack_client,
            channels: Arc::new(DashMap::new()),
            users: Arc::new(DashMap::new()),
            ttl,
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Get channel info (fetch if not cached or stale)
    ///
    /// This is LAZY - only fetches when actually needed.
    /// We do NOT pre-populate the cache with all workspace channels.
    pub async fn get_channel_info(&self, channel_id: &str) -> Option<ChannelInfo> {
        // Check cache first
        if let Some(info) = self.channels.get(channel_id) {
            if !info.is_stale(self.ttl) {
                self.stats.write().await.channel_hits += 1;
                tracing::trace!(
                    channel_id = %channel_id,
                    channel = %info.name,
                    "Channel cache hit"
                );
                return Some(info.clone());
            } else {
                tracing::debug!(
                    channel_id = %channel_id,
                    age_secs = info.fetched_at.elapsed().as_secs(),
                    "Channel cache entry stale"
                );
            }
        }

        // Cache miss or stale - fetch from API (only this specific channel)
        self.stats.write().await.channel_misses += 1;
        tracing::debug!(
            channel_id = %channel_id,
            "Channel cache miss, fetching from Slack API"
        );

        match self.fetch_channel_info(channel_id).await {
            Ok(info) => Some(info),
            Err(e) => {
                tracing::warn!(
                    channel_id = %channel_id,
                    error = %e,
                    "Failed to fetch channel info, will use ID as fallback"
                );
                None
            }
        }
    }

    /// Get user info (fetch if not cached or stale)
    ///
    /// This is LAZY - only fetches when a user actually interacts with the bot.
    /// We do NOT pre-populate the cache with all workspace users.
    pub async fn get_user_info(&self, user_id: &str) -> Option<UserInfo> {
        // Check cache first
        if let Some(info) = self.users.get(user_id) {
            if !info.is_stale(self.ttl) {
                self.stats.write().await.user_hits += 1;
                tracing::trace!(
                    user_id = %user_id,
                    user = %info.name,
                    "User cache hit"
                );
                return Some(info.clone());
            } else {
                tracing::debug!(
                    user_id = %user_id,
                    age_secs = info.fetched_at.elapsed().as_secs(),
                    "User cache entry stale"
                );
            }
        }

        // Cache miss or stale - fetch from API (only this specific user)
        self.stats.write().await.user_misses += 1;
        tracing::debug!(
            user_id = %user_id,
            "User cache miss, fetching from Slack API"
        );

        match self.fetch_user_info(user_id).await {
            Ok(info) => Some(info),
            Err(e) => {
                tracing::warn!(
                    user_id = %user_id,
                    error = %e,
                    "Failed to fetch user info, will use ID as fallback"
                );
                None
            }
        }
    }

    /// Fetch channel info from Slack API
    async fn fetch_channel_info(&self, channel_id: &str) -> Result<ChannelInfo> {
        self.stats.write().await.api_calls += 1;

        match self.slack_client.get_channel_info(channel_id).await {
            Ok(info) => {
                tracing::info!(
                    channel_id = %channel_id,
                    channel = %info.name,
                    "Fetched and cached channel info"
                );
                self.channels.insert(channel_id.to_string(), info.clone());
                Ok(info)
            }
            Err(e) => {
                self.stats.write().await.api_errors += 1;
                Err(e)
            }
        }
    }

    /// Fetch user info from Slack API
    async fn fetch_user_info(&self, user_id: &str) -> Result<UserInfo> {
        self.stats.write().await.api_calls += 1;

        match self.slack_client.get_user_info(user_id).await {
            Ok(info) => {
                tracing::info!(
                    user_id = %user_id,
                    user = %info.name,
                    "Fetched and cached user info"
                );
                self.users.insert(user_id.to_string(), info.clone());
                Ok(info)
            }
            Err(e) => {
                self.stats.write().await.api_errors += 1;
                Err(e)
            }
        }
    }

    /// Create a logging context with enriched metadata
    ///
    /// This fetches channel and user info if needed (lazy-loading).
    /// Falls back to IDs if metadata cannot be fetched.
    pub async fn log_context(&self, channel_id: &str, user_id: &str) -> LogContext {
        let channel = self.get_channel_info(channel_id).await;
        let user = self.get_user_info(user_id).await;

        LogContext::from_metadata(
            channel_id.to_string(),
            channel.as_ref(),
            user_id.to_string(),
            user.as_ref(),
        )
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Get current cache sizes
    pub fn cache_sizes(&self) -> (usize, usize) {
        (self.channels.len(), self.users.len())
    }

    /// Clear stale entries (for periodic cleanup)
    pub async fn cleanup_stale(&self) {
        let ttl = self.ttl;
        let initial_channels = self.channels.len();
        let initial_users = self.users.len();

        // Clean channels
        self.channels.retain(|_, info| !info.is_stale(ttl));

        // Clean users
        self.users.retain(|_, info| !info.is_stale(ttl));

        let removed_channels = initial_channels - self.channels.len();
        let removed_users = initial_users - self.users.len();

        if removed_channels > 0 || removed_users > 0 {
            tracing::info!(
                removed_channels = removed_channels,
                removed_users = removed_users,
                remaining_channels = self.channels.len(),
                remaining_users = self.users.len(),
                "Cleaned up stale metadata cache entries"
            );
        }
    }

    /// Log cache statistics (for periodic monitoring)
    pub async fn log_stats(&self) {
        let stats = self.get_stats().await;
        let (channel_count, user_count) = self.cache_sizes();

        let channel_hit_rate = if stats.channel_hits + stats.channel_misses > 0 {
            (stats.channel_hits as f32 / (stats.channel_hits + stats.channel_misses) as f32 * 100.0)
                as u32
        } else {
            0
        };

        let user_hit_rate = if stats.user_hits + stats.user_misses > 0 {
            (stats.user_hits as f32 / (stats.user_hits + stats.user_misses) as f32 * 100.0) as u32
        } else {
            0
        };

        tracing::info!(
            channels_cached = channel_count,
            users_cached = user_count,
            channel_hit_rate = channel_hit_rate,
            user_hit_rate = user_hit_rate,
            api_calls = stats.api_calls,
            api_errors = stats.api_errors,
            "Metadata cache statistics"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SlackConfig;

    #[test]
    fn test_cache_sizes() {
        // Initialize crypto provider for rustls
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let config = SlackConfig {
            bot_token: "xoxb-test".to_string(),
            app_token: "xapp-test".to_string(),
            signing_secret: "test-secret".to_string(),
        };
        let slack_client = Arc::new(SlackClient::new(config).unwrap());
        let cache = MetadataCache::new(slack_client);

        let (channels, users) = cache.cache_sizes();
        assert_eq!(channels, 0);
        assert_eq!(users, 0);
    }
}
