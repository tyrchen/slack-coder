# Spec 0007: Improved Structured Logging and Tracing

**Status**: Implementation
**Created**: 2025-10-27
**Priority**: High
**Related**: All modules (slack, agent, main)

## Overview

This specification defines improvements to the logging and tracing system across the Slack bot application. The current logging approach has several issues including verbose debug dumps, inconsistent formatting, lack of structured fields, and poor integration with log aggregation systems.

## Problem Statement

### Current Issues

1. **Verbose Debug Dumps**: Full `{:?}` dumps of Slack events can produce 500+ line outputs
2. **Inconsistent Formatting**: Mix of emoji-based, string-based, and structured logging
3. **Non-Queryable Fields**: Important metrics buried in formatted strings
4. **Missing Context**: No timing information, request IDs, or operation spans
5. **Poor Machine Parsing**: Hard to integrate with log aggregation tools (Datadog, ELK, etc.)
6. **Duplicate Logging**: Same information logged multiple times at different levels

### Example of Current Problem

```rust
// Current (bad): Massive debug dump
tracing::info!("📨 Received push event: {:?}", event.event);
// Output: 500+ lines of nested Slack event JSON

// Current (bad): Non-queryable metrics
tracing::info!(
    "📤 Sending shutdown notice {} session={} ({}/{})",
    channel_id.log_format(),
    session_id,
    success_count + failure_count + 1,
    total_agents
);
```

## Goals

1. **Reduce Log Verbosity**: 50-70% reduction in debug spam
2. **Improve Debuggability**: Better context through spans and structured fields
3. **Enable Metrics**: Machine-parseable fields for dashboards and alerting
4. **Consistent Style**: Uniform approach across all modules
5. **Better Performance**: Less string formatting, more efficient field capture

## Design Principles

### 1. Structured Field Logging

Use `tracing`'s structured field syntax for all important data:

```rust
// ✅ GOOD: Structured fields
tracing::info!(
    channel = %channel.as_str(),
    user = %user.as_str(),
    message_len = text.len(),
    "Processing message"
);

// ❌ BAD: Formatted string
tracing::info!(
    "Processing message channel={} user={} len={}",
    channel,
    user,
    text.len()
);
```

### 2. Use Spans for Multi-Step Operations

Create spans for operations that span multiple function calls:

```rust
// ✅ GOOD: Operation span
let span = tracing::info_span!(
    "process_query",
    channel = %channel.as_str(),
    user = %user.as_str()
);
let _guard = span.enter();

// All logs within this scope inherit channel and user
tracing::debug!("acquiring agent lock");
tracing::debug!("sending query to Claude");
tracing::info!("query completed");
```

### 3. Selective Debug Dumps

Never log full structs with `{:?}`. Extract only necessary fields:

```rust
// ❌ BAD: Full debug dump
tracing::debug!("Full event: {:?}", event);

// ✅ GOOD: Selective field extraction
tracing::debug!(
    event_type = %event.event_type,
    channel = ?event.channel,
    user = ?event.user,
    ts = %event.ts,
    "Received Slack event"
);
```

### 4. Log Levels

- **ERROR**: Unrecoverable errors that require human intervention
- **WARN**: Recoverable errors, timeouts, expected failures
- **INFO**: Key state changes, major operations, user-facing events
- **DEBUG**: Detailed flow information, helpful for troubleshooting
- **TRACE**: Very verbose, typically not enabled in production

### 5. Field Naming Conventions

Use consistent field names across the codebase:

| Data | Field Name | Format |
|------|-----------|--------|
| Channel ID | `channel` | `%channel.as_str()` |
| User ID | `user` | `%user.as_str()` |
| Session ID | `session_id` | `%session_id` |
| Message timestamp | `ts` | `%ts` |
| Thread timestamp | `thread_ts` | `?thread_ts` |
| Duration | `duration_ms` | milliseconds (u64) |
| Count/Size | `count`, `size`, `len` | integer |
| Error | `error`, `error_kind` | `%e`, type name |

