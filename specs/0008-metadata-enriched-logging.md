# Spec 0008: Metadata-Enriched Logging System

**Status**: Design
**Created**: 2025-10-27
**Priority**: High
**Related**: 0007-improved-structured-logging.md

## Problem Statement

Current logging shows only Slack IDs which are not human-readable:

```
INFO app_mention{channel=C09NU1KFXHT user=U09JDBT2MCM}: App mentioned - message received
```

**Issues:**
1. Channel IDs like `C09NU1KFXHT` are meaningless to humans
2. User IDs like `U09JDBT2MCM` don't tell us who sent the message
3. No context about what workspace/team we're in
4. Hard to debug without constantly looking up IDs in Slack
5. Logs lack the "story" - who did what in which channel

**What we need:**
```
INFO app_mention{channel_id=C09NU1KFXHT channel="engineering" user_id=U09JDBT2MCM user="john.doe"}:
     App mentioned in #engineering by @john.doe: "Can you help me debug this issue..."
```

**Key requirement**: BOTH ID and name must be in structured fields for:
- IDs: Precise identification, correlation, filtering
- Names: Human readability, debugging, understanding context

## Goals

1. **Human-Readable Logs**: Show channel names, usernames, workspace names
2. **Structured + Readable**: Keep structured fields but add human context
3. **Performance**: Cache metadata to avoid excessive Slack API calls
4. **Lazy Loading**: Fetch metadata on-demand, not upfront
5. **Resilient**: Gracefully handle API failures, fall back to IDs
6. **Memory Efficient**: Evict old/unused metadata from cache

## Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        MetadataCache                            │
│  (Global singleton - shared across all channels)                │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │ ChannelCache: DashMap<ChannelId, ChannelInfo>          │  │
│  │ UserCache: DashMap<UserId, UserInfo>                    │  │
│  │ WorkspaceInfo: Team name, domain, etc.                  │  │
│  └─────────────────────────────────────────────────────────┘  │
│                                                                 │
│  Methods:                                                       │
│  - get_channel_info(id) -> Option<ChannelInfo>                │
│  - get_user_info(id) -> Option<UserInfo>                      │
│  - refresh_channel(id) -> Result<ChannelInfo>                 │
│  - refresh_user(id) -> Result<UserInfo>                       │
│  - log_context(channel, user) -> LogContext                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ├──> Lazy fetch from Slack API
                              ├──> TTL-based expiration (1 hour)
                              └──> LRU eviction (max 1000 entries)

┌─────────────────────────────────────────────────────────────────┐
│                       Usage in Code                             │
│                                                                 │
│  let ctx = metadata.log_context(&channel_id, &user_id);        │
│                                                                 │
│  tracing::info!(                                               │
│      channel = %ctx.channel_name,          // "engineering"    │
│      channel_id = %ctx.channel_id,         // C09NU1KFXHT     │
│      user = %ctx.user_name,                // "john.doe"      │
│      user_id = %ctx.user_id,               // U09JDBT2MCM     │
│      "App mentioned in #{} by @{}: {}",                       │
│      ctx.channel_name, ctx.user_name, message                 │
│  );                                                            │
└─────────────────────────────────────────────────────────────────┘
```

## Data Structures

### 1. ChannelInfo

```rust
use std::time::Instant;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// Channel ID (e.g., C09NU1KFXHT)
    pub id: String,

    /// Channel name without # (e.g., "engineering", "general")
    pub name: String,

    /// Channel type: channel, group, im, mpim
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
```

### 2. UserInfo

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
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
```

