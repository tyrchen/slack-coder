# Feature Specification: Slack Session Management & Commands

## Overview

Implement Slack slash commands and session management to allow users to:
1. Start fresh conversations without context carry-over
2. Get help on available commands
3. Have clear visual indicators when sessions start

## Requirements

### Functional Requirements

**REQ-1**: Support `/help` command
- Display available commands
- Show usage examples
- Respond in the same channel

**REQ-2**: Support `/new-session` command
- Clear conversation context
- Generate new session ID
- Notify user that new session started
- All subsequent messages use the new session

**REQ-3**: Auto-generate session on agent startup
- Each repo agent starts with a unique session ID
- Notify user when agent first loads
- Session persists until `/new-session` or agent restart

**REQ-4**: Session persistence per channel
- Each Slack channel has its own session
- Sessions are independent across channels
- Session ID stored in RepoAgent instance

### Non-Functional Requirements

**Performance**:
- Command responses < 100ms
- Session switching doesn't interrupt ongoing work
- No message loss during session transitions

**User Experience**:
- Clear feedback when session changes
- Visual distinction between session start and regular messages
- Consistent command formatting

**Reliability**:
- Session IDs are unique and collision-free
- Failed session creation falls back gracefully
- Existing sessions unaffected by new session creation

## Architecture

### Session ID Format

```
session-{channel_id}-{timestamp}-{random}
```

Example: `session-C09NNKZ8SPP-1761520471-a3f9b2`

**Benefits**:
- Unique across channels
- Sortable by time
- Debuggable (can see when created)
- No collisions (random suffix)

### Component Changes

#### 1. RepoAgent Structure

**Current**:
```rust
pub struct RepoAgent {
    client: ClaudeClient,
    plan: Arc<Mutex<Plan>>,
    channel_id: ChannelId,
    last_activity: Arc<RwLock<Instant>>,
}
```

**Proposed**:
```rust
pub struct RepoAgent {
    client: ClaudeClient,
    plan: Arc<Mutex<Plan>>,
    channel_id: ChannelId,
    current_session_id: Arc<RwLock<String>>,  // NEW
    last_activity: Arc<RwLock<Instant>>,
}
```

#### 2. Message Processing Flow

**Current Flow**:
```
User Message → MessageProcessor → RepoAgent.query(text) → Claude
```

**New Flow**:
```
User Message → Check if command → {
  If command: Handle command (help, new-session)
  Else: RepoAgent.query_with_session(text, session_id) → Claude
}
```

#### 3. Command Handler

**New Component**: `SlackCommandHandler`

```rust
pub struct SlackCommandHandler {
    slack_client: Arc<SlackClient>,
}

impl SlackCommandHandler {
    pub async fn handle_command(
        &self,
        command: &str,
        channel: &ChannelId,
        agent_manager: &AgentManager,
    ) -> Result<CommandResult> {
        match command {
            "/help" => self.handle_help(channel).await,
            "/new-session" => self.handle_new_session(channel, agent_manager).await,
            _ => Err(SlackCoderError::UnknownCommand(command.to_string())),
        }
    }
}
```

### Session Lifecycle

#### Startup (Agent Creation)

```
1. RepoAgent::new() called
2. Generate session_id = generate_session_id(channel_id)
3. Store in RepoAgent.current_session_id
4. Connect to Claude
5. Send notification to Slack:
   "🚀 New session started: session-C09NNKZ8SPP-..."
```

#### Normal Operation

```
1. User sends message
2. Extract session_id from RepoAgent
3. Call client.query_with_session(message, session_id)
4. Stream response as usual
```

#### Session Reset (/new-session)

```
1. User sends "/new-session"
2. CommandHandler.handle_new_session():
   a. Get RepoAgent for channel
   b. Generate new session_id
   c. Update RepoAgent.current_session_id
   d. Send notification:
      "🔄 New session started: session-C09NNKZ8SPP-..."
      "Previous context cleared."
3. Next user message uses new session_id
```

## Implementation Plan

### Phase 1: Session ID Management

**Files to modify:**
- `src/agent/repo_agent.rs`: Add `current_session_id` field
- `src/agent/types.rs` or new `src/session.rs`: Add session ID generation

**Implementation:**

```rust
// src/session.rs (NEW)
use crate::slack::ChannelId;
use uuid::Uuid;

pub fn generate_session_id(channel_id: &ChannelId) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let random = Uuid::new_v4().to_string()[..6].to_string();

    format!("session-{}-{}-{}", channel_id.as_str(), timestamp, random)
}
```

