# Feature: Enhanced Logging for Request Flow Tracing

## Overview

The current logging implementation provides basic event tracking but lacks sufficient detail to understand the complete request flow through the application. This specification defines comprehensive logging enhancements to enable developers to trace user requests from Slack event reception through Claude API interaction and back to Slack response delivery.

## Problem Statement

Current logging gaps:
- Missing entry/exit markers for major operations
- Insufficient timing information for performance analysis
- Lack of request correlation IDs to trace related log entries
- Minimal visibility into agent state transitions
- Limited error context (missing upstream call stack info)
- Sparse logging during Claude API streaming responses

## Requirements

### Functional Requirements

**REQ-1: Request Flow Visibility**
- Every user request must be traceable from Slack event → Agent processing → Claude API → Response delivery
- Log entries must clearly indicate phase transitions (received, processing, streaming, completed)
- All logs related to a single request must be easily correlatable

**REQ-2: Performance Metrics**
- Log duration for key operations: agent locking, Claude API calls, message formatting, Slack delivery
- Include cumulative timing at completion to measure end-to-end latency
- Identify slow operations (> 1 second) with warnings

**REQ-3: State Transition Logging**
- Log agent lifecycle events: creation, connection, query execution, session changes, disconnection
- Track concurrent request handling (when multiple channels are active)
- Log message streaming progress (chunks received, bytes processed)

**REQ-4: Error Context**
- Include upstream operation context in error logs (what was being attempted)
- Log partial failure scenarios (e.g., Claude responded but Slack delivery failed)
- Preserve error chains with full context

**REQ-5: Configuration and Control**
- Support standard RUST_LOG environment variable for level control
- Maintain emoji prefixes for visual log scanning
- Keep structured logging format with consistent fields

### Non-Functional Requirements

**Performance**
- Logging overhead must not exceed 5% of request processing time
- Use efficient string formatting (avoid unnecessary allocations)
- Leverage tracing span context for automatic field propagation

**Maintainability**
- Follow existing conventions (emoji prefixes, channel.log_format())
- Use consistent log message formats across modules
- Include line numbers and targets for debugging

## Architecture

### Logging Levels Strategy

```
ERROR   - System failures, API errors, data corruption
WARN    - Degraded performance, retries, unexpected states
INFO    - Request flow milestones, state transitions, completion events
DEBUG   - Detailed operation steps, variable values, loop iterations
TRACE   - Low-level SDK calls, internal state dumps (rarely used)
```

### Structured Logging Fields

Every significant operation should include:
- `channel_id` - Channel identifier
- `session_id` - Session identifier (where applicable)
- `user_id` - User identifier (where applicable)
- `operation` - High-level operation name
- `duration_ms` - Operation duration (at completion)

### Request Correlation

Use tracing spans to correlate related log entries:

```rust
let span = tracing::info_span!(
    "process_user_message",
    channel_id = %channel.log_format(),
    user_id = %user.as_str(),
    msg_length = text.len()
);
let _guard = span.enter();
```

## Implementation Steps

### Phase 1: Core Flow Enhancement (events.rs, messages.rs)

**events.rs changes:**
1. Add span for event processing lifecycle
2. Log event deduplication decisions with event key
3. Add timing for event routing (mention vs setup vs command)
4. Log background task spawn with unique task ID

**messages.rs changes:**
1. Add span for message processing with correlation fields
2. Log agent lock acquisition timing (contention indicator)
3. Add streaming progress logs (every N chunks or X seconds)
4. Log response size and formatting duration
5. Add completion summary with total duration

### Phase 2: Agent Lifecycle (manager.rs, repo_agent.rs)

**manager.rs changes:**
1. Log agent pool state periodically (active agents count)
2. Add detailed logging for agent restoration (success/failure per channel)
3. Log cleanup operations with removed agent details
4. Add timing for MainAgent setup operations

**repo_agent.rs changes:**
1. Log session transitions with before/after session IDs
2. Add Claude connection establishment timing
3. Log query submission with truncated preview
4. Add streaming rate statistics (chunks/sec, bytes/sec)
5. Log activity updates and expiration checks

### Phase 3: User Interactions (forms.rs, commands.rs)

**forms.rs changes:**
1. Log setup form display with channel context
2. Add validation logging (format checks, repository checks)
3. Log each setup phase transition with timing
4. Add completion summary with total setup duration

**commands.rs changes:**
1. Log command parsing and routing
2. Add per-command handler entry/exit logs
3. Log session state before/after /new-session
4. Include help command invocation statistics

### Phase 4: Slack Client (client.rs)

