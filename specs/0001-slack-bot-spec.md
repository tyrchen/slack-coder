# Slack Coder Bot - Requirements Specification

## 1. Overview

The Slack Coder Bot is an intelligent assistant that integrates with Slack channels to provide automated code and documentation generation using Claude AI. The bot acts as a message broker between Slack and Claude agents, delegating all heavy lifting (repository operations, code analysis, code generation) to Claude agents. When invited to a channel, the bot facilitates the setup process where the main Claude agent clones and analyzes a GitHub repository, then spawns a repository-specific agent for ongoing coding assistance.

## 2. Architecture Philosophy

The Slack Coder Bot follows a clear separation of concerns:

- **Slack Bot Application**: Handles Slack API communication, message routing, and UI interactions
- **Main Claude Agent**: One-time setup agent that handles repository validation, cloning, analysis, and system prompt generation
- **Repository-Specific Agents**: Long-lived agents (one per channel) that handle all coding tasks with full repository context

The bot's primary role is to pass messages between Slack and Claude agents, extract progress information (via PostToolUse hooks), and present it to users in Slack.

## 3. Core Features

### 3.1 Bot Startup and Channel Discovery

**FR-1.0: Startup Channel Scan**

- On startup, the bot shall query Slack API for all channels where the bot is a member
- For each channel, the bot shall check if a repository is already configured at `~/.slack_coder/repos/{channel_id}`
- If configured, the bot shall initialize a repository-specific agent for that channel
- If not configured, the bot shall wait for user to provide repository information
- The bot shall store channel-to-agent mapping in `DashMap<ChannelId, RepoAgent>`

### 3.2 Bot Initialization (New Channel)

**FR-1.1: Channel Invitation**

- When the bot is invited to a new Slack channel, it shall trigger an initialization workflow
- The bot shall post a welcome message introducing itself
- The bot shall present an interactive form to collect repository information

**FR-1.2: Repository Information Collection**

- The bot shall collect the following information via Slack interactive form:
  - Repository name (format: `owner/repo-name`, e.g., `tyrchen/rust-lib-template`)
- The form shall validate the repository name format before submission
- The bot shall handle form submission and cancellation events

**FR-1.3: Main Agent Invocation**

- The bot shall invoke the main Claude agent with the repository name and channel ID
- The bot shall pass a setup prompt to the main agent:
  ```
  Please set up the repository {owner}/{repo-name} for channel {channel_id}.

  Tasks:
  1. Validate the repository exists and is accessible using gh CLI
  2. Clone it to ~/.slack_coder/repos/{channel_id}
  3. Analyze the codebase comprehensively
  4. Generate a system prompt for this repository
  5. Save the system prompt to ~/.slack_coder/system/{channel_id}/system_prompt.md

  The repository name provided by the user is: {owner}/{repo-name}
  ```
- The bot shall NOT perform any git or gh operations itself
- All repository operations are delegated to the main Claude agent

### 3.3 Agent Progress Tracking

**FR-2.1: PostToolUse Hook Integration**

- The bot shall configure a PostToolUse hook for TodoWrite tool in Claude Agent SDK
- When the main agent or repo agent uses TodoWrite, the hook shall intercept the todo list data
- The bot shall extract todo items and their states from the hook payload
- The bot shall format and send todo updates to the Slack channel

**FR-2.2: Progress Updates**

- The bot shall display todo lists in Slack as formatted messages:
  ```
  Setup Progress:
  ✅ Validate repository access
  ⏳ Clone repository to workspace
  ⬜ Analyze codebase
  ⬜ Generate system prompt
  ⬜ Initialize repository agent
  ```
- As the agent marks todos as in_progress or completed, the bot shall update the message
- The bot shall use Slack message updates (edit existing message) to show live progress

**FR-2.3: Agent Response Streaming**

