# Slack Coder Bot

A Slack bot that integrates with Claude AI to provide intelligent code generation and documentation assistance directly in your Slack channels. The bot analyzes your repository, learns your coding conventions, and helps you write code that matches your project's style.

## Features

- **Repository-Aware**: Analyzes your codebase to understand conventions, patterns, and architecture
- **Channel-Isolated**: Each Slack channel can work with a different repository
- **Real-Time Progress**: TodoWrite hook integration shows live progress updates
- **Context-Aware**: Maintains conversation context within threads
- **Full Claude SDK Support**: Access to all Claude Agent SDK capabilities (file operations, git, gh CLI)

## Architecture

### High-Level Overview

```mermaid
graph TB
    User[Slack User] -->|mentions @bot| Slack[Slack API]
    Slack -->|socket mode| Bot[Slack Coder Bot]
    Bot -->|setup request| MainAgent[Main Claude Agent]
    Bot -->|code request| RepoAgent[Repo-Specific Agent]

    MainAgent -->|gh repo view| GitHub[GitHub API]
    MainAgent -->|git clone| FS[File System]
    MainAgent -->|analyze| FS
    MainAgent -->|save prompt| SystemPrompt[System Prompt]

    RepoAgent -->|read prompt| SystemPrompt
    RepoAgent -->|code ops| RepoFS[Repository Files]
    RepoAgent -->|git/gh| GitHub

    Bot -->|progress updates| Slack
    RepoAgent -->|responses| Bot

    style Bot fill:#e1f5ff
    style MainAgent fill:#ffe1f5
    style RepoAgent fill:#f5ffe1
```

### Component Architecture

```mermaid
graph LR
    subgraph "Slack Bot (Message Broker)"
        SC[Slack Client]
        EH[Event Handler]
        MP[Message Processor]
        FH[Form Handler]
        PT[Progress Tracker]
    end

    subgraph "Agent Manager"
        MA[Main Agent]
        DM[DashMap<ChannelId, RepoAgent>]
    end

    subgraph "File System"
        R[~/.slack_coder/repos/channel_id/]
        S[~/.slack_coder/system/channel_id/system_prompt.md]
    end

    EH --> MP
    EH --> FH
    MP --> DM
    FH --> MA
    MA --> R
    MA --> S
    DM --> R
    PT -.->|TodoWrite hook| MA
    PT -.->|TodoWrite hook| DM

    style SC fill:#4a9eff
    style PT fill:#ff9e4a
    style MA fill:#9eff4a
    style DM fill:#ff4a9e
```

### Setup Flow

```mermaid
sequenceDiagram
    participant U as User
    participant S as Slack
    participant B as Bot
    participant M as Main Agent
    participant P as Progress Tracker
    participant F as File System

    U->>S: Invites @SlackCoderBot to #project
    S->>B: app_mention event
    B->>S: Show setup form
    U->>S: Submits "owner/repo"
    S->>B: Form submission
    B->>M: Create & setup_repository()
    M->>M: connect()

    M->>M: Use TodoWrite (validate repo)
    M-->>P: PostToolUse hook
    P->>S: Update progress: ⏳ Validating...

    M->>F: gh repo view owner/repo
    M->>M: Use TodoWrite (clone)
    M-->>P: PostToolUse hook
    P->>S: Update progress: ✅ Validated, ⏳ Cloning...

    M->>F: gh repo clone → ~/.slack_coder/repos/C123/
    M->>M: Use TodoWrite (analyze)
    M-->>P: PostToolUse hook
    P->>S: Update progress: ✅ Cloned, ⏳ Analyzing...

    M->>F: Read files, detect patterns
    M->>M: Use TodoWrite (generate prompt)
    M-->>P: PostToolUse hook
    P->>S: Update progress: ✅ Analyzed, ⏳ Generating...

    M->>F: Write ~/.slack_coder/system/C123/system_prompt.md
    M->>M: Use TodoWrite (complete)
    M-->>P: PostToolUse hook
    P->>S: Update progress: ✅ All done!

    M->>M: disconnect()
    B->>B: Create RepoAgent with system prompt
    B->>S: "Repository ready! 🎉"
```

### Message Processing Flow

