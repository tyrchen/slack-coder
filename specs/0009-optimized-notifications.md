# Spec 0009: Optimized Parallel Operations and Consolidated Notifications

**Status**: Design & Implementation
**Created**: 2025-10-27
**Priority**: High
**Related**: 0006-usage-metrics-and-notifications.md

## Problem Statement

### Current Issues

#### Issue 1: Sequential Agent Restoration (SLOW)
**Location**: `src/agent/manager.rs:36-72`

Currently restores agents **sequentially**:
```rust
for channel_id in channels {
    if self.workspace.is_channel_setup(&channel_id).await {
        match self.create_repo_agent(channel_id.clone()).await {
            Ok(agent) => { /* ... */ }
        }
    }
}
```

**Problems**:
- If 10 channels need restoration, takes 10x time
- Each agent creation involves file I/O, API calls
- Startup time scales linearly with channel count
- No parallelization

**Example**: 10 channels × 500ms each = **5 seconds startup time**

#### Issue 2: Per-Channel Startup Notifications (SPAM)
**Location**: `src/agent/manager.rs:137-148`

Currently sends notification **per agent**:
```rust
let notification = format!(
    "🤖 *Agent Ready*\n\nSession ID: `{}`\n\n...",
    session_id
);
slack_client.send_message(&channel_id, &notification, None).await;
```

**Problems**:
- 10 restored channels = 10 Slack messages
- Creates notification spam
- Users get pinged multiple times
- Not scalable

#### Issue 3: Per-Channel Shutdown Notifications (SPAM)
**Location**: `src/main.rs:190-230`

Currently sends shutdown notice **per channel**:
```rust
for (channel_id, session_id) in agents {
    slack_client.send_shutdown_notice(&channel_id, &session_id).await;
}
```

**Problems**:
- Sequential sending (slow)
- Each channel gets separate message
- Users in multiple channels get multiple notifications

#### Issue 4: Multiple Notifications Per Query (SPAM)
**Location**: `src/slack/messages.rs:187-217`

Currently sends **3 separate messages**:
1. Result message
2. Metrics message (triggers notification)
3. Completion alert (triggers notification)

**Problems**:
- Users get 2-3 Slack notifications per query
- Clutters the channel
- Metrics and completion should be in ONE message

## Proposed Solutions

### Solution 1: Parallel Agent Restoration

**Restore all agents in parallel** using `futures::future::join_all()`:

```rust
pub async fn scan_and_restore_channels(&self, slack_client: &SlackClient) -> Result<()> {
    let channels = slack_client.list_channels().await?;

    // Create restore futures for all channels
    let restore_futures: Vec<_> = channels
        .into_iter()
        .filter(|ch| self.workspace.is_channel_setup(ch).await)
        .map(|channel_id| async move {
            self.create_repo_agent(channel_id.clone()).await
                .map(|agent| (channel_id, agent))
        })
        .collect();

    // Execute all in parallel
    let results = futures::future::join_all(restore_futures).await;

    // Insert successful agents
    for result in results {
        if let Ok((channel_id, agent)) = result {
            self.repo_agents.insert(channel_id, Arc::new(Mutex::new(agent)));
        }
    }
}
```

**Benefits**:
- 10 channels restore in ~500ms (not 5 seconds)
- **10x faster startup**
- Bounded concurrency (can limit to 5 at a time if needed)

### Solution 2: Single Consolidated Startup Summary

**Send ONE summary notification** after all agents restored:

```rust
// After all agents restored
let summary = format!(
    "🤖 *All Agents Ready*\n\n\
     Restored {} agent(s) in {} channels:\n\
     {}\n\n\
     I'm ready to help! Type `/help` for commands.",
    agent_count,
    channel_count,
    channel_list  // "• #engineering\n• #backend\n• #frontend"
);

// Send to a designated admin channel or log channel
slack_client.send_message(&admin_channel, &summary, None).await;
```

**Alternative**: Don't send startup notifications at all - agents just work when mentioned.