**client.rs changes:**
1. Log API call attempts with endpoint and payload size
2. Add retry logic logging (if implemented)
3. Log rate limiting indicators
4. Add response status code and size logging

## Testing Strategy

### Manual Testing

**Test Case 1: Basic Request Flow**
1. Send simple message to bot
2. Verify logs show: event received → message processing → agent locked → query sent → streaming → response formatted → delivered
3. Check duration is logged at each step

**Test Case 2: Error Scenarios**
1. Trigger agent not found error
2. Verify error log includes channel ID and upstream operation
3. Check user receives helpful error message

**Test Case 3: Concurrent Requests**
1. Send messages from multiple channels simultaneously
2. Verify logs clearly distinguish between channels
3. Check no log interleaving confusion

**Test Case 4: Large Response**
1. Request large output from Claude
2. Verify streaming progress logs appear
3. Check chunking logs when response exceeds Slack limit

### Automated Testing

- Add unit test for log formatting utilities (if created)
- Test span context propagation in isolated async tasks
- Verify no log output in release builds with RUST_LOG=error

## Acceptance Criteria

- [ ] Every user request produces clear entry → processing → completion log trail
- [ ] Operation durations are logged for all major steps
- [ ] Error logs include sufficient context to diagnose issues
- [ ] Logs can be filtered by channel using standard tools (grep, etc.)
- [ ] No performance regression (< 5% overhead)
- [ ] Existing log formatting conventions (emoji, channel.log_format()) preserved
- [ ] Documentation updated with logging best practices

## Examples

### Before (Current Logging)

```
INFO slack_coder::slack::events: 📨 Received push event: AppMention
INFO slack_coder::slack::events: 🔔 App mentioned C09NU1KFXHT by user: U123
INFO slack_coder::slack::messages: 💬 Processing message C09NU1KFXHT user=U123
INFO slack_coder::slack::messages: ✅ Agent found! Forwarding message to repository agent...
INFO slack_coder::slack::messages: ✅ Response sent C09NU1KFXHT
```

### After (Enhanced Logging)

```
INFO slack_coder::slack::events: 📨 Received push event: AppMention channel=C09NU1KFXHT event_key=mention:C09NU1KFXHT:1234567890.123456
DEBUG slack_coder::slack::events: ✅ New event, processing: mention:C09NU1KFXHT:1234567890.123456
INFO slack_coder::slack::events: 🔔 App mentioned C09NU1KFXHT by user: U123 text_length=45
DEBUG slack_coder::slack::events: 🧹 Cleaned text: 'help me write a function'
INFO slack_coder::slack::events: 💬 Processing regular message, spawning handler task_id=abc123
INFO slack_coder::slack::messages: 💬 Processing message C09NU1KFXHT user=U123 session=session-C09NU1KFXHT-20250127-xyz
DEBUG slack_coder::slack::messages: 🔍 Agent check C09NU1KFXHT has_agent=true
INFO slack_coder::slack::messages: ✅ Agent found! Forwarding to repository agent...
DEBUG slack_coder::slack::messages: 🔒 Acquiring agent lock C09NU1KFXHT...
INFO slack_coder::slack::messages: 🔒 Agent locked C09NU1KFXHT (wait_time=12ms), sending query to Claude...
DEBUG slack_coder::agent::repo_agent: 📤 Sending query session=session-C09NU1KFXHT-20250127-xyz text_preview='help me write a...'
INFO slack_coder::slack::messages: ✅ Query sent C09NU1KFXHT, streaming response...
DEBUG slack_coder::slack::messages: 📦 Received chunk #1 from Claude (size=256 bytes)
DEBUG slack_coder::slack::messages: 📦 Received chunk #5 from Claude (size=512 bytes)
INFO slack_coder::slack::messages: ✅ Received final result C09NU1KFXHT (3847 chars, stream_duration=2.3s)
DEBUG slack_coder::slack::messages: 🔄 Converting markdown to Slack format (before=3847 chars)
DEBUG slack_coder::slack::messages: 📤 Sending response C09NU1KFXHT (after=3921 chars, format_duration=5ms)
INFO slack_coder::slack::messages: ✅ Response sent C09NU1KFXHT (total_duration=2.5s)
```

## Migration Plan

1. **Implement enhancements incrementally** - One module at a time
2. **Deploy with DEBUG level** - Initially to capture detailed flow
3. **Monitor performance impact** - Check latency hasn't increased
4. **Tune to INFO level** - After validation, reduce verbosity for production
5. **Document findings** - Create debugging guide with common log patterns

## Related Documents

- `docs/DEBUGGING.md` - Will be updated with logging best practices
- `prompts/main-agent-system-prompt.md` - Logging conventions reference
