# Spec 0010: Fix MainAgent Progress Display

**Status**: Implementation
**Created**: 2025-10-27
**Priority**: High

## Root Cause Analysis

### Problem
MainAgent doesn't show TodoWrite progress updates during repository setup, even though:
- ✅ TodoWrite hooks are configured correctly
- ✅ ProgressTracker is passed and working
- ✅ The hooks DO work for RepoAgent

### Root Cause

**Location**: `src/agent/main_agent.rs:86-93`

```rust
while let Some(message) = stream.next().await {
    let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

    if let claude_agent_sdk_rs::Message::Result(res) = message {
        final_result = res.result.unwrap_or_default();
        break;  // ❌ EXITS EARLY!
    }
}
```

**The Issue**:
1. Claude streams multiple message types: `StreamEvent`, `Assistant`, `Result`
2. TodoWrite hook fires when `StreamEvent` with tool_use is processed
3. MainAgent **breaks on first Result message**
4. This exits the loop before all StreamEvents are processed
5. Hooks may fire but stream ends prematurely

### How RepoAgent Works (Correctly)

**RepoAgent** doesn't consume the stream itself:
```rust
// In repo_agent.rs
pub fn receive_response(&mut self) -> impl Stream<...> {
    self.client.receive_response()  // Returns stream to caller
}
```

**MessageProcessor consumes it fully**:
```rust
// In messages.rs
while let Some(message) = stream.next().await {
    // Process ALL messages until stream ends naturally
    if let ClaudeMessage::Result(res) = message {
        final_result = res.result.clone();
        break;
    }
    // Continues processing other message types
}
```

The SDK's internal hook execution happens **as the stream is consumed**. When we break early, we don't give hooks time to complete.

## Solution

### Option 1: Consume Entire Stream (Recommended)

**Don't break** on Result - let the stream complete naturally:

```rust
while let Some(message) = stream.next().await {
    let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

    match message {
        claude_agent_sdk_rs::Message::Result(res) => {
            final_result = res.result.unwrap_or_default();
            // Don't break - continue processing remaining messages
        }
        claude_agent_sdk_rs::Message::StreamEvent(_) => {
            // StreamEvents trigger hooks internally
            // Continue processing
        }
        claude_agent_sdk_rs::Message::Assistant(_) => {
            // Assistant messages (text output)
            // Continue processing
        }
        _ => {
            // Other message types
        }
    }
    // Stream ends naturally when Claude finishes
}
```

### Option 2: Process All Messages Until Stream Ends

Be explicit about consuming everything:

```rust
let mut stream = self.client.receive_response();
let mut final_result = String::new();

// Consume ENTIRE stream to ensure hooks fire
while let Some(message) = stream.next().await {
    let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

    // Capture result but DON'T break
    if let claude_agent_sdk_rs::Message::Result(res) = message {
        final_result = res.result.unwrap_or_default();
        tracing::debug!("Received result message, continuing stream processing");
    }

    // Stream will end naturally when complete
}

tracing::info!("Stream complete, all hooks processed");
```

## Implementation

**File**: `src/agent/main_agent.rs:82-96`

### Change Required

```rust
// BEFORE (broken - exits early)
while let Some(message) = stream.next().await {
    let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

    if let claude_agent_sdk_rs::Message::Result(res) = message {
        final_result = res.result.unwrap_or_default();
        break;  // ❌ EXITS EARLY
    }
}

// AFTER (fixed - processes entire stream)
while let Some(message) = stream.next().await {
    let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

    // Capture result but continue processing stream
    if let claude_agent_sdk_rs::Message::Result(res) = message {
        final_result = res.result.unwrap_or_default();
        tracing::debug!("Received result, continuing to process remaining messages");
    }
    // Stream continues until it naturally ends
}

tracing::debug!("Stream processing complete");
```

## Why This Works

1. **Hooks fire during stream consumption**: The SDK processes tool_use events as they're streamed
2. **TodoWrite hook triggers**: When Claude calls TodoWrite, hook fires → updates plan → calls ProgressTracker
3. **ProgressTracker updates Slack**: Sends/updates the progress message in the channel
4. **Stream completes naturally**: All events processed, all hooks executed

## Testing Plan

1. **Start a repository setup**:
   ```
   @slack-coder tyrchen/some-repo
   ```

2. **Observe expected behavior**:
   - Initial message: "🔧 Setting up repository..."
   - Progress updates appear as Claude works through tasks
   - Tasks shown with checkboxes and progress bar
   - Final message: Setup complete

3. **Check logs**:
   ```
   TodoWrite hook triggered
   Parsed TodoWrite plan total_tasks=5 completed=0
   Progress updated in Slack
   ```

## Expected Outcome

**Before fix**:
```
🔧 Setting up repository tyrchen/slack-coder...
This may take a minute. I'll update you on progress.

[No progress updates shown]

[Eventually completes]
```

**After fix**:
```
🔧 Setting up repository tyrchen/slack-coder...
This may take a minute. I'll update you on progress.

⬜ Validating repository
⏳ Cloning repository  [=========>        ] 45%
⬜ Analyzing codebase
⬜ Generating system prompt
⬜ Saving configuration

[Updates in real-time as tasks progress]

✅ Validating repository
✅ Cloning repository
✅ Analyzing codebase
✅ Generating system prompt
✅ Saving configuration
```

## Conclusion

Simple fix: **Don't break early** - consume the entire stream to ensure all hooks execute.