**Recommendation**: **Remove per-channel startup notifications entirely**. Users know the bot is ready when it responds to their first message.

### Solution 3: Parallel Shutdown Notifications

**Send all shutdown notices in parallel**:

```rust
let notification_futures: Vec<_> = agents
    .into_iter()
    .map(|(channel_id, session_id)| {
        let client = slack_client.clone();
        async move {
            client.send_shutdown_notice(&channel_id, &session_id).await
        }
    })
    .collect();

// Execute all in parallel with overall timeout
tokio::time::timeout(
    Duration::from_secs(10),
    futures::future::join_all(notification_futures)
).await;
```

**Benefits**:
- 10 channels notify in ~200ms (not 30 seconds)
- **50x faster shutdown**
- Still ensures all messages sent before exit

### Solution 4: Consolidated Query Completion Message

**Combine result, metrics, and completion into ONE message**:

```rust
// Instead of 3 separate messages:
// 1. Result
// 2. Metrics
// 3. Completion alert

// Send ONE combined message:
let combined = format!(
    "{}\n\n---\n\n\
     📊 *Query Complete*\n\
     • Tokens: {} input + {} output = **{} total**\n\
     • Cost: {}\n\
     • Duration: {:.2}s\n\
     • Session: `{}`",
    result_text,
    metrics.input_tokens,
    metrics.output_tokens,
    metrics.total_tokens,
    cost_str,
    duration_sec,
    session_id
);

slack_client.send_message(channel, &combined, thread_ts).await;
```

**Benefits**:
- **ONE notification** instead of 2-3
- Cleaner channel (fewer messages)
- All info in one place
- Still shows metrics for tracking

## Design Decisions

### Decision 1: Remove Per-Channel Startup Notifications

**Current**: Each restored channel gets "Agent Ready" message
**Proposed**: Remove these entirely

**Rationale**:
- Users don't need to know when agent starts
- They'll know it's ready when it responds
- Reduces noise significantly
- Aligns with Slack bot best practices

**Implementation**: Delete notification code from `src/agent/manager.rs:137-148`

### Decision 2: Optional Single Admin Notification on Startup

**Proposed**: Send ONE message to a configured admin channel (if configured):

```toml
[slack]
admin_channel = "C123456789"  # Optional: channel for admin notifications
```

```rust
// After all agents restored
if let Some(admin_channel) = &settings.slack.admin_channel {
    let summary = format!(
        "🤖 *Slack Bot Started*\n\
         • Restored {} agents in {} channels\n\
         • Startup time: {:.2}s\n\
         • Ready to receive messages",
        restored_count,
        total_channels,
        startup_duration
    );
    slack_client.send_message(&admin_channel, &summary, None).await;
}
```

**Benefits**:
- Admins know when bot restarts
- Single notification in admin channel
- Regular users not spammed
- Optional (disabled by default)

### Decision 3: Parallel Shutdown with Consolidated Summary

**Proposed**:
1. Send all "Agent Gone" messages **in parallel**
2. Optionally send ONE summary to admin channel

```rust
// Parallel shutdown notices
futures::future::join_all(notification_futures).await;

// Optional: Single summary to admin channel
if let Some(admin_channel) = &settings.slack.admin_channel {
    let summary = format!(
        "🔴 *Slack Bot Shutdown*\n\
         • {} agents disconnected\n\
         • {} shutdown notices sent\n\
         • Shutdown reason: {}",
        agent_count,
        success_count,
        signal_name
    );
    slack_client.send_message(&admin_channel, &summary, None).await;
}
```

### Decision 4: Single Combined Completion Message

**Proposed**: Combine result + metrics into ONE message

**Option A**: Append metrics to result message
```
[Agent's response text]

---
📊 *Query Complete*
• Tokens: 2598 total • Cost: $0.0042 • Duration: 1.5s • Session: session-123
```

**Option B**: Send result, then ONE follow-up with metrics + completion
```
Message 1: [Agent's response]
Message 2: 📊 Query Complete - 2598 tokens, $0.0042, 1.5s (Session: session-123)
```