```mermaid
sequenceDiagram
    participant U as User
    participant S as Slack
    participant B as Bot
    participant R as Repo Agent
    participant C as Claude
    participant F as Files

    U->>S: @SlackCoderBot add API endpoint
    S->>B: app_mention event
    B->>B: Check has_agent(channel)

    alt Agent exists
        B->>R: get_repo_agent().lock()
        B->>R: query("add API endpoint")
        R->>C: Send with system prompt

        R->>R: Use TodoWrite
        R-->>B: PostToolUse hook
        B->>S: Progress: ⏳ Reviewing code...

        C->>R: Read existing code
        R->>F: File operations

        R->>R: Use TodoWrite
        R-->>B: PostToolUse hook
        B->>S: Progress: ⏳ Generating code...

        C->>R: Generate code
        R->>F: Write new files

        R->>R: Use TodoWrite
        R-->>B: PostToolUse hook
        B->>S: Progress: ✅ Complete

        R->>B: Result message
        B->>S: "Created src/api/endpoint.rs..."
    else No agent
        B->>S: "Please setup repository first"
    end
```

## Quick Start

**New to this bot?** → [Quick Start Guide (15 minutes)](docs/QUICK_START.md)

**Need detailed Slack setup?** → [Complete Slack Setup Guide](docs/SLACK_SETUP.md)

## Setup

### Prerequisites

1. **Rust** (2024 edition)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **GitHub CLI** (`gh`)
   ```bash
   # macOS
   brew install gh

   # Linux
   sudo apt install gh

   # Authenticate
   gh auth login
   ```

3. **Git**
   ```bash
   git --version  # Should be installed
   ```

### Slack App Configuration

1. **Create a Slack App** at https://api.slack.com/apps
   - Click "Create New App" → "From scratch"
   - Name: "Slack Coder Bot"
   - Choose your workspace

2. **Configure OAuth & Permissions**
   - Navigate to "OAuth & Permissions"
   - Add Bot Token Scopes:
     - `app_mentions:read` - Read mentions
     - `channels:history` - Read channel messages
     - `channels:read` - List channels
     - `chat:write` - Send messages
     - `groups:history` - Read private channel messages
     - `groups:read` - List private channels
     - `im:history` - Read DMs
     - `im:read` - List DMs
     - `im:write` - Send DMs
   - Install App to Workspace
   - Copy **Bot User OAuth Token** (starts with `xoxb-`)

3. **Enable Socket Mode**
   - Navigate to "Socket Mode"
   - Enable Socket Mode
   - Create App-Level Token with `connections:write` scope
   - Copy **App-Level Token** (starts with `xapp-`)

4. **Subscribe to Events**
   - Navigate to "Event Subscriptions"
   - Enable Events
   - Subscribe to bot events:
     - `app_mention` - When bot is mentioned
     - `message.channels` - Channel messages
     - `message.groups` - Private channel messages
     - `message.im` - Direct messages

5. **Get Signing Secret**
   - Navigate to "Basic Information"
   - Copy **Signing Secret**

### Installation

1. **Clone the repository**
   ```bash
   git clone https://github.com/tyrchen/slack-coder
   cd slack-coder
   ```

2. **Configure environment**
   ```bash
   cp .env.example .env
   # Edit .env with your tokens
   ```

3. **Set environment variables** in `.env`:
   ```env
   # Slack Configuration
   SLACK_BOT_TOKEN=xoxb-your-bot-token-here
   SLACK_APP_TOKEN=xapp-your-app-token-here
   SLACK_SIGNING_SECRET=your-signing-secret-here

   # Claude Configuration
   CLAUDE_API_KEY=your-claude-api-key-here
   CLAUDE_MODEL=claude-sonnet-4
   CLAUDE_MAX_TOKENS=8192

   # Workspace Configuration
   WORKSPACE_BASE_PATH=~/.slack_coder
   MAX_REPO_SIZE_MB=1024
   CLEANUP_INTERVAL_SECS=3600

   # Agent Configuration
   MAIN_AGENT_PROMPT_PATH=specs/0003-system-prompt.md
   AGENT_TIMEOUT_SECS=1800
   MAX_CONCURRENT_REQUESTS=10

   # Logging
   RUST_LOG=info
   ```

4. **Build and run**
   ```bash
   cargo build --release
   cargo run --release
   ```

## Usage

### Initial Setup (Per Channel)

1. **Invite the bot** to a Slack channel:
   ```
   /invite @SlackCoderBot
   ```

2. **Provide repository** when prompted:
   ```
   tyrchen/rust-lib-template
   ```

3. **Wait for setup** (typically 1-2 minutes):
   ```
   Progress:
   ✅ Validate repository access
   ✅ Clone repository to workspace
   ⏳ Analyze codebase
   ⬜ Generate system prompt
   ⬜ Save system prompt to disk
   ```

4. **Start coding** when you see:
   ```
   ✅ Repository `tyrchen/rust-lib-template` is now ready!

   You can now ask me to generate code, write documentation,
   or use commands like `/help`.
   ```

### Daily Usage

**Generate code:**
```
@SlackCoderBot add a new API endpoint for user authentication
```