```rust
// Update RepoAgent::new()
pub async fn new(...) -> Result<Self> {
    // ... existing code ...

    let session_id = generate_session_id(&channel_id);

    Ok(Self {
        client,
        plan,
        channel_id: channel_id.clone(),
        current_session_id: Arc::new(RwLock::new(session_id)),
        last_activity: Arc::new(RwLock::new(Instant::now())),
    })
}
```

### Phase 2: Update query() to use sessions

**Files to modify:**
- `src/agent/repo_agent.rs`: Change `query()` to use `query_with_session()`

**Implementation:**

```rust
// In RepoAgent
pub async fn query(&mut self, message: &str) -> Result<()> {
    let session_id = self.current_session_id.read().unwrap().clone();

    self.client
        .query_with_session(message, session_id)
        .await
        .map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

    self.update_activity();
    Ok(())
}

pub async fn start_new_session(&mut self, channel: &ChannelId) -> Result<String> {
    let new_session_id = generate_session_id(channel);
    *self.current_session_id.write().unwrap() = new_session_id.clone();
    Ok(new_session_id)
}

pub fn get_session_id(&self) -> String {
    self.current_session_id.read().unwrap().clone()
}
```

### Phase 3: Command Detection & Handling

**Files to create:**
- `src/slack/commands.rs`: New command handler

**Files to modify:**
- `src/slack/messages.rs`: Add command detection
- `src/slack/mod.rs`: Export command handler

**Implementation:**

```rust
// src/slack/commands.rs (NEW)
use crate::agent::AgentManager;
use crate::error::Result;
use crate::slack::{ChannelId, SlackClient};
use std::sync::Arc;

pub struct SlackCommandHandler {
    slack_client: Arc<SlackClient>,
}

impl SlackCommandHandler {
    pub fn new(slack_client: Arc<SlackClient>) -> Self {
        Self { slack_client }
    }

    pub async fn handle_command(
        &self,
        command: &str,
        channel: &ChannelId,
        agent_manager: &AgentManager,
    ) -> Result<()> {
        match command.trim() {
            "/help" => self.handle_help(channel).await,
            "/new-session" => self.handle_new_session(channel, agent_manager).await,
            _ => {
                self.slack_client
                    .send_message(
                        channel,
                        &format!("❓ Unknown command: `{}`\n\nType `/help` for available commands.", command),
                        None,
                    )
                    .await?;
                Ok(())
            }
        }
    }

    async fn handle_help(&self, channel: &ChannelId) -> Result<()> {
        let help_text = r#"📚 *Available Commands*

`/help` - Show this help message
`/new-session` - Start a fresh conversation (clears context)

*Examples:*
- Type `/new-session` to start over with a clean slate
- Type `/help` anytime to see available commands

*Note:* Commands must be sent as a message to the bot, not as Slack's built-in slash commands."#;

        self.slack_client.send_message(channel, help_text, None).await?;
        Ok(())
    }

    async fn handle_new_session(
        &self,
        channel: &ChannelId,
        agent_manager: &AgentManager,
    ) -> Result<()> {
        // Get agent for channel
        let agent_mutex = agent_manager.get_repo_agent(channel).await?;
        let mut agent = agent_mutex.lock().await;

        // Start new session
        let new_session_id = agent.start_new_session(channel).await?;

        // Notify user
        let message = format!(
            r#"🔄 *New Session Started*

Session ID: `{}`

Your conversation context has been cleared. You can now start fresh!

*What does this mean?*
- Previous conversation history is no longer accessible
- The bot won't remember earlier discussions in this channel
- Great for switching to a completely different task"#,
            new_session_id
        );

        self.slack_client.send_message(channel, &message, None).await?;
        Ok(())
    }
}
```

```rust
// Update src/slack/messages.rs::process_message()
pub async fn process_message(&self, message: SlackMessage) -> Result<()> {
    // Check if message is a command
    if message.text.starts_with('/') {
        let command_handler = SlackCommandHandler::new(self.slack_client.clone());
        return command_handler
            .handle_command(&message.text, &message.channel, &self.agent_manager)
            .await;
    }

    // ... existing message processing logic ...
}
```

### Phase 4: Agent Startup Notification

**Files to modify:**
- `src/agent/manager.rs`: Send notification after agent creation

**Implementation:**

```rust
// In AgentManager::create_repo_agent()
async fn create_repo_agent(&self, channel_id: ChannelId) -> Result<RepoAgent> {
    let mut agent = RepoAgent::new(...).await?;
    agent.connect().await?;

    // Send session start notification
    let session_id = agent.get_session_id();
    let notification = format!(
        "🤖 *Agent Ready*\n\nSession ID: `{}`\n\nI'm ready to help with this repository!",
        session_id
    );

    self.progress_tracker
        .slack_client()
        .send_message(&channel_id, &notification, None)
        .await?;

    Ok(agent)
}
```

