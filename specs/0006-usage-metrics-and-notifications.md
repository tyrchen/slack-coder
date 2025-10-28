# Spec 0006: Usage Metrics and Enhanced Notifications

**Status**: Draft
**Created**: 2025-10-27
**Related**: 0001-slack-bot-spec.md, 0005-slack-new-session-command.md

## Overview

This specification describes three new notification features for the Slack bot:
1. Usage metrics notification after final results
2. Task completion notification/alert
3. Graceful shutdown notification to all active channels

## Background

Currently, the Slack bot sends query results to users but doesn't provide visibility into:
- Token usage and API costs per query
- Clear notification when all tasks are complete
- Graceful shutdown messages when the bot terminates

The `ResultMessage` struct in the Claude Agent SDK already contains this information:
- `total_cost_usd: Option<f64>` - Total cost in USD
- `usage: Option<serde_json::Value>` - Usage statistics (tokens consumed)
- `duration_ms: u64` - Total duration
- `num_turns: u32` - Number of conversation turns
- `session_id: String` - Session identifier

## Requirements

### Feature 1: Usage Metrics Notification

**User Story**: As a user, I want to see the tokens consumed and cost after each query so I can track my API usage.

**Acceptance Criteria**:
- After the final result is sent to Slack, send a follow-up message with metrics
- Message should include:
  - Total tokens consumed (input + output)
  - Cost in USD (if available)
  - Query duration
  - Number of turns
  - Session ID (for tracking)
- Message should be formatted cleanly and consistently
- Should handle cases where cost/usage data is not available

**Implementation Notes**:
- Hook into `src/slack/messages.rs` where `ResultMessage` is processed (line 134)
- Extract metrics from `ResultMessage.usage` JSON value
- Format and send as a separate message immediately after result

### Feature 2: Task Completion Notification

**User Story**: As a user, I want to be notified when the bot has finished all tasks so I know when to check the results.

**Acceptance Criteria**:
- Send a notification when the query completes (after ResultMessage received)
- Notification should be visually distinct (use emoji/formatting)
- Should appear in the same thread as the query
- Should be sent AFTER both the result and metrics messages
- Notification timing should trigger Slack's alert system

**Implementation Notes**:
- This can be combined with the metrics message OR sent as a separate ping
- Consider using "@channel" or specific formatting to trigger notification
- Must be in the same thread to maintain context

### Feature 3: Graceful Shutdown Notification

**User Story**: As a user, I want to know when the bot shuts down so I understand why it's not responding.

**Acceptance Criteria**:
- When the bot application quits, send "Agent Gone" message to all active channels
- Message format should match the "Agent Ready" style:
  ```
  🔴 Agent Gone
  Session ID: `session-xxx-xxx` ended
  ```