**Write documentation:**
```
@SlackCoderBot document the authentication module
```

**Refactor code:**
```
@SlackCoderBot refactor the user service to use async/await
```

**Fix bugs:**
```
@SlackCoderBot fix the null pointer error in line 42 of api/user.rs
```

**Use slash commands:**
```
@SlackCoderBot /help
@SlackCoderBot /context
```

### Features in Action

**Progress Tracking:**
All operations show real-time progress:
```
Progress: 2/4
Current: Generating code

✅ Review existing API structure
✅ Design user profile endpoint
⏳ Implement endpoint handler
⬜ Add tests
```

**Context-Aware Responses:**
The bot learns from your codebase and generates code that matches your:
- Coding style and conventions
- Architecture patterns
- Testing frameworks
- Documentation standards
- Naming conventions

**Thread Support:**
Continue conversations in threads for better organization.

## Directory Structure

After setup, your workspace will look like:

```
~/.slack_coder/
├── repos/
│   ├── C12345ABC/              # Channel ID
│   │   ├── .git/
│   │   ├── src/
│   │   └── ...                 # Full repository clone
│   └── C67890DEF/
│       └── ...
└── system/
    ├── C12345ABC/
    │   └── system_prompt.md    # Repository-specific instructions
    └── C67890DEF/
        └── system_prompt.md
```

## Development

### Running Tests

```bash
cargo test
```

### Linting

```bash
cargo clippy --all-targets --all-features
```

### Building for Production

```bash
cargo build --release
```

### Docker Deployment

```bash
docker build -t slack-coder .
docker run -d \
  --name slack-coder \
  --env-file .env \
  -v ~/.slack_coder:/root/.slack_coder \
  slack-coder
```

## Troubleshooting

### Bot doesn't respond

**Check Socket Mode connection:**
```bash
# Look for this in logs:
# "Event handler starting..."
# "Listening for Slack events..."
```

**Verify tokens:**
```bash
# Check SLACK_APP_TOKEN is valid
# Check SLACK_BOT_TOKEN is valid
```

### Repository setup fails

**Check GitHub authentication:**
```bash
gh auth status
# Should show: Logged in to github.com as <username>
```

**Check repository access:**
```bash
gh repo view owner/repo-name
# Should show repository details
```

**Check disk space:**
```bash
df -h ~/.slack_coder
# Ensure sufficient space for repository
```

### Agent not responding

**Check agent status:**
```bash
# Look for logs:
# "Agent restored for channel C12345"
# "Processing message from U123 in channel C12345"
```

**Check system prompt exists:**
```bash
ls -la ~/.slack_coder/system/C12345/system_prompt.md
cat ~/.slack_coder/system/C12345/system_prompt.md
```

**Restart the bot:**
```bash
# Kill and restart - agents will be restored on startup
```

## Configuration Reference

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SLACK_BOT_TOKEN` | ✅ | - | Bot OAuth token (xoxb-...) |
| `SLACK_APP_TOKEN` | ✅ | - | App-level token (xapp-...) |
| `SLACK_SIGNING_SECRET` | ✅ | - | Signing secret for verification |
| `CLAUDE_API_KEY` | ✅ | - | Claude API key |
| `CLAUDE_MODEL` | ❌ | claude-sonnet-4 | Claude model to use |
| `CLAUDE_MAX_TOKENS` | ❌ | 8192 | Max tokens per request |
| `WORKSPACE_BASE_PATH` | ❌ | ~/.slack_coder | Base directory for repos |
| `MAX_REPO_SIZE_MB` | ❌ | 1024 | Max repository size (MB) |
| `CLEANUP_INTERVAL_SECS` | ❌ | 3600 | Agent cleanup interval |
| `MAIN_AGENT_PROMPT_PATH` | ❌ | specs/0003-system-prompt.md | Main agent prompt |
| `AGENT_TIMEOUT_SECS` | ❌ | 1800 | Inactive agent timeout |
| `MAX_CONCURRENT_REQUESTS` | ❌ | 10 | Max concurrent requests |
| `RUST_LOG` | ❌ | info | Log level (trace, debug, info, warn, error) |

### Slack Permissions Required

**Bot Token Scopes:**
- `app_mentions:read`
- `channels:history`
- `channels:read`
- `chat:write`
- `groups:history`
- `groups:read`
- `im:history`
- `im:read`
- `im:write`

**App-Level Token Scopes:**
- `connections:write` (for Socket Mode)

## How It Works

### 1. Bot Initialization

```mermaid
sequenceDiagram
    participant B as Bot
    participant S as Slack API
    participant W as Workspace
    participant A as Agent Manager

    B->>B: Load config from .env
    B->>W: Create workspace dirs
    B->>S: Connect via Socket Mode
    B->>A: Create AgentManager
    B->>S: List all channels

    loop For each channel
        S-->>B: Channel C12345
        B->>W: Check if setup (repos/C12345 exists)
        alt Already setup
            W-->>B: Found
            B->>A: Create RepoAgent from disk
            B->>B: Add to agent pool
        else Not setup
            B->>B: Wait for user setup
        end
    end

    B->>S: Start listening for events