### Phase 5: Testing Strategy

**Unit Tests:**
- Session ID generation (uniqueness, format)
- Command parsing
- Session switching

**Integration Tests:**
- `/help` command response
- `/new-session` command creates new session
- Messages after `/new-session` use new session ID
- Multiple sessions across different channels

**Manual Tests:**
- Send `/help` → Verify help text displayed
- Have conversation → Send `/new-session` → Verify context cleared
- Send message → Verify it uses new session
- Restart bot → Verify new session created on startup

## Data Models

### Session ID

```rust
pub type SessionId = String;

pub struct SessionInfo {
    pub id: SessionId,
    pub channel_id: ChannelId,
    pub created_at: SystemTime,
}
```

### Command Result

```rust
pub enum CommandResult {
    Help,
    NewSession(SessionId),
}
```

## Edge Cases & Error Handling

### Case 1: Command sent when no agent exists

```
User: /new-session
Bot: ⚠️ No agent configured for this channel.
     Please mention me with a repository name to set up first.
```

### Case 2: Unknown command

```
User: /foo
Bot: ❓ Unknown command: `/foo`
     Type `/help` for available commands.
```

### Case 3: Agent busy processing

```
User: /new-session
Bot: (Queue the command, execute after current message completes)
     Or: "⚠️ Agent is currently busy. Please try again in a moment."
```

### Case 4: Session creation fails

```
Try 1: Generate new session ID
If fails: Retry with different random suffix
If still fails: Use fallback session ID format
Never fail the user request
```

## UI/UX Design

### Help Command Response

```
📚 *Available Commands*

`/help` - Show this help message
`/new-session` - Start a fresh conversation (clears context)

*Examples:*
- Type `/new-session` to start over with a clean slate
- Type `/help` anytime to see available commands

*Note:* Commands must be sent as a message to the bot, not as Slack's built-in slash commands.
```

### New Session Notification (Manual)

```
🔄 *New Session Started*

Session ID: `session-C09NNKZ8SPP-1761520471-a3f9b2`

Your conversation context has been cleared. You can now start fresh!

*What does this mean?*
- Previous conversation history is no longer accessible
- The bot won't remember earlier discussions in this channel
- Great for switching to a completely different task
```

### New Session Notification (Agent Startup)

```
🤖 *Agent Ready*

Session ID: `session-C09NNKZ8SPP-1761520471-a3f9b2`

I'm ready to help with this repository! Type `/help` for available commands.
```

## Implementation Checklist

### Phase 1: Core Session Management
- [ ] Add `uuid` dependency to Cargo.toml
- [ ] Create `src/session.rs` with session ID generation
- [ ] Add `current_session_id` field to `RepoAgent`
- [ ] Update `RepoAgent::new()` to generate session ID
- [ ] Implement `start_new_session()` method
- [ ] Implement `get_session_id()` method

### Phase 2: Use Sessions in Queries
- [ ] Update `RepoAgent::query()` to use `query_with_session()`
- [ ] Test that messages use correct session ID
- [ ] Verify session persistence across messages

### Phase 3: Command Handling
- [ ] Create `src/slack/commands.rs`
- [ ] Implement `SlackCommandHandler` struct
- [ ] Implement `/help` handler
- [ ] Implement `/new-session` handler
- [ ] Add command detection in `MessageProcessor`
- [ ] Update `src/slack/mod.rs` exports

### Phase 4: Startup Notifications
- [ ] Add notification on agent creation
- [ ] Update `AgentManager::create_repo_agent()`
- [ ] Update `AgentManager::scan_and_restore_channels()`
- [ ] Test notification appears on bot restart

### Phase 5: Testing
- [ ] Unit test: session ID generation
- [ ] Unit test: session ID uniqueness
- [ ] Unit test: command parsing
- [ ] Integration test: `/help` command
- [ ] Integration test: `/new-session` command
- [ ] Integration test: session persistence
- [ ] Manual test: Full workflow

## Acceptance Criteria

- [x] `/help` command displays available commands
- [x] `/new-session` command creates new session
- [x] New session notification is clear and informative
- [x] Session IDs are unique across calls
- [x] Messages use correct session ID
- [x] Agent startup sends session notification
- [x] Commands work when agent is busy (queued or rejected gracefully)
- [x] Session switching doesn't lose messages
- [x] Each channel has independent sessions

## Dependencies

### New Dependencies