**Recommendation**: **Option B** - 2 messages instead of 3
- First message: Clean result (easy to read)
- Second message: Compact metrics + completion (ONE notification)

## Implementation Plan

### Phase 1: Parallel Operations (High Impact)

#### 1.1 Parallel Agent Restoration
**File**: `src/agent/manager.rs:36-72`

```rust
pub async fn scan_and_restore_channels(&self, slack_client: &SlackClient) -> Result<()> {
    let span = tracing::info_span!("scan_channels");
    let _guard = span.enter();

    let start = Instant::now();
    let channels = slack_client.list_channels().await?;

    tracing::info!(
        total_channels = channels.len(),
        "Scanning for existing setups"
    );

    // Check which channels are setup (can be done in parallel too)
    let setup_checks: Vec<_> = channels
        .into_iter()
        .map(|ch| {
            let workspace = self.workspace.clone();
            async move {
                if workspace.is_channel_setup(&ch).await {
                    Some(ch)
                } else {
                    None
                }
            }
        })
        .collect();

    let setup_channels: Vec<_> = futures::future::join_all(setup_checks)
        .await
        .into_iter()
        .flatten()
        .collect();

    tracing::info!(
        setup_count = setup_channels.len(),
        "Found channels with existing setup"
    );

    // Restore agents in parallel
    let restore_futures: Vec<_> = setup_channels
        .into_iter()
        .map(|channel_id| {
            let settings = self.settings.clone();
            let workspace = self.workspace.clone();
            let progress_tracker = self.progress_tracker.clone();

            async move {
                match Self::create_repo_agent_static(
                    channel_id.clone(),
                    workspace,
                    settings,
                    progress_tracker,
                )
                .await
                {
                    Ok(agent) => Some((channel_id, agent)),
                    Err(e) => {
                        tracing::warn!(
                            channel_id = %channel_id.as_str(),
                            error = %e,
                            "Failed to restore agent"
                        );
                        None
                    }
                }
            }
        })
        .collect();

    let results = futures::future::join_all(restore_futures).await;

    // Insert successful agents
    let mut restored_count = 0;
    for result in results {
        if let Some((channel_id, agent)) = result {
            self.repo_agents
                .insert(channel_id.clone(), Arc::new(Mutex::new(agent)));
            restored_count += 1;
        }
    }

    let duration = start.elapsed();
    tracing::info!(
        restored = restored_count,
        duration_ms = duration.as_millis() as u64,
        "Agent restoration complete"
    );

    Ok(())
}
```

#### 1.2 Parallel Shutdown Notifications
**File**: `src/main.rs:170-257`

```rust
async fn send_shutdown_notifications(...) {
    let agents = agent_manager.get_all_active_agents().await;

    // Send all in parallel
    let futures: Vec<_> = agents
        .into_iter()
        .map(|(channel_id, session_id)| {
            let client = slack_client.clone();
            async move {
                tokio::time::timeout(
                    Duration::from_secs(3),
                    client.send_shutdown_notice(&channel_id, &session_id),
                )
                .await
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    // Count successes/failures
    let success_count = results.iter()
        .filter(|r| matches!(r, Ok(Ok(_))))
        .count();

    tracing::info!(
        succeeded = success_count,
        total = results.len(),
        "Shutdown notifications sent"
    );
}
```

### Phase 2: Remove Per-Channel Notifications

#### 2.1 Remove "Agent Ready" Message
**File**: `src/agent/manager.rs:137-148`

**Action**: **DELETE** these lines:
```rust
// DELETE THIS ENTIRE BLOCK
let session_id = agent.get_session_id();
let notification = format!("🤖 *Agent Ready*\n\n...");
let _ = slack_client.send_message(&channel_id, &notification, None).await;
```

**Impact**: No more per-channel startup spam

#### 2.2 Remove "Agent Gone" Per-Channel Messages
**File**: `src/main.rs:170-257`

**Action**: Remove individual channel notifications, keep only summary logging

**Before**: Send "Agent Gone" to each channel
**After**: Just disconnect agents, log summary