## Implementation Plan

### Phase 1: Core Infrastructure (Priority: HIGH)

#### 1.1 Helper Functions Module

Create `src/logging.rs` with helper functions:

```rust
//! Logging utilities for structured tracing

use std::time::Instant;
use tracing::Span;

/// Create a span for message processing
pub fn message_span(channel: &str, user: &str) -> Span {
    tracing::info_span!("process_message", channel = %channel, user = %user)
}

/// Create a span for agent operations
pub fn agent_span(channel: &str, operation: &str) -> Span {
    tracing::info_span!("agent_operation", channel = %channel, operation = %operation)
}

/// Log an error with context
pub fn log_error(operation: &str, error: &impl std::error::Error) {
    tracing::error!(
        operation = %operation,
        error = %error,
        error_kind = std::any::type_name_of_val(error),
        "Operation failed"
    );
}

/// Track operation timing
pub struct Timer {
    start: Instant,
    operation: String,
}

impl Timer {
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            operation: operation.into(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let duration_ms = self.start.elapsed().as_millis() as u64;
        tracing::info!(
            operation = %self.operation,
            duration_ms = duration_ms,
            "Operation completed"
        );
    }
}
```

#### 1.2 Event Helpers for Slack Types

Add methods to Slack types for clean field extraction:

```rust
// In src/slack/types.rs

impl SlackMessage {
    /// Extract loggable fields (no debug dump)
    pub fn log_fields(&self) -> LogFields {
        LogFields {
            channel: self.channel.as_str(),
            user: self.user.as_str(),
            text_len: self.text.len(),
            has_thread: self.thread_ts.is_some(),
        }
    }
}

struct LogFields<'a> {
    channel: &'a str,
    user: &'a str,
    text_len: usize,
    has_thread: bool,
}
```

### Phase 2: Refactor High-Priority Files

#### 2.1 src/slack/events.rs (CRITICAL)

**Current Problems:**
- Line 111: Full event debug dump
- Line 167: Full mention event dump
- Line 240: Full message dump
- Redundant logging at multiple levels

**Changes:**

```rust
// BEFORE (Line 111)
tracing::info!("📨 Received push event: {:?}", event.event);

// AFTER
tracing::debug!(
    event_type = ?event.event_type,
    "Received push event"
);

// BEFORE (Line 167)
tracing::debug!("Full mention event: {:?}", mention);

// AFTER
tracing::debug!(
    channel = %mention.channel.as_str(),
    user = %mention.user.as_str(),
    text_len = mention.content.text.as_ref().map(|t| t.len()).unwrap_or(0),
    has_blocks = mention.content.blocks.is_some(),
    "App mention event"
);

// BEFORE (Line 240)
tracing::debug!("Full message: {:?}", message);

// AFTER
tracing::debug!(
    channel = %message.channel.as_str(),
    user = %message.user.as_str(),
    text_len = message.text.len(),
    has_thread = message.thread_ts.is_some(),
    "Message event"
);
```

**Additional Improvements:**

1. Add span for event processing:
```rust
let span = tracing::info_span!(
    "handle_event",
    event_id = %event_envelope.event_id,
    event_type = ?event.event_type
);
let _guard = span.enter();
```

2. Remove redundant logs:
```rust
// DELETE: Already logged by span
// tracing::info!("Processing {} event", event_type);
```

#### 2.2 src/slack/messages.rs (HIGH PRIORITY)

**Current Problems:**
- No timing information for query->response
- Character counts not structured
- Lock acquisition logging verbose
- Missing span context

**Changes:**