```

### 2. Repository Setup (Main Agent)

The main agent performs these steps:

1. **Validate** - Uses `gh repo view` to check accessibility
2. **Clone** - Uses `gh repo clone` to `~/.slack_coder/repos/{channel_id}/`
3. **Analyze** - Reads files to understand:
   - Languages and frameworks
   - Code conventions and patterns
   - Architecture and design
   - Testing approaches
   - Documentation style
4. **Generate Prompt** - Creates repository-specific instructions
5. **Save** - Writes to `~/.slack_coder/system/{channel_id}/system_prompt.md`

### 3. Code Generation (Repo Agent)

Each channel gets a dedicated agent that:

1. **Loads** system prompt with repository knowledge
2. **Sets working directory** to repository location
3. **Processes requests** with full context
4. **Performs operations** (read, write, git, gh)
5. **Maintains state** across conversation threads

### 4. Progress Tracking

Uses PostToolUse hook to intercept TodoWrite calls:

```rust
// When agent uses TodoWrite:
{
  "todos": [
    {"content": "Review code", "activeForm": "Reviewing code", "status": "completed"},
    {"content": "Generate endpoint", "activeForm": "Generating endpoint", "status": "in_progress"},
    {"content": "Add tests", "activeForm": "Adding tests", "status": "pending"}
  ]
}

// Hook automatically updates Slack:
Progress: 1/3
Current: Generating endpoint

✅ Review code
⏳ Generating endpoint
⬜ Add tests
```

## Project Structure

```
slack-coder/
├── Cargo.toml
├── README.md
├── .env.example
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Module exports
│   ├── error.rs             # Error types
│   ├── config/
│   │   ├── mod.rs
│   │   └── settings.rs      # Configuration loading
│   ├── slack/
│   │   ├── mod.rs
│   │   ├── client.rs        # Slack API wrapper
│   │   ├── events.rs        # Socket Mode event handler
│   │   ├── forms.rs         # Setup form handler
│   │   ├── messages.rs      # Message routing
│   │   ├── progress.rs      # Progress tracking
│   │   └── types.rs         # Slack types
│   ├── agent/
│   │   ├── mod.rs
│   │   ├── hooks.rs         # PostToolUse TodoWrite hook
│   │   ├── main_agent.rs    # Setup agent
│   │   ├── repo_agent.rs    # Repository agent
│   │   ├── manager.rs       # Agent lifecycle
│   │   └── types.rs         # Task, Plan types
│   └── storage/
│       ├── mod.rs
│       └── workspace.rs     # Path management
├── specs/
│   ├── 0001-slack-bot-spec.md
│   ├── 0002-slack-bot-design.md
│   ├── 0003-system-prompt.md
│   └── 0004-initial-plan.md
└── examples/
    └── agent.rs             # Simple agent example
```

## Advanced Usage

### Multiple Channels

Each channel maintains its own repository:

```
#project-alpha → tyrchen/project-alpha
#project-beta  → tyrchen/project-beta
#team-shared   → company/shared-lib
```

Agents are completely isolated - no cross-channel data leakage.

### Agent Cleanup

Inactive agents are automatically cleaned up after timeout (default: 30 minutes).

To manually trigger cleanup:
```rust
// Will be implemented via admin commands
```

### Custom System Prompts

You can manually edit system prompts:

```bash
# Edit the generated prompt
vim ~/.slack_coder/system/C12345/system_prompt.md

# Restart bot to reload (or wait for next agent creation)
```

## Contributing

Contributions welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Run clippy: `cargo clippy --all-targets --all-features`
6. Submit a pull request

## License

This project is distributed under the terms of MIT.

See [LICENSE](LICENSE.md) for details.

Copyright 2025 Tyr Chen

## Related Projects

- [claude-agent-sdk-rs](https://github.com/anthropics/claude-agent-sdk-rs) - Claude Agent SDK for Rust
- [slack-morphism](https://github.com/abdolence/slack-morphism-rust) - Slack API client for Rust

## Support

For issues and questions:
- GitHub Issues: https://github.com/tyrchen/slack-coder/issues
- Documentation: See `specs/` directory for detailed specifications