- Should be sent to ALL channels that have active agents
- Should be best-effort (don't block shutdown if delivery fails)
- Should use the same channel and thread context where the agent was active

**Implementation Notes**:
- Add shutdown handler in `src/main.rs`
- Register signal handlers (SIGTERM, SIGINT, etc.)
- Iterate through `AgentManager` to find all active agents and their channels
- Send notification to each channel
- Timeout the shutdown process if it takes too long (e.g., 5 seconds)

## Architecture

### Component Interactions

```
┌─────────────────────────────────────────────────────────────┐
│                    EventHandler                              │
│                 (slack/events.rs)                            │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│               MessageProcessor                               │
│              (slack/messages.rs)                             │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ process_message()                                      │ │
│  │   ├─ Forward to agent                                 │ │
│  │   ├─ Send result message                              │ │
│  │   ├─ [NEW] Send metrics message                       │ │ NEW
│  │   └─ [NEW] Send completion notification               │ │ NEW
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                   SlackClient                                │
│                (slack/client.rs)                             │
│                                                              │
│  • send_message() - Text messages                           │
│  • [NEW] send_metrics() - Formatted metrics                 │ NEW
│  • [NEW] send_completion_alert() - Notification             │ NEW
│  • [NEW] send_shutdown_notice() - Agent gone message        │ NEW
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                      main.rs                                 │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ [NEW] Shutdown Handler                                 │ │ NEW
│  │   ├─ Register signal handlers (SIGTERM, SIGINT)       │ │
│  │   ├─ Get all active agents from AgentManager          │ │
│  │   ├─ Send shutdown notice to each channel             │ │
│  │   └─ Cleanup and exit                                 │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Data Flow

#### Metrics Notification Flow
```
RepoAgent.query() returns ResultMessage
    ↓
MessageProcessor extracts ResultMessage
    ↓
Send main result to Slack
    ↓
Extract metrics from ResultMessage:
  - usage.input_tokens
  - usage.output_tokens
  - total_cost_usd
  - duration_ms
  - num_turns
    ↓
Format metrics message
    ↓
SlackClient.send_metrics(channel, thread_ts, metrics)
    ↓
Send completion notification
    ↓
User sees: Result + Metrics + "Task Complete" in thread
```

#### Shutdown Notification Flow
```
OS sends SIGTERM/SIGINT
    ↓
Signal handler triggered in main.rs
    ↓
AgentManager.get_all_active_agents()
    ↓
For each agent:
  - Get channel_id
  - Get session_id
  - Get last thread_ts (if any)
    ↓
Format shutdown message per channel
    ↓
SlackClient.send_shutdown_notice(channel, session_id)
    ↓
Cleanup and exit (with timeout)
```

## Implementation Details

### 1. Usage Metrics Message Format

```rust
pub struct UsageMetrics {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: Option<f64>,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    pub num_turns: u32,
    pub session_id: String,
}

impl UsageMetrics {
    pub fn from_result_message(result: &ResultMessage) -> Self {
        // Parse usage JSON to extract token counts
        // Calculate total_tokens
        // Extract cost_usd
        // Return structured metrics
    }

    pub fn format_slack_message(&self) -> String {
        format!(
            "📊 *Query Metrics*\n\
             • Tokens: {} input + {} output = {} total\n\
             • Cost: ${:.4} USD\n\
             • Duration: {:.2}s ({} turns)\n\
             • Session: `{}`",
            self.input_tokens,
            self.output_tokens,
            self.total_tokens,
            self.cost_usd.unwrap_or(0.0),
            self.duration_ms as f64 / 1000.0,
            self.num_turns,
            self.session_id
        )
    }
}
```

### 2. ResultMessage.usage Structure

Based on the Claude API, the `usage` field is a JSON object like:
```json
{
  "input_tokens": 2095,
  "cache_creation_input_tokens": 0,
  "cache_read_input_tokens": 0,
  "output_tokens": 503
}
```

We need to deserialize this to extract token counts.

### 3. New Methods in SlackClient

```rust
impl SlackClient {
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
        let text = format!(
            "🔴 *Agent Gone*\n\nSession ID: `{}` ended",
            session_id
        );
        self.send_message(channel, &text, None).await
    }
}
```

### 4. Modified MessageProcessor Logic

In `src/slack/messages.rs`, modify the `process_message()` function:

```rust
// After sending result message
if let ClaudeMessage::Result(result) = message {
    // [EXISTING] Send the main result
    self.slack_client
        .send_message(&channel_id, &formatted_result, Some(&thread_ts))
        .await?;

    // [NEW] Send metrics
    let metrics = UsageMetrics::from_result_message(&result);
    self.slack_client
        .send_metrics(&channel_id, Some(&thread_ts), &metrics)
        .await?;

    // [NEW] Send completion notification
    self.slack_client
        .send_completion_alert(&channel_id, Some(&thread_ts))
        .await?;
}
```

### 5. Shutdown Handler in main.rs

**IMPROVED VERSION** - Handles multiple signals and ensures complete delivery:

```rust
use tokio::signal;
use tokio::sync::Notify;

async fn main() -> Result<()> {
    // ... existing setup code ...

    // Setup shutdown signal handler
    let shutdown_notify = Arc::new(Notify::new());
    let shutdown_signal = setup_shutdown_handler(shutdown_notify.clone());

    // Run application with shutdown handling
    tokio::select! {
        result = event_handler.start() => {
            tracing::info!("Event handler completed normally");
            result?;
        }
        signal_name = shutdown_signal => {
            tracing::info!("🛑 Received {} signal, initiating graceful shutdown...", signal_name);

            // Send shutdown notifications and wait for completion
            send_shutdown_notifications(&agent_manager, &slack_client).await;

            tracing::info!("👋 Graceful shutdown complete");
        }
    }

    Ok(())
}

/// Setup signal handlers for graceful shutdown
/// Handles SIGINT (Ctrl+C), SIGTERM, and SIGQUIT on Unix systems
async fn setup_shutdown_handler(_shutdown_notify: Arc<Notify>) -> String {
    #[cfg(unix)]
    {
        use signal::unix::{signal, SignalKind};

        let mut sigint = signal(SignalKind::interrupt())
            .expect("Failed to setup SIGINT handler");
        let mut sigterm = signal(SignalKind::terminate())
            .expect("Failed to setup SIGTERM handler");
        let mut sigquit = signal(SignalKind::quit())
            .expect("Failed to setup SIGQUIT handler");

        tokio::select! {
            _ = sigint.recv() => {
                tracing::debug!("Caught SIGINT signal");
                "SIGINT (Ctrl+C)".to_string()
            }
            _ = sigterm.recv() => {
                tracing::debug!("Caught SIGTERM signal");
                "SIGTERM".to_string()
            }
            _ = sigquit.recv() => {
                tracing::debug!("Caught SIGQUIT signal");
                "SIGQUIT".to_string()
            }
        }
    }

    #[cfg(not(unix))]
    {
        // On Windows, only handle Ctrl+C
        signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        tracing::debug!("Caught Ctrl+C signal");
        "Ctrl+C".to_string()
    }
}