```rust
// Add timing at start
pub async fn process_message(&self, message: SlackMessage) -> Result<()> {
    let _timer = Timer::new("process_message");
    let span = tracing::info_span!(
        "process_message",
        channel = %message.channel.as_str(),
        user = %message.user.as_str(),
        text_len = message.text.len(),
    );
    let _guard = span.enter();

    // Remove emoji, simplify
    // BEFORE
    tracing::info!(
        "💬 Processing message {} user={}",
        message.channel.log_format(),
        message.user.as_str()
    );

    // AFTER: Just use span context
    tracing::info!("Processing message");

    // ... rest of function
}

// Lock acquisition
// BEFORE
tracing::debug!("Getting agent {}...", channel.log_format());
let agent_mutex = self.agent_manager.get_repo_agent(channel).await?;
tracing::debug!("  Got agent, attempting to acquire lock with timeout...");

// AFTER
tracing::debug!("Acquiring agent lock");
let agent_mutex = self.agent_manager.get_repo_agent(channel).await?;

match timeout(Duration::from_secs(3), agent_mutex.lock()).await {
    Ok(agent) => {
        tracing::info!("Agent lock acquired");
        agent
    }
    Err(_) => {
        tracing::warn!(
            timeout_secs = 3,
            "Agent lock timeout"
        );
        // ... send user message
    }
}

// Response sending
// BEFORE
tracing::info!(
    "📤 Sending response {} ({} chars)...",
    channel.log_format(),
    final_result.len()
);

// AFTER
tracing::info!(
    response_len = final_result.len(),
    is_chunked = slack_formatted.len() > MAX_SLACK_MESSAGE_SIZE,
    "Sending response"
);
```

#### 2.3 src/agent/hooks.rs (MEDIUM PRIORITY)

**Current Problems:**
- Raw Plan JSON dumped (can be very large)
- Redundant logging
- Tool input logged multiple times

**Changes:**

```rust
// BEFORE (Line 34, 63)
tracing::debug!("  Tool input: {}", post_tool.tool_input);
tracing::debug!("  Raw input: {}", post_tool.tool_input);

// AFTER: Log only summary
tracing::debug!(
    tool_name = %post_tool.tool_name,
    input_len = post_tool.tool_input.len(),
    "Tool invocation"
);

// When parsing Plan
// BEFORE
let new_plan: Plan = serde_json::from_str(&post_tool.tool_input)?;

// AFTER
let new_plan: Plan = serde_json::from_str(&post_tool.tool_input)?;
tracing::debug!(
    total_tasks = new_plan.todos.len(),
    completed = new_plan.todos.iter().filter(|t| t.status == TaskStatus::Completed).count(),
    in_progress = new_plan.todos.iter().filter(|t| t.status == TaskStatus::InProgress).count(),
    "Parsed TodoWrite plan"
);
```

#### 2.4 src/agent/manager.rs (MEDIUM PRIORITY)

**Current Problems:**
- Verbose setup logging
- Some info logs should be debug
- No span for setup operation

**Changes:**

```rust
// Setup operation span
pub async fn setup_channel(&self, channel_id: ChannelId, repo_name: String) -> Result<()> {
    let span = tracing::info_span!(
        "setup_channel",
        channel = %channel_id.as_str(),
        repo = %repo_name
    );
    let _guard = span.enter();

    tracing::info!("Starting channel setup");

    // BEFORE: Too verbose
    tracing::info!(
        "🎬 Setting up {} repo={}",
        channel_id.log_format(),
        repo_name
    );
    tracing::debug!("Creating main agent...");
    tracing::info!("✅ Main agent created");

    // AFTER: Simplified
    let mut main_agent = MainAgent::new(...).await?;
    tracing::debug!("Main agent created");

    main_agent.connect().await?;
    tracing::debug!("Connected to Claude");

    main_agent.setup_repository(&repo_name, &channel_id).await?;
    tracing::info!("Repository setup completed");

    // ... rest
}

// Channel scanning
pub async fn scan_and_restore_channels(&self, slack_client: &SlackClient) -> Result<()> {
    let span = tracing::info_span!("scan_channels");
    let _guard = span.enter();

    let channels = slack_client.list_channels().await?;
    tracing::info!(
        total_channels = channels.len(),
        "Scanning for existing setups"
    );

    let mut restored_count = 0;
    for channel_id in channels {
        // BEFORE: Log every channel
        tracing::debug!("Checking {}", channel_id.log_format());

        // AFTER: Only log if setup exists
        if self.workspace.is_channel_setup(&channel_id).await {
            tracing::info!(
                channel = %channel_id.as_str(),
                "Restoring agent"
            );
            // ... restore
            restored_count += 1;
        }
    }

    tracing::info!(
        restored = restored_count,
        total_scanned = channels.len(),
        "Channel scan complete"
    );
}
```