### Phase 3: Consolidate Query Completion Notifications

#### 3.1 Combine Metrics + Completion into Result Message
**File**: `src/slack/messages.rs:187-217`

**Current** (3 messages, 2+ notifications):
1. Result message
2. Metrics message ← **notification**
3. Completion alert ← **notification**

**Proposed** (1 message, 0-1 notifications):

**Option A**: Append metrics to result (no additional notification)
```rust
let combined = format!(
    "{}\n\n---\n📊 **Query Complete** • {} tokens • ${:.4} • {:.1}s",
    result_text,
    metrics.total_tokens,
    metrics.cost_usd.unwrap_or(0.0),
    metrics.duration_ms as f64 / 1000.0
);
slack_client.send_message(channel, &combined, thread_ts).await;
```

**Option B**: Result + compact metrics footer (1 notification)
```rust
// Message 1: Result
slack_client.send_message(channel, &result_text, thread_ts).await;

// Message 2: Compact metrics (ONE notification)
let metrics_compact = format!(
    "✅ *Complete* • {} tokens • ${:.4} • {:.1}s • session: `{}`",
    metrics.total_tokens,
    metrics.cost_usd.unwrap_or(0.0),
    metrics.duration_ms as f64 / 1000.0,
    metrics.session_id
);
slack_client.send_message(channel, &metrics_compact, thread_ts).await;
```

**Recommendation**: **Option A** - Single message, zero additional notifications

Users can see metrics at the bottom of the response. If they want a notification, the main result already triggers one.

## Detailed Implementation

### Change 1: Make `create_repo_agent` Static Method

**File**: `src/agent/manager.rs`

```rust
impl AgentManager {
    // Make this a static associated function so it can be called in parallel
    async fn create_repo_agent_static(
        channel_id: ChannelId,
        workspace: Arc<Workspace>,
        settings: Arc<Settings>,
        progress_tracker: Arc<ProgressTracker>,
    ) -> Result<RepoAgent> {
        // Same logic as before, but doesn't need &self
        let mut agent = RepoAgent::new(
            channel_id.clone(),
            workspace,
            settings,
            progress_tracker.clone(),
        )
        .await?;

        agent.connect().await?;

        // NO notification sent here anymore

        Ok(agent)
    }

    // Keep the old method that calls the static one
    async fn create_repo_agent(&self, channel_id: ChannelId) -> Result<RepoAgent> {
        Self::create_repo_agent_static(
            channel_id,
            self.workspace.clone(),
            self.settings.clone(),
            self.progress_tracker.clone(),
        )
        .await
    }
}
```

### Change 2: Parallel Restoration

**File**: `src/agent/manager.rs:36-72`

```rust
pub async fn scan_and_restore_channels(&self, slack_client: &SlackClient) -> Result<()> {
    let span = tracing::info_span!("scan_channels");
    let _guard = span.enter();

    let start = std::time::Instant::now();
    let channels = slack_client.list_channels().await?;

    tracing::info!(
        total_channels = channels.len(),
        "Scanning for existing setups"
    );

    // Filter to channels that are setup (sequential for now, could parallelize)
    let mut setup_channels = Vec::new();
    for channel_id in channels {
        if self.workspace.is_channel_setup(&channel_id).await {
            setup_channels.push(channel_id);
        }
    }

    tracing::info!(
        setup_count = setup_channels.len(),
        "Found channels with existing setup"
    );

    if setup_channels.is_empty() {
        tracing::info!("No channels to restore");
        return Ok(());
    }

    // Restore all agents in parallel
    let restore_futures: Vec<_> = setup_channels
        .into_iter()
        .map(|channel_id| {
            let workspace = self.workspace.clone();
            let settings = self.settings.clone();
            let progress_tracker = self.progress_tracker.clone();

            async move {
                Self::create_repo_agent_static(
                    channel_id.clone(),
                    workspace,
                    settings,
                    progress_tracker,
                )
                .await
                .map(|agent| (channel_id, agent))
                .map_err(|e| (channel_id.clone(), e))
            }
        })
        .collect();

    tracing::info!(
        agent_count = restore_futures.len(),
        "Restoring agents in parallel"
    );

    let results = futures::future::join_all(restore_futures).await;

    // Process results
    let mut restored_count = 0;
    let mut failed_count = 0;

    for result in results {
        match result {
            Ok((channel_id, agent)) => {
                self.repo_agents
                    .insert(channel_id.clone(), Arc::new(Mutex::new(agent)));
                restored_count += 1;
            }
            Err((channel_id, e)) => {
                failed_count += 1;
                tracing::warn!(
                    channel_id = %channel_id.as_str(),
                    error = %e,
                    "Failed to restore agent"
                );
            }
        }
    }

    let duration = start.elapsed();
    tracing::info!(
        restored = restored_count,
        failed = failed_count,
        duration_ms = duration.as_millis() as u64,
        "Agent restoration complete"
    );

    Ok(())
}
```