/// Send shutdown notifications to all active channels
/// This function will block until all notifications are sent or timeout (3s per message)
async fn send_shutdown_notifications(
    agent_manager: &Arc<AgentManager>,
    slack_client: &Arc<SlackClient>,
) {
    info!("Sending shutdown notifications to all channels");

    // Get all active agents
    let agents = agent_manager.get_all_active_agents().await;

    if agents.is_empty() {
        info!("No active agents to notify");
        return;
    }

    // Send notifications sequentially to ensure delivery
    let mut success_count = 0;
    let mut failure_count = 0;
    let total_agents = agents.len();

    for (channel_id, session_id) in agents {
        // Try to send with 3-second timeout per message
        match tokio::time::timeout(
            Duration::from_secs(3),
            slack_client.send_shutdown_notice(&channel_id, &session_id),
        )
        .await
        {
            Ok(Ok(_)) => {
                success_count += 1;
                info!("✅ Shutdown notice sent to {}", channel_id);
            }
            Ok(Err(e)) => {
                failure_count += 1;
                warn!("❌ Failed to send shutdown notice to {}: {}", channel_id, e);
            }
            Err(_) => {
                failure_count += 1;
                warn!("⏱️  Timeout sending shutdown notice to {}", channel_id);
            }
        }
    }

    info!("📊 Shutdown notifications: {} succeeded, {} failed out of {} total",
        success_count, failure_count, total_agents);
}
```

**Key Improvements:**
1. **Multiple Signal Support**: Handles SIGINT, SIGTERM, and SIGQUIT on Unix
2. **Sequential Delivery**: Sends messages one at a time to ensure delivery
3. **Per-Message Timeout**: 3 seconds per message (not total timeout)
4. **Detailed Logging**: Shows progress and summary of deliveries
5. **Cross-Platform**: Works on both Unix and Windows
6. **Blocking Completion**: Main process waits for all notifications before exit

### 6. New Method in AgentManager

```rust
impl AgentManager {
    /// Get all active agents and their session IDs
    pub async fn get_all_active_agents(&self) -> Vec<(ChannelId, String)> {
        let mut result = Vec::new();

        for entry in self.agents.iter() {
            let channel_id = entry.key().clone();

            if let Ok(agent) = entry.value().try_lock() {
                if agent.is_connected() {
                    let session_id = agent.get_session_id();
                    result.push((channel_id, session_id));
                }
            }
        }

        result
    }
}
```

## Testing Plan

### Unit Tests
1. `UsageMetrics::from_result_message()` - Test JSON parsing
2. `UsageMetrics::format_slack_message()` - Test message formatting
3. `AgentManager::get_all_active_agents()` - Test agent enumeration

### Integration Tests
1. Send a query and verify metrics message appears after result
2. Verify completion notification appears after metrics
3. Send SIGTERM and verify shutdown messages sent to all channels
4. Test shutdown timeout (simulate slow Slack API)

### Manual Testing
1. Deploy to test workspace
2. Send query and observe three messages:
   - Result
   - Metrics
   - Completion alert
3. Kill the process and verify "Agent Gone" messages
4. Verify session IDs match across messages

## Error Handling

### Metrics Extraction Failures
- If `usage` field is missing: Show "Usage data not available"
- If `total_cost_usd` is None: Show "Cost: N/A"
- Log warnings for parsing failures

### Shutdown Notification Failures
- Use best-effort delivery (don't block shutdown)
- Log warnings for failed deliveries
- Respect 5-second timeout to prevent hanging

### Thread Context
- Metrics and completion messages MUST be in same thread as result
- Shutdown messages can be in main channel (no thread)

## Migration Plan

### Phase 1: Metrics and Completion Notifications
1. Implement `UsageMetrics` struct
2. Add methods to `SlackClient`
3. Modify `MessageProcessor` to send additional messages
4. Test in development environment
5. Deploy to production

### Phase 2: Shutdown Notifications
1. Implement `get_all_active_agents()` in `AgentManager`
2. Add shutdown handler in `main.rs`
3. Test graceful shutdown scenarios
4. Deploy to production

### Rollback Plan
- If issues arise, remove the additional message sends
- Original functionality (sending results) remains unchanged
- No database migrations required

## Open Questions

1. **Should metrics be a reply or separate message?**
   - Decision: Separate message in same thread for clarity

2. **Should we notify on partial results or only final?**
   - Decision: Only on ResultMessage (final result)

3. **What if shutdown takes longer than 5 seconds?**
   - Decision: Timeout and exit anyway, log warnings

4. **Should shutdown message include last thread context?**
   - Decision: No, send to main channel without thread to ensure visibility

## Success Metrics

- Users can see token usage after every query
- Users receive clear completion notifications
- All channels receive shutdown notice when bot terminates
- No performance degradation in message processing
- Error rate remains below 1% for metrics delivery

## Future Enhancements

1. Aggregate metrics per session (total tokens/cost for session)
2. Daily/weekly usage reports
3. Budget alerts when cost exceeds threshold
4. Startup notifications when bot recovers from crash
5. Health check endpoint for monitoring

## References

- Claude Agent SDK: `/vendors/claude-agent-sdk-rs/src/types/messages.rs`
- Current message flow: `src/slack/messages.rs:134`
- Agent ready message: `src/agent/manager.rs:137-149`
- Slack client: `src/slack/client.rs`