#### 2.5 src/main.rs (LOW PRIORITY)

**Current Problems:**
- Summary stats as strings
- Missing structured fields

**Changes:**

```rust
// BEFORE
tracing::info!(
    "📊 Shutdown notification summary: {} succeeded, {} failed out of {} total",
    success_count,
    failure_count,
    total_agents
);

// AFTER
tracing::info!(
    succeeded = success_count,
    failed = failure_count,
    total = total_agents,
    success_rate = (success_count as f32 / total_agents as f32 * 100.0) as u32,
    "Shutdown notifications complete"
);
```

### Phase 3: Additional Improvements

#### 3.1 Remove Emojis from Logs

Emojis are useful for human readability during development, but they:
- Don't add value in log aggregation systems
- Can cause encoding issues in some terminals
- Make logs less professional

**Strategy**: Remove ALL emojis, rely on structured fields and log levels instead.

```rust
// BEFORE
tracing::info!("✅ Response sent {}", channel.log_format());
tracing::warn!("⏳ Agent busy");
tracing::error!("❌ Command failed");

// AFTER
tracing::info!(channel = %channel.as_str(), "Response sent");
tracing::warn!("Agent busy");
tracing::error!("Command failed");
```

The severity is already indicated by the log level (INFO/WARN/ERROR).

#### 3.2 Consistent `.log_format()` Usage

The custom `.log_format()` method adds brackets around IDs:
```rust
channel.log_format() // => "[C09NRMS2A58]"
```

**Strategy**: Remove `.log_format()`, use `.as_str()` directly in structured fields.

```rust
// BEFORE
tracing::info!("Processing {}", channel.log_format());

// AFTER
tracing::info!(channel = %channel.as_str(), "Processing message");
```

Brackets can be added by the log formatter if needed, not in the application code.

#### 3.3 Add Request Tracing

Add a request ID that follows a message through the entire lifecycle:

```rust
// In src/slack/types.rs
#[derive(Debug, Clone)]
pub struct RequestId(String);

impl RequestId {
    pub fn new() -> Self {
        use uuid::Uuid;
        Self(Uuid::new_v4().to_string())
    }
}

// In message processing
pub async fn process_message(&self, message: SlackMessage) -> Result<()> {
    let request_id = RequestId::new();
    let span = tracing::info_span!(
        "process_message",
        request_id = %request_id.0,
        channel = %message.channel.as_str(),
        user = %message.user.as_str(),
    );
    // ... all logs in this span will include request_id
}
```

## Testing Strategy

### 1. Visual Inspection

Run the bot with different log levels and verify output:

```bash
# Test INFO level (default)
RUST_LOG=slack_coder=info cargo run

# Test DEBUG level
RUST_LOG=slack_coder=debug cargo run

# Test structured JSON output
RUST_LOG=slack_coder=debug RUST_LOG_FORMAT=json cargo run
```

### 2. Log Volume Measurement

Before/after comparison:

```bash
# Before refactoring
RUST_LOG=slack_coder=debug cargo run > before.log 2>&1
# Send test message, wait for response, Ctrl+C
wc -l before.log

# After refactoring
RUST_LOG=slack_coder=debug cargo run > after.log 2>&1
# Send same test message, wait for response, Ctrl+C
wc -l after.log

# Calculate reduction
echo "Reduction: $(echo "scale=2; (1 - $(wc -l < after.log) / $(wc -l < before.log)) * 100" | bc)%"
```

### 3. Structured Field Verification

Verify structured fields are properly captured:

```bash
# With JSON formatter
RUST_LOG=slack_coder=info cargo run 2>&1 | jq 'select(.channel != null)'
```

Should output valid JSON with structured fields.

### 4. Span Context Verification

Verify spans properly propagate context:

```bash
# Look for nested logs with inherited fields
RUST_LOG=slack_coder=debug cargo run 2>&1 | grep "process_message"
```

All logs within a span should inherit the span's fields.

## Success Metrics

1. **Log Volume Reduction**: 50-70% fewer lines in debug mode
2. **Structured Fields**: 100% of key metrics as queryable fields
3. **Zero Debug Dumps**: No `{:?}` on complex Slack types
4. **Consistent Style**: All modules use same structured approach
5. **Performance**: No measurable performance regression

## Migration Checklist

- [ ] Create `src/logging.rs` with helper functions
- [ ] Add `Timer` struct for operation timing
- [ ] Refactor `src/slack/events.rs`
  - [ ] Remove full event debug dumps
  - [ ] Add event processing span
  - [ ] Use selective field logging
- [ ] Refactor `src/slack/messages.rs`
  - [ ] Add message processing span
  - [ ] Add timing information
  - [ ] Structured response logging
- [ ] Refactor `src/agent/hooks.rs`
  - [ ] Remove raw Plan JSON dumps
  - [ ] Log Plan summary only
  - [ ] Remove duplicate logs
- [ ] Refactor `src/agent/manager.rs`
  - [ ] Add setup span
  - [ ] Move verbose logs to debug
  - [ ] Structured restore stats
- [ ] Refactor `src/main.rs`
  - [ ] Structured shutdown stats
- [ ] Remove all emojis from logs
- [ ] Remove all `.log_format()` calls
- [ ] Add request ID tracing
- [ ] Test with INFO level
- [ ] Test with DEBUG level
- [ ] Measure log volume reduction
- [ ] Verify structured field output

## Examples of Final Output

### Before (Current)
```
2025-10-27T14:49:19.953146Z  INFO slack_coder::slack::events: 111: 📨 Received push event: Message(SlackMessageEvent { origin: SlackMessageOrigin { ts: SlackTs("1761576558.852309"), channel: Some(SlackChannelId("C09NRMS2A58")), channel_type: Some(SlackChannelType("channel")), thread_ts: None, client_msg_id: None }, content: Some(SlackMessageContent { text: Some(":robot_face: *Agent Ready*\n\n... [500+ more lines] })
```

### After (Improved)
```
2025-10-27T14:49:19.953Z INFO process_message{channel="C09NRMS2A58" user="U09NPJCJXU6" text_len=85}: slack_coder::slack::messages: Processing message
2025-10-27T14:49:19.954Z DEBUG process_message{channel="C09NRMS2A58" user="U09NPJCJXU6" text_len=85}: slack_coder::slack::messages: Acquiring agent lock
2025-10-27T14:49:19.955Z INFO process_message{channel="C09NRMS2A58" user="U09NPJCJXU6" text_len=85}: slack_coder::slack::messages: Agent lock acquired
2025-10-27T14:49:22.103Z INFO process_message{channel="C09NRMS2A58" user="U09NPJCJXU6" text_len=85}: slack_coder::slack::messages: response_len=1423 is_chunked=false Sending response
2025-10-27T14:49:22.205Z INFO slack_coder::slack::messages: operation="process_message" duration_ms=2252 Operation completed
```

### With JSON Output
```json
{
  "timestamp": "2025-10-27T14:49:19.953Z",
  "level": "INFO",
  "target": "slack_coder::slack::messages",
  "span": {
    "name": "process_message",
    "channel": "C09NRMS2A58",
    "user": "U09NPJCJXU6",
    "text_len": 85
  },
  "fields": {
    "message": "Processing message"
  }
}
```

## Conclusion

This refactoring will significantly improve the debugging experience, reduce log noise, and enable proper observability for production deployments. The structured approach makes it easy to integrate with modern log aggregation and APM tools.

Total estimated implementation time: **2-3 hours**