### 3. WorkspaceInfo

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    /// Team/workspace ID
    pub id: String,

    /// Team name (e.g., "Acme Corp")
    pub name: String,

    /// Team domain (e.g., "acmecorp")
    pub domain: String,

    /// When this info was last fetched
    pub fetched_at: Instant,
}
```

### 4. MetadataCache (Main Cache Manager)

```rust
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub struct MetadataCache {
    /// Slack API client
    slack_client: Arc<SlackClient>,

    /// Channel metadata cache
    channels: Arc<DashMap<String, ChannelInfo>>,

    /// User metadata cache
    users: Arc<DashMap<String, UserInfo>>,

    /// Workspace info (singleton)
    workspace: Arc<RwLock<Option<WorkspaceInfo>>>,

    /// Cache TTL (how long before refresh)
    ttl: Duration,

    /// Statistics
    stats: Arc<RwLock<CacheStats>>,
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub channel_hits: u64,
    pub channel_misses: u64,
    pub user_hits: u64,
    pub user_misses: u64,
    pub api_calls: u64,
    pub api_errors: u64,
}

impl MetadataCache {
    /// Create a new metadata cache
    pub fn new(slack_client: Arc<SlackClient>) -> Self {
        Self {
            slack_client,
            channels: Arc::new(DashMap::new()),
            users: Arc::new(DashMap::new()),
            workspace: Arc::new(RwLock::new(None)),
            ttl: Duration::from_secs(3600), // 1 hour default
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
                return Some(info.clone());
            }
        }

