# Slack Coder Bot - Implementation Plan

## Overview

This document outlines the step-by-step implementation plan for the Slack Coder Bot. The plan follows a bottom-up approach, building foundational components first and gradually assembling them into the complete system.

## Phase 1: Foundation & Core Types (Days 1-2)

### Task 1.1: Project Setup
- Initialize Cargo workspace structure
- Add all dependencies to Cargo.toml
- Set up module structure (config, slack, agent, storage, error)
- Configure basic logging with tracing

### Task 1.2: Error Types
- Define SlackCoderError enum with all variants
- Implement Display and Error traits
- Create Result type alias
- Add error conversion implementations

### Task 1.3: Configuration
- Define Settings, SlackConfig, ClaudeConfig, WorkspaceConfig, AgentConfig structs
- Implement configuration loading from environment variables
- Add validation for required fields
- Create .env.example file

### Task 1.4: Basic Types
- Define ChannelId, UserId, ThreadTs, MessageTs wrapper types
- Implement Hash, PartialEq, Eq for ChannelId (needed for DashMap)
- Define SlackMessage struct
- Define Task, TaskStatus, Plan structs (matching claude-agent-sdk-rs format)

### Task 1.5: Workspace Path Management
- Implement Workspace struct with base_path
- Add methods: repo_path(), system_prompt_path()
- Add method: is_channel_setup()
- Add method: load_system_prompt()
- Create workspace directory structure on initialization

## Phase 2: Slack Integration (Days 3-5)

### Task 2.1: Slack Client Wrapper
- Initialize slack-morphism client with bot token
- Implement send_message() method
- Implement update_message() method
- Implement send_code_block() method with syntax highlighting
- Handle Slack API errors and retries

### Task 2.2: Progress Tracker
- Implement ProgressTracker with DashMap for active messages
- Implement start_progress() to create initial message
- Implement update_progress() to edit existing message
- Implement format_plan() to convert Plan to Slack markdown with emojis
- Use ✅ ⏳ ⬜ for completed/in-progress/pending status

### Task 2.3: Interactive Forms
- Implement FormHandler struct
- Create show_repo_setup_form() using Slack Block Kit
- Add form with single input field for repository name
- Implement handle_form_submission() to parse form data
- Add validate_repo_name_format() to check owner/repo pattern

### Task 2.4: Event Handler
- Set up Slack Events API or Socket Mode listener
- Implement handle_app_mention() for @bot mentions
- Implement handle_message() for thread messages
- Filter out bot's own messages
- Route events to appropriate handlers

### Task 2.5: Message Processor
- Implement MessageProcessor struct
- Add process_message() to route user messages
- Check if channel has configured agent
- If no agent, show setup form
- If agent exists, forward message to repo agent

## Phase 3: Agent Integration (Days 6-9)

### Task 3.1: Hook Implementation
- Implement create_todo_hooks() function
- Set up PostToolUse hook with "TodoWrite" matcher
- Parse Plan from tool_input in hook
- Update internal Arc<Mutex<Plan>>
- Call progress_tracker.update_progress() from hook
- Return SyncHookJsonOutput

### Task 3.2: Main Agent
- Implement MainAgent struct with ClaudeClient and Arc<Mutex<Plan>>
- Create new() method that sets up agent with hooks
- Load main agent system prompt from specs/0003-system-prompt.md
- Configure ClaudeAgentOptions with:
  - SystemPrompt from file
  - PermissionMode::BypassPermissions
  - Hooks from create_todo_hooks()
  - Working directory (will change per setup)
- Implement connect(), query(), disconnect() lifecycle methods

### Task 3.3: Main Agent Setup Flow
- Implement setup_repository() method
- Format setup prompt with channel_id and repo_name
- Call client.query() with formatted prompt
- Stream response using receive_response()
- Monitor plan updates via get_plan()
- Wait for completion or error
- Verify system prompt was saved to correct location

### Task 3.4: Repo Agent
- Implement RepoAgent struct with ClaudeClient and Arc<Mutex<Plan>>
- Create new() method that loads system prompt from disk
- Configure ClaudeAgentOptions with:
  - SystemPrompt from ~/.slack_coder/system/{channel_id}/system_prompt.md
  - Working directory: ~/.slack_coder/repos/{channel_id}/
  - Hooks from create_todo_hooks()
- Implement connect(), query(), receive_response() methods
- Track last_activity timestamp

### Task 3.5: Agent Manager
- Implement AgentManager with main_agent and DashMap<ChannelId, RepoAgent>
- Create new() to initialize main agent
- Implement setup_channel() to invoke main agent
- Implement get_repo_agent() to retrieve from DashMap
- Create repo agent after successful setup
- Implement has_agent() to check if channel is configured