### Change 3: Parallel Shutdown Notifications

**File**: `src/main.rs:170-257`

```rust
async fn send_shutdown_notifications(
    agent_manager: &Arc<AgentManager>,
    slack_client: &Arc<SlackClient>,
) {
    tracing::info!("Sending shutdown notifications to all channels");

    let agents = agent_manager.get_all_active_agents().await;
    tracing::info!(agent_count = agents.len(), "Found active agents");

    if agents.is_empty() {
        tracing::info!("No active agents to notify");
        return;
    }

    // Send all shutdown notices in parallel
    let notification_futures: Vec<_> = agents
        .into_iter()
        .map(|(channel_id, session_id)| {
            let client = slack_client.clone();
            async move {
                let result = tokio::time::timeout(
                    Duration::from_secs(3),
                    client.send_shutdown_notice(&channel_id, &session_id),
                )
                .await;

                (channel_id, session_id, result)
            }
        })
        .collect();

    let total = notification_futures.len();
    tracing::info!(total = total, "Sending shutdown notices in parallel");

    let results = futures::future::join_all(notification_futures).await;

    // Count successes/failures
    let mut success_count = 0;
    let mut failure_count = 0;

    for (channel_id, _session_id, result) in results {
        match result {
            Ok(Ok(_)) => {
                success_count += 1;
                tracing::debug!(
                    channel_id = %channel_id.as_str(),
                    "Shutdown notice sent"
                );
            }
            Ok(Err(e)) => {
                failure_count += 1;
                tracing::warn!(
                    channel_id = %channel_id.as_str(),
                    error = %e,
                    "Failed to send shutdown notice"
                );
            }
            Err(_) => {
                failure_count += 1;
                tracing::warn!(
                    channel_id = %channel_id.as_str(),
                    "Timeout sending shutdown notice"
                );
            }
        }
    }

    let success_rate = if total > 0 {
        (success_count as f32 / total as f32 * 100.0) as u32
    } else {
        0
    };

    tracing::info!(
        succeeded = success_count,
        failed = failure_count,
        total = total,
        success_rate = success_rate,
        "Shutdown notification summary"
    );
}
```

### Change 4: Consolidated Query Completion

**File**: `src/slack/messages.rs:143-217`

```rust
// Send response to Slack
if !final_result.is_empty() {
    let slack_formatted = markdown_to_slack(&final_result);

    // Append metrics to result if available
    let final_message = if let Some(result_msg) = &result_message {
        let metrics = UsageMetrics::from_result_message(result_msg);

        // Compact metrics footer
        let metrics_footer = format!(
            "\n\n---\n📊 **Complete** • {} tokens • ${:.4} • {:.1}s",
            metrics.total_tokens,
            metrics.cost_usd.unwrap_or(0.0),
            metrics.duration_ms as f64 / 1000.0
        );

        format!("{}{}", slack_formatted, metrics_footer)
    } else {
        slack_formatted
    };

    // Send ONE message with result + metrics
    if final_message.len() > MAX_SLACK_MESSAGE_SIZE {
        // Chunking logic...
    } else {
        self.slack_client
            .send_message(channel, &final_message, thread_ts)
            .await?;
    }

    tracing::info!(
        response_len = final_message.len(),
        "Response with metrics sent"
    );
}
```