        // Cache miss or stale - fetch from API (only this specific channel)
        self.stats.write().await.channel_misses += 1;
        self.fetch_channel_info(channel_id).await.ok()
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
                return Some(info.clone());
            }
        }

        // Cache miss or stale - fetch from API (only this specific user)
        self.stats.write().await.user_misses += 1;
        self.fetch_user_info(user_id).await.ok()
    }

    /// Fetch channel info from Slack API
    async fn fetch_channel_info(&self, channel_id: &str) -> Result<ChannelInfo> {
        self.stats.write().await.api_calls += 1;

        // Call Slack API: conversations.info
        match self.slack_client.get_channel_info(channel_id).await {
            Ok(info) => {
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

        // Call Slack API: users.info
        match self.slack_client.get_user_info(user_id).await {
            Ok(info) => {
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
    pub async fn log_context(&self, channel_id: &str, user_id: &str) -> LogContext {
        let channel = self.get_channel_info(channel_id).await;
        let user = self.get_user_info(user_id).await;

        LogContext {
            channel_id: channel_id.to_string(),
            channel_name: channel.as_ref().map(|c| c.name.clone()).unwrap_or_else(|| channel_id.to_string()),
            channel_display: channel.as_ref().map(|c| c.display_name()).unwrap_or_else(|| channel_id.to_string()),
            user_id: user_id.to_string(),
            user_name: user.as_ref().map(|u| u.name.clone()).unwrap_or_else(|| user_id.to_string()),
            user_display: user.as_ref().map(|u| u.display_name_with_at()).unwrap_or_else(|| format!("@{}", user_id)),
        }
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Clear stale entries (for periodic cleanup)
    pub async fn cleanup_stale(&self) {
        let ttl = self.ttl;

        // Clean channels
        self.channels.retain(|_, info| !info.is_stale(ttl));

        // Clean users
        self.users.retain(|_, info| !info.is_stale(ttl));
    }
}
```

### 5. LogContext (Helper for Logging)

```rust
/// Context for enriched logging
#[derive(Debug, Clone)]
pub struct LogContext {
    /// Raw channel ID
    pub channel_id: String,

    /// Channel name without prefix (e.g., "engineering")
    pub channel_name: String,

    /// Channel display name with prefix (e.g., "#engineering")
    pub channel_display: String,

    /// Raw user ID
    pub user_id: String,

    /// Username/handle (e.g., "john.doe")
    pub user_name: String,

    /// User display name with @ (e.g., "@John Doe")
    pub user_display: String,
}
```

## Slack API Integration

### New Methods in SlackClient

```rust
impl SlackClient {
    /// Get channel information
    pub async fn get_channel_info(&self, channel_id: &str) -> Result<ChannelInfo> {
        let session = self.client.open_session(&self.token);

        let request = SlackApiConversationsInfoRequest::new(
            SlackChannelId(channel_id.into())
        );

        let response = session
            .conversations_info(&request)
            .await
            .map_err(|e| SlackCoderError::SlackApi(e.to_string()))?;

        Ok(ChannelInfo {
            id: response.channel.id.to_string(),
            name: response.channel.name.unwrap_or_default(),
            channel_type: if response.channel.is_channel.unwrap_or(false) {
                if response.channel.is_private.unwrap_or(false) {
                    ChannelType::PrivateChannel
                } else {
                    ChannelType::PublicChannel
                }
            } else if response.channel.is_im.unwrap_or(false) {
                ChannelType::DirectMessage
            } else {
                ChannelType::MultiPartyDirectMessage
            },
            is_private: response.channel.is_private.unwrap_or(false),
            member_count: response.channel.num_members.map(|n| n as u32),
            fetched_at: Instant::now(),
            topic: response.channel.topic.and_then(|t| t.value),
        })
    }

    /// Get user information
    pub async fn get_user_info(&self, user_id: &str) -> Result<UserInfo> {
        let session = self.client.open_session(&self.token);

        let request = SlackApiUsersInfoRequest::new(
            SlackUserId(user_id.into())
        );

        let response = session
            .users_info(&request)
            .await
            .map_err(|e| SlackCoderError::SlackApi(e.to_string()))?;

        Ok(UserInfo {
            id: response.user.id.to_string(),
            name: response.user.name,
            real_name: response.user.real_name,
            display_name: response.user.profile.and_then(|p| p.display_name),
            email: response.user.profile.and_then(|p| p.email),
            is_bot: response.user.is_bot.unwrap_or(false),
            fetched_at: Instant::now(),
        })
    }
}
```

## Usage Examples

### Example 1: Event Handler

```rust
// In EventHandler::new()
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

// In process_event() for app_mention
async fn process_event(...) {
    match event.event {
        SlackEventCallbackBody::AppMention(mention) => {
            let channel_id = mention.channel.to_string();
            let user_id = mention.user.to_string();

            // Get enriched context
            let ctx = self.metadata_cache
                .log_context(&channel_id, &user_id)
                .await;

            let span = tracing::info_span!(
                "app_mention",
                channel_id = %ctx.channel_id,        // C09NU1KFXHT
                channel = %ctx.channel_name,         // "engineering"
                user_id = %ctx.user_id,              // U09JDBT2MCM
                user = %ctx.user_name,               // "john.doe"
            );
            let _guard = span.enter();

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
        }
    }
}
```

**Structured fields explanation:**
- `channel_id`: For filtering/correlation (e.g., `filter(channel_id == "C09NU1KFXHT")`)
- `channel`: For human reading (e.g., "engineering")
- `user_id`: For filtering/correlation (e.g., `filter(user_id == "U09JDBT2MCM")`)
- `user`: For human reading (e.g., "john.doe")

### Example 2: Message Processor

```rust
pub async fn process_message(&self, message: SlackMessage) -> Result<()> {
    // Get enriched context
    let ctx = self.metadata_cache
        .log_context(message.channel.as_str(), message.user.as_str())
        .await;

    let span = tracing::info_span!(
        "process_message",
        channel_id = %ctx.channel_id,
        channel = %ctx.channel_name,
        user_id = %ctx.user_id,
        user = %ctx.user_name,
    );
    let _guard = span.enter();

    tracing::info!(
        channel_id = %ctx.channel_id,
        channel = %ctx.channel_display,
        user_id = %ctx.user_id,
        user = %ctx.user_display,
        message = %message_preview,
        "Processing message in {} from {}: \"{}\"",
        ctx.channel_display,
        ctx.user_display,
        message_preview
    );
}
```

## Expected Log Output

### Before (Current - Only IDs)
```
2025-10-27T16:07:22.981Z INFO app_mention{channel=C09NU1KFXHT user=U09JDBT2MCM}:
    App mentioned - message received
```

### After (With Metadata Enrichment)
```
2025-10-27T16:07:22.981Z INFO app_mention{channel_id=C09NU1KFXHT channel="engineering" user_id=U09JDBT2MCM user="john.doe"}:
    App mentioned in #engineering by @John Doe: "Can you help me debug this issue with the API endpoint?"
```

**Benefits of having both ID and name:**

1. **Filtering by ID** (precise):
   ```bash
   # Find all activity in a specific channel
   grep 'channel_id=C09NU1KFXHT' app.log
   ```

2. **Filtering by name** (readable):
   ```bash
   # Find all activity in engineering channel (human-readable)
   grep 'channel="engineering"' app.log
   ```

3. **Correlation** (across systems):
   - Use ID to correlate with Slack API responses, webhooks, other services
   - Use name to correlate with human communication ("check logs for #engineering")

4. **JSON output** (for log aggregation):
   ```json
   {
     "level": "INFO",
     "span": {
       "channel_id": "C09NU1KFXHT",
       "channel": "engineering",
       "user_id": "U09JDBT2MCM",
       "user": "john.doe"
     },
     "message": "App mentioned in #engineering by @John Doe: \"Can you help...\""
   }
   ```
   Both fields available for Elasticsearch/Datadog queries!

## Implementation Plan

### Phase 1: Core Infrastructure (2-3 hours)

1. **Create metadata module** (`src/metadata/mod.rs`)
   - Define `ChannelInfo`, `UserInfo`, `WorkspaceInfo` structs
   - Define `LogContext` helper
   - Add serialization support

2. **Create MetadataCache** (`src/metadata/cache.rs`)
   - Implement DashMap-based caching
   - Add TTL support
   - Add statistics tracking
   - Implement `get_channel_info()`, `get_user_info()`
   - Implement `log_context()` helper

3. **Add Slack API methods** (`src/slack/client.rs`)
   - Implement `get_channel_info()`
   - Implement `get_user_info()`
   - Add error handling

4. **Update lib.rs**
   - Export metadata module
   - Add metadata to public API

### Phase 2: Integration (1-2 hours)

5. **Update EventHandler**
   - Add `metadata_cache` field
   - Use `log_context()` in app_mention handler
   - Use `log_context()` in message handler

6. **Update MessageProcessor**
   - Add `metadata_cache` field
   - Use enriched context in process_message()
   - Use enriched context in forward_to_agent()

7. **Update main.rs**
   - Create MetadataCache instance
   - Pass to EventHandler and MessageProcessor

### Phase 3: Optimization (1 hour)

8. **Add background cleanup task**
   - Spawn task to cleanup stale entries every 15 minutes
   - Log cache statistics periodically

9. **Add cache warming**
   - Pre-populate cache on startup with known channels
   - Implement batch fetching if possible

10. **Error handling improvements**
    - Graceful degradation when API fails
    - Retry logic for transient failures

### Phase 4: Testing (1 hour)

11. **Unit tests**
    - Test cache hit/miss logic
    - Test TTL expiration
    - Test fallback to IDs

12. **Integration tests**
    - Test with real Slack workspace
    - Verify log output quality
    - Measure performance impact

## Caching Strategy: Lazy-Loading Only

### Important: NO Bulk Pre-fetching

**We do NOT pre-fetch all workspace users** - that could be thousands of users!

**Strategy: Lazy-load on demand**
- Only fetch channel info when bot is added to that channel
- Only fetch user info when that user interacts with the bot
- Cache grows organically based on actual usage

**Example**:
1. Bot added to #engineering → fetch channel info for #engineering
2. @john.doe mentions bot → fetch user info for john.doe
3. @jane.smith mentions bot → fetch user info for jane.smith
4. Cache now contains: 1 channel, 2 users (not 10,000 workspace users!)

### Memory Usage

- **Estimated memory per entry**:
  - ChannelInfo: ~200 bytes
  - UserInfo: ~300 bytes

- **Realistic usage (small team)**:
  - 10 channels + 20 active users = ~8 KB

- **Realistic usage (large team)**:
  - 50 channels + 200 active users = ~70 KB

- **Even with 1000 users interacting over time**:
  - Total: ~310 KB (still negligible)

**Key point**: Cache size grows with actual bot usage, not workspace size!

### API Call Reduction

- **Without cache**:
  - Every message = 2 API calls (channel + user)
  - 100 messages = 200 API calls

- **With cache (1h TTL)**:
  - First message = 2 API calls
  - Next 99 messages in same channel = 0 API calls
  - **Reduction: 99%**

### Latency Impact

- **Cache hit**: <1ms (DashMap lookup)
- **Cache miss**: ~50-200ms (Slack API call)
- **Overall impact**: Minimal since most lookups will be cache hits

## Error Handling Strategy

### Graceful Degradation

```rust
pub async fn log_context(&self, channel_id: &str, user_id: &str) -> LogContext {
    let channel = self.get_channel_info(channel_id).await;
    let user = self.get_user_info(user_id).await;

    LogContext {
        channel_id: channel_id.to_string(),
        // Fallback to ID if fetch fails
        channel_name: channel.as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| channel_id.to_string()),
        // ... similar for other fields
    }
}
```

If Slack API fails, logs will still show IDs - no worse than current state.

## Configuration

Add to `config.toml`:

```toml
[metadata_cache]
# Cache TTL in seconds (default: 3600 = 1 hour)
ttl_secs = 3600

# Maximum cache size (default: 10000 entries)
max_size = 10000

# Cleanup interval in seconds (default: 900 = 15 minutes)
cleanup_interval_secs = 900

# Enable/disable metadata enrichment (default: true)
enabled = true
```

## Migration Strategy

1. **Add metadata module** without changing existing logs
2. **Test cache functionality** independently
3. **Gradually migrate** high-value log statements
4. **Monitor performance** impact
5. **Roll out** to all log statements

## Success Metrics

1. **Developer Experience**
   - Developers can read logs without looking up IDs
   - "Story" of what happened is clear from logs

2. **Performance**
   - <5% increase in memory usage
   - <1% increase in latency (P99)
   - >90% cache hit rate after warmup

3. **Reliability**
   - Graceful degradation if Slack API unavailable
   - No log failures due to metadata fetching

## Optional Optimization: Pre-warm Channel Members

**Not implemented in initial version**, but could be added later:

When bot joins a channel, optionally fetch member list and pre-populate user cache:

```rust
/// Optionally pre-warm cache with channel members (disabled by default)
pub async fn warm_channel_members(&self, channel_id: &str) -> Result<()> {
    if !self.config.prewarm_channel_members {
        return Ok(()); // Feature disabled
    }

    // Fetch channel members (conversations.members API)
    let members = self.slack_client.get_channel_members(channel_id).await?;

    // Batch fetch user info (max 100 at a time to avoid rate limits)
    for chunk in members.chunks(100) {
        // Fetch in parallel
        let futures: Vec<_> = chunk.iter()
            .map(|user_id| self.fetch_user_info(user_id))
            .collect();

        futures::future::join_all(futures).await;
    }

    Ok(())
}
```

**Pros**:
- First message in channel has all user names cached
- No API calls during first conversation

**Cons**:
- Wastes API calls if some members never interact with bot
- Rate limit concerns (100+ API calls per channel join)
- Most users in a channel may never mention the bot

**Recommendation**: Start with pure lazy-loading, add this only if needed.

## Future Enhancements

1. **Persistent cache** across restarts (Redis/disk)
2. **Batch API calls** to fetch multiple users/channels at once
3. **Webhook updates** to keep cache fresh when channels/users change
4. **Workspace context** in all logs (team name)
5. **Message threading** info (parent message context)
6. **Optional channel member pre-warming** (see above)

## Conclusion

This metadata enrichment system will make logs significantly more useful while maintaining performance. The lazy-loading cache ensures we only pay the API cost when necessary, and graceful fallback ensures reliability.

Estimated total implementation time: **5-7 hours**