- The bot shall stream agent text responses to Slack in real-time
- For long responses, the bot shall split into multiple messages if needed (respecting Slack's 40KB limit)
- The bot shall format code blocks with appropriate syntax highlighting
- The bot shall preserve markdown formatting in agent responses

### 3.4 Repository Analysis and Setup

**FR-3.1: Repository Validation and Cloning**

- The main Claude agent shall validate repository accessibility using `gh repo view`
- The main Claude agent shall clone the repository to: `~/.slack_coder/repos/{channel_id}/`
- If validation or cloning fails, the agent shall report errors back to the bot
- The bot shall relay error messages to the Slack channel with actionable instructions
- The bot shall allow users to retry setup process

**FR-3.2: Codebase Analysis**

- The main Claude agent shall perform comprehensive codebase review:
  - Identify programming languages and frameworks used
  - Analyze project structure and architecture patterns
  - Detect coding conventions and style guidelines
  - Identify dependencies and build systems
  - Understand domain-specific concepts and terminology
  - Review documentation standards
  - Identify test frameworks and patterns

**FR-3.3: System Prompt Generation**

- The main Claude agent shall generate a repository-specific system prompt based on analysis
- The system prompt shall be saved to: `~/.slack_coder/system/{channel_id}/system_prompt.md`
- The system prompt shall include:
  - Project overview and purpose
  - Technology stack and frameworks
  - Architecture and design patterns
  - Coding conventions and style guidelines
  - File structure and organization
  - Testing approach and frameworks
  - Documentation standards
  - Common patterns and idioms used in the codebase
  - Domain-specific knowledge and terminology

**FR-3.4: Repository Agent Initialization**

- After successful setup, the bot shall create a repository-specific Claude agent
- The agent shall be initialized with the system prompt from `~/.slack_coder/system/{channel_id}/system_prompt.md`
- The agent's working directory shall be set to `~/.slack_coder/repos/{channel_id}/`
- The agent shall have access to all tools provided by Claude Agent SDK
- The agent instance shall be stored in `DashMap<ChannelId, RepoAgent>`

**FR-3.5: Setup Completion**

- The bot shall send a completion message to the channel including:
  - Repository name and successful setup confirmation
  - Available capabilities overview
  - Quick start examples
  - Available commands

### 3.5 Message Processing

**FR-4.1: Message Listening**

- The bot shall listen to all messages in channels where it's a member
- The bot shall only respond to:
  - Direct mentions (`@SlackCoderBot`)
  - Messages in threads where the bot has participated
  - Direct messages to the bot
- The bot shall ignore its own messages to prevent loops

**FR-4.2: Message Routing**

- The bot shall extract the user's message text
- The bot shall look up the repository-specific agent for the channel from `DashMap<ChannelId, RepoAgent>`
- If no agent exists for the channel, the bot shall prompt the user to set up a repository first
- The bot shall forward the user's message to the repository-specific agent
- The bot shall pass conversation context (thread_ts, user_id, etc.) to the agent

**FR-4.3: Command Support**

- The bot shall pass all user messages to the repository agent without command interpretation
- The repository agent shall handle all Claude Agent SDK slash commands:
  - `/help` - Show available commands
  - `/context` - Show the context of the current conversation
  - Custom slash commands defined in the repository
- The bot shall relay command results from agent back to Slack

**FR-4.4: Natural Language Processing**

- The repository agent shall process natural language instructions for:
  - Code generation
  - Documentation generation
  - Code refactoring
  - Bug fixing
  - Feature implementation
  - Code explanation
- The bot shall maintain conversation context within threads by passing thread_ts
- The repository agent shall maintain conversation history internally

**FR-4.5: Response Handling**

- The bot shall stream agent text responses to Slack in real-time
- The bot shall use threading to keep conversations organized
- The bot shall intercept TodoWrite via PostToolUse hook to show progress updates
- The bot shall format code blocks with appropriate syntax highlighting
- The bot shall use Slack's rich text formatting for better readability

### 3.6 Agent-Driven Operations

All file operations, git operations, and repository modifications are performed by the repository-specific Claude agent. The bot merely relays requests and responses.

**FR-5.1: Code Generation**

- The repository agent shall generate code based on user instructions
- Generated code shall follow repository conventions (learned during setup)
- The agent shall create/modify files in `~/.slack_coder/repos/{channel_id}/`
- The agent shall provide file diffs and the bot shall format them for Slack

**FR-5.2: Documentation Generation**

- The repository agent shall generate documentation following project standards
- Documentation shall be placed in appropriate locations
- The agent shall update existing docs or create new ones as needed

**FR-5.3: Git Integration**

- The repository agent shall create git branches for changes
- The repository agent shall commit changes with descriptive messages
- The repository agent shall optionally create pull requests via `gh` CLI
- The repository agent shall push changes when requested
- All git operations are performed by the agent, not the bot

### 3.7 Multi-Channel Support

**FR-6.1: Channel Isolation**

- Each channel shall have its own repository at `~/.slack_coder/repos/{channel_id}/`
- Each channel shall have its own system prompt at `~/.slack_coder/system/{channel_id}/system_prompt.md`
- Each channel shall have its own repository-specific agent instance
- Multiple channels cannot share the same repository (enforced by channel-specific paths)
- Each channel shall have isolated conversation context
- The bot shall prevent cross-channel data leakage

**FR-6.2: Agent Management**

- The bot shall maintain one main Claude agent instance (for setup operations only)
- The bot shall maintain a pool of repository-specific agents in `DashMap<ChannelId, RepoAgent>`
- Repository agents shall be created on-demand during channel setup
- Repository agents shall be loaded from existing configuration on bot startup
- Inactive agents may be cleaned up after timeout to save resources
- The bot shall handle agent lifecycle properly

### 3.8 Error Handling

**FR-7.1: Graceful Degradation**

- The bot shall handle Slack API failures gracefully
- The bot shall handle Claude agent errors gracefully
- The bot shall retry transient failures automatically
- The bot shall report persistent errors to users clearly
- Agent errors shall be relayed to Slack with full context

**FR-7.2: User Feedback**

- Error messages from agents shall be formatted for readability
- Error messages shall be user-friendly and actionable
- The bot shall suggest solutions when possible (via agent responses)
- The bot shall provide debugging information when requested

## 4. Non-Functional Requirements

### 4.1 Performance

**NFR-1.1: Response Time**

- The bot shall acknowledge messages within 2 seconds
- The bot shall stream agent responses in real-time
- Progress updates (via TodoWrite hook) shall be reflected in Slack within 1 second

**NFR-1.2: Concurrency**

- The bot shall handle multiple channels simultaneously
- The bot shall support concurrent requests within a channel
- Each repository agent shall handle requests sequentially to avoid race conditions
- The bot shall implement proper request queuing for each agent

### 4.2 Reliability

**NFR-2.1: Availability**

- The bot shall maintain 99% uptime
- The bot shall recover automatically from crashes
- The bot shall restore agent pool from disk on restart
- Channel configurations shall persist across restarts

**NFR-2.2: Data Integrity**

- Repository agents shall handle file modifications (bot does not touch files)
- The bot shall maintain consistent state in the agent mapping
- The bot shall gracefully handle agent crashes and restart them

### 4.3 Security

**NFR-3.1: Authentication**

- The bot shall use OAuth for Slack authentication
- Claude agents shall use GitHub credentials from environment (gh CLI uses system auth)
- Credentials shall be stored securely (environment variables/secrets manager)

**NFR-3.2: Authorization**

- The bot shall respect Slack channel permissions
- The bot shall only initialize repositories for channels where bot is a member
- The bot shall validate all user inputs before passing to agents

**NFR-3.3: Data Privacy**

- The bot shall not log sensitive repository content
- Repository data shall be stored in user-controlled workspace
- The bot shall support data deletion requests (delete channel workspace)

### 4.4 Maintainability

**NFR-4.1: Code Quality**

- The bot shall follow Rust best practices
- Code shall be well-documented
- The bot shall have comprehensive test coverage (>70%)
- The bot's role as message broker should be clear in code

**NFR-4.2: Observability**

- The bot shall provide structured logging
- The bot shall log agent invocations and responses
- The bot shall expose metrics for monitoring (messages processed, agent count, etc.)
- TodoWrite hook data shall be logged for debugging

## 5. System Constraints

### 5.1 Technical Constraints

**TC-1.1: Platform Requirements**

- Rust 2024 edition
- Tokio async runtime
- Claude Agent SDK 0.2.0+
- GitHub CLI (`gh`) installed on system
- Git CLI installed on system

**TC-1.2: External Dependencies**

- Slack API (Events API, Web API, Socket Mode)
- Claude API (via claude-agent-sdk-rs)
- All git/gh operations delegated to agents

**TC-1.3: Resource Limits**

- Maximum repository size: 1GB (enforced by agents during clone)
- Maximum concurrent channels: 100
- Message size limit: 40KB (Slack limit)
- Agent context window: As per Claude API limits

### 5.2 Operational Constraints

**OC-1.1: Deployment**

- The bot shall run as a long-lived service
- The bot shall support containerized deployment
- The bot shall support graceful shutdown (cleanup agents)
- Workspace directory `~/.slack_coder/` must be persistent

**OC-1.2: Configuration**

- The bot shall use environment variables for configuration
- The bot shall validate configuration on startup
- Agent configuration (model, etc.) shall be defined at startup

## 6. User Scenarios

### 6.1 Bot Startup

```
System: Bot starts up
Bot: Queries Slack API for all channels where bot is member
Bot: Finds #project-alpha (channel_id: C12345)
Bot: Checks if ~/.slack_coder/repos/C12345 exists
Bot: Found existing repository, loading system prompt
Bot: Initializes repository agent for C12345
Bot: Ready to serve requests in #project-alpha
```

### 6.2 Initial Setup (New Channel)

```
User: Invites @SlackCoderBot to #new-project
Bot: "Welcome! I'm your coding assistant. To get started, please provide your repository."
Bot: [Shows interactive form]
User: Fills in "mycompany/new-project"
Bot: [Forwards to main agent]: "Setup repository mycompany/new-project for channel C67890"
Bot: [Updates from TodoWrite hook]:
     "Setup Progress:
      ⏳ Validate repository access
      ⬜ Clone repository to workspace
      ⬜ Analyze codebase
      ⬜ Generate system prompt
      ⬜ Initialize repository agent"

[Main agent validates via gh]
Bot: [Updates progress]:
     "Setup Progress:
      ✅ Validate repository access
      ⏳ Clone repository to workspace
      ⬜ Analyze codebase
      ⬜ Generate system prompt
      ⬜ Initialize repository agent"

[Main agent clones, analyzes, generates prompt]
Bot: [Final update]:
     "Setup Progress:
      ✅ Validate repository access
      ✅ Clone repository to workspace
      ✅ Analyze codebase
      ✅ Generate system prompt
      ✅ Initialize repository agent"

Bot: "Repository setup complete! You can now ask me to generate code, documentation, or use commands like /help."
```

### 6.3 Code Generation

```
User: @SlackCoderBot add a new API endpoint for user profile updates
Bot: [Forwards to repo agent for C12345]
Bot: [TodoWrite hook]:
     "⏳ Review existing API structure
      ⬜ Design user profile endpoint
      ⬜ Implement endpoint handler
      ⬜ Add tests"

[Agent streams response]
Bot: [Thread] "I'll create a new API endpoint for user profile updates.
     Let me review the existing API structure first..."
Bot: [Progress updates as agent works]
Bot: "Based on the codebase, I'll add the endpoint following your REST patterns.
     Creating: src/api/user_profile.rs"
Bot: [Shows code diff]
Bot: "Would you like me to commit these changes?"
User: yes
Bot: [Forwards to agent]
Bot: "Committed and pushed to branch: feature/user-profile-endpoint"
```

### 6.4 Documentation

```
User: @SlackCoderBot /help
Bot: [Forwards to repo agent]
Bot: [Agent response]:
     "Available commands:
      /help - Show this help
      /context - Show conversation context

      You can also ask me to:
      - Generate code
      - Write documentation
      - Refactor existing code
      - Review and commit changes
      - Create pull requests"
```

## 7. Future Enhancements

### 7.1 Planned Features

- Multi-repository support per channel (allow switching between repos)
- Custom agent hooks and callbacks
- Code review automation (PR reviews)
- Advanced progress tracking (file-level progress bars)
- Conversation persistence and history
- Agent performance metrics
- Background task execution

### 7.2 Possible Extensions

- Support for other version control systems (GitLab, Bitbucket)
- Multi-language support for bot responses
- Voice-to-code via Slack huddles
- Integration with project management tools (Jira, Linear)
- Analytics dashboard for bot usage
- Team collaboration features (shared agents)
- Custom system prompt templates