**uuid** (v1.18 or latest):
```toml
[dependencies]
uuid = { version = "1.18", features = ["v4", "serde"] }
```

Already exists in the project (used by claude-agent-sdk-rs).

### API Dependencies

**Claude Agent SDK** (v0.2.1+):
- `ClaudeClient::query_with_session()`
- `ClaudeClient::new_session()`

## Migration Path

### Backward Compatibility

**Existing agents** (created before this feature):
- Don't have `current_session_id` field
- Need to regenerate agents or add migration

**Solution**: Regenerate all agents on bot restart (already happens via `scan_and_restore_channels`)

### Rollout Strategy

**Phase 1: Deploy**
1. Deploy code
2. Restart bot
3. All agents regenerated with session support

**Phase 2: Announce**
1. Send message to all active channels
2. Inform users about `/help` and `/new-session`

## Future Enhancements

### V2 Features

**Session History** (optional):
- Store session IDs and timestamps
- `/sessions` command to list past sessions
- `/resume <session-id>` to switch back

**Session Names** (optional):
- Allow custom session names
- `/new-session <name>` creates named session
- Easier to identify than IDs

**Session Persistence** (optional):
- Save session state to disk
- Survive bot restarts
- Resume exact conversation state

**Analytics** (optional):
- Track session duration
- Track messages per session
- Identify when users frequently restart sessions

## Security Considerations

### Session ID Privacy

- Session IDs are not sensitive data
- Contain channel ID (already known to channel members)
- Random suffix prevents guessing
- No PII or secrets in session ID

### Command Injection

- Commands are exact string matches
- No dynamic execution
- No shell command injection risk

### Rate Limiting

- Commands count toward message processing
- Use existing rate limiting mechanisms
- No special handling needed

## Examples

### Example 1: User Workflow

```
User: @bot Implement user authentication
Bot: 🤖 Agent Ready
     Session ID: session-C09NNKZ8SPP-1761520471-a3f9b2

[... conversation about authentication ...]

User: /new-session
Bot: 🔄 New Session Started
     Session ID: session-C09NNKZ8SPP-1761521234-b8c4d1
     Previous context cleared.

User: @bot How do I deploy this?
Bot: [Answers without knowing about authentication discussion]
```

### Example 2: Help Command

```
User: /help
Bot: 📚 *Available Commands*

     `/help` - Show this help message
     `/new-session` - Start a fresh conversation

     ...
```

### Example 3: Multiple Channels

```
Channel A: session-C09NNKZ8SPP-1761520471-a3f9b2 (discussing auth)
Channel B: session-C09NNMDNJH3-1761520472-c5d6e2 (discussing database)

Each maintains independent context.
```

## Technical Notes

### Why query_with_session instead of fork_session?

**`query_with_session(prompt, session_id)`**:
- ✅ Explicit session control
- ✅ Can switch between sessions
- ✅ Session ID is visible and traceable
- ✅ Can have multiple concurrent sessions

**`fork_session=true` option**:
- ❌ Global setting (all queries fork)
- ❌ No session identity
- ❌ Can't resume previous sessions
- ❌ Less control

**Decision**: Use `query_with_session` for explicit control and traceability.

### Session State

**What's preserved in a session:**
- Conversation history
- Tool use context
- File reads/writes made
- Variable state (if any)

**What's NOT preserved:**
- TodoWrite plan (managed separately)
- Git state (actual repository)
- File system state

**Implication**: Sessions are for conversation memory, not state management.

## Open Questions

**Q1**: Should we show session ID in every response?
**A1**: No - only on session start. It clutters regular messages.

**Q2**: Should we persist sessions across bot restarts?
**A2**: No initially - sessions are ephemeral. Add later if needed.

**Q3**: Should we have a timeout for sessions?
**A3**: Use existing agent timeout mechanism. When agent times out and is removed, session is also lost.

**Q4**: Should `/new-session` take an optional name parameter?
**A4**: Not in V1. Keep simple. Can add in V2.

**Q5**: Should we clear the TodoWrite plan on `/new-session`?
**A5**: Yes! Clear the plan to match the cleared conversation context.

## Success Metrics

- Users can successfully use `/help` command
- Users can successfully use `/new-session` command
- Session IDs are unique (no collisions observed)
- No message loss during session transitions
- Clear user feedback on session changes
- 0 errors related to session management in logs

## References

- Claude Agent SDK v0.2.1 Changelog
- Example: `vendors/claude-agent-sdk-rs/examples/16_session_management.rs`
- Slack mrkdwn formatting: https://api.slack.com/reference/surfaces/formatting