## Expected Behavior Changes

### Startup (Before)
```
[Channel #engineering]
🤖 Agent Ready
Session ID: session-C123-...

[Channel #backend]
🤖 Agent Ready
Session ID: session-C456-...

[Channel #frontend]
🤖 Agent Ready
Session ID: session-C789-...
```
**3 notifications, 3 messages**

### Startup (After)
```
[No messages to users]
[Logs show: "Agent restoration complete - restored=3 duration_ms=523"]
```
**0 notifications, 0 messages** (silent startup - users only notice when they use bot)

### Query Completion (Before)
```
[Message 1] Here's the analysis... [Result]
[Message 2] 📊 Query Metrics... [Notification]
[Message 3] ✅ Task Complete! [Notification]
```
**3 messages, 2-3 notifications**

### Query Completion (After - Option A)
```
Here's the analysis...
[full response]

---
📊 Complete • 2598 tokens • $0.0042 • 1.5s
```
**1 message, 1 notification** (from the main result)

### Shutdown (Before)
```
[Sequential, 10s for 10 channels]
[Channel #engineering] 🔴 Agent Gone...
[Channel #backend] 🔴 Agent Gone...
[Channel #frontend] 🔴 Agent Gone...
```
**10 notifications, 30 seconds**

### Shutdown (After)
```
[Parallel, 3s total]
All 10 channels get "Agent Gone" simultaneously
```
**10 notifications (unavoidable), but 3 seconds instead of 30**

## Performance Impact

### Startup Time
- **Before**: 10 channels × 500ms = 5 seconds
- **After**: 10 channels in parallel = 500ms
- **Improvement**: **10x faster**

### Shutdown Time
- **Before**: 10 channels × 3s timeout = 30 seconds
- **After**: 10 channels in parallel = 3 seconds
- **Improvement**: **10x faster**

### Notification Reduction
- **Startup**: 10 notifications → 0 notifications (**100% reduction**)
- **Per query**: 3 messages → 1 message (**67% reduction**)
- **Shutdown**: 10 sequential → 10 parallel (same count, 10x faster)

## Migration Checklist

- [ ] Add `futures` to Cargo.toml if not already present
- [ ] Refactor `create_repo_agent` to be static method
- [ ] Implement parallel restoration in `scan_and_restore_channels()`
- [ ] Remove "Agent Ready" notification code
- [ ] Implement parallel shutdown notifications
- [ ] Consolidate result + metrics into single message
- [ ] Remove separate completion alert
- [ ] Test startup with multiple channels
- [ ] Test shutdown with Ctrl+C
- [ ] Test query completion message format
- [ ] Verify notification count reduced

## Configuration (Optional Future Enhancement)

```toml
[notifications]
# Send consolidated startup summary to admin channel (optional)
admin_channel = "C123456789"

# Startup notification enabled (default: false)
send_startup_notification = false

# Shutdown notification enabled (default: true)
send_shutdown_notification = true

# Include metrics in result message (default: true)
include_metrics_in_result = true
```

## Success Metrics

1. **Startup Performance**
   - ✅ 10x faster agent restoration
   - ✅ Zero user notifications
   - ✅ Clean structured logs

2. **User Experience**
   - ✅ 67% fewer messages per query
   - ✅ 1 notification per query (down from 2-3)
   - ✅ All info in one place

3. **Shutdown Performance**
   - ✅ 10x faster shutdown
   - ✅ Proper agent cleanup
   - ✅ All messages delivered before exit

## Conclusion

These optimizations will result in:
- **Much faster** startup and shutdown (10x improvement)
- **Much cleaner** Slack channels (67% fewer messages)
- **Better UX** (fewer notifications, consolidated info)
- **Same reliability** (parallel execution doesn't compromise delivery)

Estimated implementation time: **2-3 hours**