### Task 3.6: Channel Scanning & Restoration
- Implement scan_and_restore_channels() method
- Query Slack API for all channels where bot is member
- For each channel, check workspace.is_channel_setup()
- If setup, create and cache RepoAgent
- Load system prompt and initialize agent
- Add to repo_agents DashMap

## Phase 4: End-to-End Workflows (Days 10-12)

### Task 4.1: Channel Setup Workflow
- User invites bot to channel
- Bot detects app_mention or join event
- If not setup, show repo setup form
- User submits form with repo name
- Validate repo name format
- Call agent_manager.setup_channel()
- Main agent executes setup (validate, clone, analyze, generate prompt)
- Progress updates displayed via TodoWrite hook
- On completion, create RepoAgent and add to DashMap
- Send success message to channel

### Task 4.2: Message Forwarding Workflow
- User mentions bot with message
- Message processor checks if channel has agent
- If no agent, show setup form
- If agent exists, get from DashMap
- Call agent.query() with user message
- Stream response using receive_response()
- For each Message chunk:
  - If text content, send to Slack (with threading)
  - If TodoWrite hook fires, progress tracker updates message
  - If code blocks, format with syntax highlighting
- Update agent last_activity timestamp

### Task 4.3: Error Handling & Recovery
- Handle git/gh errors from main agent (clone failures, auth issues)
- Handle missing system prompt errors when creating repo agent
- Handle Slack API rate limits and retries
- Handle agent disconnections and reconnections
- Provide user-friendly error messages in Slack
- Allow retry of setup process

### Task 4.4: Agent Lifecycle Management
- Implement cleanup_inactive_agents() background task
- Check agents for expiration based on last_activity
- Disconnect and remove expired agents from DashMap
- Optionally save agent state before removal
- Run cleanup task every N minutes

## Phase 5: Testing & Polish (Days 13-15)

### Task 5.1: Unit Tests
- Test configuration loading and validation
- Test workspace path generation
- Test Plan update logic
- Test progress message formatting
- Test repo name validation

### Task 5.2: Integration Tests
- Mock Slack client for testing
- Test full setup workflow with mock agent
- Test message forwarding with mock responses
- Test progress tracking with mock TodoWrite calls
- Test concurrent channel handling

### Task 5.3: Manual Testing
- Create test Slack workspace
- Test with real repositories of various types
- Test with multiple channels simultaneously
- Test error scenarios (invalid repo, auth failures)
- Test agent cleanup after inactivity

### Task 5.4: Documentation
- Update README with setup instructions
- Document environment variables
- Add example .env file
- Document Slack app configuration requirements
- Add troubleshooting guide

### Task 5.5: Deployment Preparation
- Create Dockerfile
- Add docker-compose.yml for local development
- Document deployment to cloud platforms
- Set up health check endpoint
- Configure graceful shutdown handling

## Implementation Notes

### Critical Path Items
1. Hook implementation (Task 3.1) - Core to progress tracking
2. Main agent setup flow (Task 3.3) - Core to repository initialization
3. Channel scanning (Task 3.6) - Required for bot startup
4. Message forwarding (Task 4.2) - Core user interaction

### Risk Areas
1. **Hook timing**: Ensure hooks fire before responses complete
2. **Agent state**: Properly manage Arc<Mutex<Plan>> across async boundaries
3. **Slack rate limits**: Implement proper backoff and retry logic
4. **Large repositories**: Handle cloning timeouts and size limits
5. **Concurrent requests**: Ensure proper request queuing per agent

### Testing Strategy
- Unit test all components in isolation
- Integration test workflows with mocks
- Manual test with real Slack workspace
- Test with various repository types and sizes
- Load test with multiple concurrent channels

## Success Criteria

### Milestone 1 (End of Phase 2)
- [ ] Bot connects to Slack successfully
- [ ] Bot responds to mentions
- [ ] Progress tracker displays formatted messages
- [ ] Forms are displayed and submitted correctly

### Milestone 2 (End of Phase 3)
- [ ] Main agent can be created with hooks
- [ ] TodoWrite hook updates progress in Slack
- [ ] Repo agent can be created and connected
- [ ] Agent manager maintains agent pool

### Milestone 3 (End of Phase 4)
- [ ] Complete setup workflow works end-to-end
- [ ] User can send messages and get responses
- [ ] Progress tracking works during both setup and coding
- [ ] Multiple channels work independently

### Final Release (End of Phase 5)
- [ ] All tests pass
- [ ] Documentation complete
- [ ] Deployment ready
- [ ] Tested with 5+ different repositories
- [ ] Performance acceptable with 10+ concurrent channels

## Timeline Summary

- **Week 1**: Foundation + Slack Integration (Phases 1-2)
- **Week 2**: Agent Integration (Phase 3)
- **Week 3**: Workflows + Testing (Phases 4-5)

Total: **15 working days / 3 weeks**
