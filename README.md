# Slack Coder Bot

A Slack bot that integrates with Claude AI to provide intelligent code generation and documentation assistance directly in your Slack channels. The bot analyzes your repository, learns your coding conventions, and helps you write code that matches your project's style.

## Features

- **Repository-Aware**: Analyzes your codebase to understand conventions, patterns, and architecture
- **Channel-Isolated**: Each Slack channel can work with a different repository
- **Real-Time Progress**: TodoWrite hook integration shows live progress updates
- **Context-Aware**: Maintains conversation context within threads
- **Full Claude SDK Support**: Access to all Claude Agent SDK capabilities (file operations, git, gh CLI)

## Architecture

### System Overview

```mermaid
graph TB
    User[Slack User] -->|mentions @bot| Slack[Slack API]
    Slack -->|Socket Mode WebSocket| Bot[Slack Coder Bot]
    Bot -->|setup request| MainAgent[Main Claude Agent]
    Bot -->|code/command request| RepoAgent[Repo-Specific Agent]

    MainAgent -->|gh repo view| GitHub[GitHub API]
    MainAgent -->|gh repo clone| FS[File System]
    MainAgent -->|analyze codebase| FS
    MainAgent -->|save| SystemPrompt[System Prompt .md]

    RepoAgent -->|read| SystemPrompt
    RepoAgent -->|code operations| RepoFS[Repository Files]
    RepoAgent -->|git/gh commands| GitHub

    Bot -->|progress updates| Slack
    RepoAgent -->|streaming responses| Bot
    MainAgent -->|progress updates| Bot

    style Bot fill:#e1f5ff
    style MainAgent fill:#ffe1f5
    style RepoAgent fill:#f5ffe1
```

### Component Architecture

```mermaid
graph TB
    subgraph "Slack Layer"
        SC[SlackClient<br/>API Wrapper]
        EH[EventHandler<br/>Socket Mode]
        MP[MessageProcessor<br/>Message Router]
        FH[FormHandler<br/>Setup Forms]
        CH[CommandHandler<br/>/help, /new-session]
        PT[ProgressTracker<br/>TodoWrite Hook]
        MD[markdown_to_slack<br/>Formatter]
    end

    subgraph "Agent Management Layer"
        AM[AgentManager<br/>Lifecycle]
        MA[MainAgent<br/>Repository Setup]
        RA[RepoAgent<br/>Code Generation]
        DM[DashMap&lt;ChannelId, RepoAgent&gt;<br/>Agent Pool]
    end

    subgraph "Storage Layer"
        WS[Workspace<br/>Path Manager]
        RP["repos/channel_id/<br/>Repository Clone"]
        SP["system/channel_id/<br/>system_prompt.md"]
    end

    subgraph "Session Management"
        SM[SessionId Generator<br/>UUID-based]
        SS[Session State<br/>Per RepoAgent]
    end

    subgraph "External Services"
        Claude[Claude API<br/>claude-agent-sdk-rs]
        GitHub[GitHub<br/>gh CLI]
        Git[Git<br/>Repository Ops]
    end

    EH --> MP
    EH --> FH
    MP --> CH
    MP --> AM
    FH --> AM
    AM --> MA
    AM --> RA
    AM --> DM
    MA --> WS
    RA --> WS
    RA --> SM
    WS --> RP
    WS --> SP
    PT -.->|hook| MA
    PT -.->|hook| RA
    PT --> SC
    MP --> MD
    MD --> SC
    MA --> Claude
    RA --> Claude
    MA --> GitHub
    RA --> GitHub
    RA --> Git

    style SC fill:#4a9eff
    style PT fill:#ff9e4a
    style MA fill:#9eff4a
    style DM fill:#ff4a9e
    style Claude fill:#ffeb3b
    style SM fill:#e1bee7
```

### Data Flow Architecture

```mermaid
graph LR
    subgraph "Input Flow"
        U[User Message] --> SE[Slack Event]
        SE --> DD[Dedup Cache]
        DD --> EM[Event Matcher]
    end

    subgraph "Processing Flow"
        EM -->|command| CMD[Command Router]
        EM -->|repo pattern| SETUP[Setup Flow]
        EM -->|text| QUERY[Query Flow]

        CMD --> HC[Help Command]
        CMD --> NSC[New Session Command]

        SETUP --> VA[Validate Repo]
        VA --> CL[Clone Repo]
        CL --> AN[Analyze Code]
        AN --> GP[Generate Prompt]
        GP --> CR[Create Agent]

        QUERY --> GA[Get Agent]
        GA --> SQ[Send Query]
        SQ --> SR[Stream Response]
    end

    subgraph "Output Flow"
        HC --> FMT[Format Markdown]
        NSC --> FMT
        CR --> FMT
        SR --> FMT
        FMT --> SPLIT[Split Chunks]
        SPLIT --> SLACK[Send to Slack]
    end

    style EM fill:#b3e5fc
    style SETUP fill:#ffccbc
    style QUERY fill:#c8e6c9
    style FMT fill:#f8bbd0
```

### Repository Setup Flow (Detailed)

```mermaid
sequenceDiagram
    participant U as User
    participant S as Slack
    participant EH as EventHandler
    participant FH as FormHandler
    participant AM as AgentManager
    participant MA as MainAgent
    participant PT as ProgressTracker
    participant FS as File System
    participant GH as GitHub (gh CLI)
    participant C as Claude API

    U->>S: Invites @SlackCoderBot to #project
    S->>EH: app_mention event (channel_join)
    EH->>FH: show_repo_setup_form()
    FH->>S: Display welcome message + instructions

    U->>S: Mentions bot with "owner/repo"
    S->>EH: app_mention event
    EH->>EH: Parse repo pattern (owner/repo)
    EH->>FH: handle_repo_setup(channel, "owner/repo")

    FH->>AM: setup_channel(channel, repo_name)
    AM->>AM: Create MainAgent with hooks
    AM->>MA: new(settings, workspace, tracker, channel)
    MA->>MA: Load main-agent-system-prompt.md
    MA->>MA: Create TodoWrite hooks
    MA-->>AM: MainAgent instance

    AM->>MA: connect()
    MA->>C: Connect to Claude API
    C-->>MA: Connection established

    AM->>MA: setup_repository(repo_name, channel)
    MA->>C: Send setup prompt with tasks

    Note over MA,C: Claude executes tasks with TodoWrite

    MA->>MA: TodoWrite: Validate repository
    MA->>PT: PostToolUse hook triggered
    PT->>S: Update: ⏳ Validating repository...

    MA->>GH: gh repo view owner/repo
    GH-->>MA: Repository metadata

    MA->>MA: TodoWrite: Clone repository
    MA->>PT: PostToolUse hook
    PT->>S: Update: ✅ Validated, ⏳ Cloning...

    MA->>GH: gh repo clone owner/repo
    GH->>FS: Clone to ~/.slack_coder/repos/C123/
    FS-->>MA: Repository cloned

    MA->>MA: TodoWrite: Analyze codebase
    MA->>PT: PostToolUse hook
    PT->>S: Update: ✅ Cloned, ⏳ Analyzing...

    MA->>FS: Read package.json, Cargo.toml, etc.
    MA->>FS: Read source files
    FS-->>MA: File contents
    MA->>MA: Detect patterns, conventions

    MA->>MA: TodoWrite: Generate system prompt
    MA->>PT: PostToolUse hook
    PT->>S: Update: ✅ Analyzed, ⏳ Generating...

    MA->>MA: Create repository-specific instructions
    MA->>FS: Write ~/.slack_coder/system/C123/system_prompt.md
    FS-->>MA: File written

    MA->>MA: TodoWrite: Complete
    MA->>PT: PostToolUse hook
    PT->>S: Update: ✅ All done!

    MA->>C: Disconnect
    C-->>MA: Disconnected
    MA-->>AM: Setup complete

    AM->>AM: create_repo_agent(channel)
    AM->>AM: Create RepoAgent with system prompt
    AM->>AM: Store in DashMap<ChannelId, RepoAgent>
    AM->>S: "🤖 Agent Ready - Session ID: session-C123-..."

    Note over S: Channel is now ready for queries
```

### Message Processing Flow (Code Generation)

```mermaid
sequenceDiagram
    participant U as User
    participant S as Slack
    participant EH as EventHandler
    participant MP as MessageProcessor
    participant AM as AgentManager
    participant RA as RepoAgent
    participant PT as ProgressTracker
    participant C as Claude API
    participant FS as File System
    participant MD as Markdown Formatter

    U->>S: @SlackCoderBot add user authentication API
    S->>EH: app_mention event
    EH->>EH: Check dedup cache
    EH->>EH: Strip bot mention

    EH->>MP: process_message(SlackMessage)
    MP->>MP: Check if command (starts with /)
    MP->>AM: has_agent(channel)?
    AM-->>MP: true

    MP->>AM: get_repo_agent(channel)
    AM-->>MP: Arc<Mutex<RepoAgent>>

    MP->>RA: lock().await
    Note over MP,RA: Exclusive lock acquired

    MP->>RA: query("add user authentication API")
    RA->>RA: Get current session_id
    RA->>C: query_with_session(message, session_id)
    RA->>RA: Update last_activity timestamp

    Note over RA,C: Claude processes with TodoWrite hooks

    RA->>RA: TodoWrite: Planning authentication
    RA->>PT: PostToolUse hook
    PT->>S: Update: ⏳ Planning authentication...

    RA->>FS: Read existing auth code
    FS-->>RA: Current implementation

    RA->>RA: TodoWrite: Generating auth module
    RA->>PT: PostToolUse hook
    PT->>S: Update: ⏳ Generating auth module...

    RA->>FS: Write src/auth/mod.rs
    RA->>FS: Write src/auth/jwt.rs
    FS-->>RA: Files created

    RA->>RA: TodoWrite: Adding tests
    RA->>PT: PostToolUse hook
    PT->>S: Update: ⏳ Adding tests...

    RA->>FS: Write src/auth/tests.rs
    FS-->>RA: Tests created

    RA->>RA: TodoWrite: Complete
    RA->>PT: PostToolUse hook
    PT->>S: Update: ✅ Complete!

    C-->>RA: Stream final result
    RA-->>MP: Result message

    MP->>MD: markdown_to_slack(result)
    MD-->>MP: Slack-formatted text

    MP->>MP: Check message size (40KB limit)
    alt Message > 40KB
        MP->>S: Send in chunks with "(continued...)"
    else Normal size
        MP->>S: Send formatted message
    end

    MP->>RA: unlock()
    Note over MP,RA: Lock released

    S-->>U: Display response with code
```

### Session Management Flow

```mermaid
sequenceDiagram
    participant U as User
    participant S as Slack
    participant MP as MessageProcessor
    participant CH as CommandHandler
    participant RA as RepoAgent
    participant C as Claude API

    Note over RA: Initial session created on agent startup
    RA->>RA: session_id = generate_session_id(channel)
    Note over RA: Format: session-C123-1234567890-a3f9b2

    U->>S: @bot /new-session
    S->>MP: Process message
    MP->>CH: handle_command("/new-session")
    CH->>RA: start_new_session()

    RA->>RA: Generate new session_id
    RA->>RA: Update current_session_id (RwLock)
    RA->>C: Subsequent queries use new session_id
    Note over RA,C: Previous conversation context is cleared

    RA-->>CH: new_session_id
    CH->>S: "New Session Started\nSession ID: session-C123-..."

    Note over U,C: All future messages in this channel<br/>use the new session ID
```

### TodoWrite Hook Processing Flow

```mermaid
sequenceDiagram
    participant C as Claude API
    participant RA as RepoAgent/MainAgent
    participant H as TodoWrite Hook
    participant P as Plan (Arc<Mutex>)
    participant PT as ProgressTracker
    participant S as Slack

    C->>RA: Execute tool: TodoWrite
    Note over C,RA: Tool input contains todos array

    RA->>H: PostToolUse hook triggered
    H->>H: Parse tool_input as Plan

    H->>P: lock().await
    H->>P: update(new_plan)
    Note over P: Merges new tasks with timing data

    P->>P: Track task start times
    P->>P: Calculate task durations
    P-->>H: Updated plan with timing

    H->>PT: update_progress(channel, plan)

    PT->>PT: Format progress message
    Note over PT: Progress: 2/5<br/>Current: Generating code<br/>✅ Planning (2.3s)<br/>✅ Reading files (1.1s)<br/>⏳ Generating code<br/>⬜ Adding tests<br/>⬜ Documentation

    PT->>S: Update or send new message
    alt Progress message exists
        PT->>S: Update existing message
    else No progress message
        PT->>S: Send new progress message
    end

    S-->>PT: Message updated
    H-->>RA: Hook processing complete
```

## Quick Start

**New to this bot?** → [Quick Start Guide (15 minutes)](docs/QUICK_START.md)

**Need detailed Slack setup?** → [Complete Slack Setup Guide](docs/SLACK_SETUP.md)

**Bot not responding?** → [Debugging Guide](docs/DEBUGGING.md)

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

## Module Architecture

```mermaid
graph TB
    subgraph "Application Entry"
        MAIN[main.rs<br/>Bot Initialization]
        LIB[lib.rs<br/>Module Exports]
    end

    subgraph "Configuration Module"
        CONF[config/settings.rs<br/>Environment Variables<br/>Settings Struct]
    end

    subgraph "Error Handling"
        ERR[error.rs<br/>SlackCoderError<br/>Result Type]
    end

    subgraph "Slack Module"
        CLIENT[client.rs<br/>SlackClient<br/>API Wrapper]
        EVENTS[events.rs<br/>EventHandler<br/>Socket Mode Listener]
        MSGS[messages.rs<br/>MessageProcessor<br/>Query Router]
        FORMS[forms.rs<br/>FormHandler<br/>Setup Flow]
        CMDS[commands.rs<br/>CommandHandler<br/>/help, /new-session]
        PROG[progress.rs<br/>ProgressTracker<br/>TodoWrite Display]
        MDCONV[markdown.rs<br/>markdown_to_slack()<br/>Format Converter]
        TYPES[types.rs<br/>ChannelId, UserId<br/>MessageTs, ThreadTs]
    end

    subgraph "Agent Module"
        MGR[manager.rs<br/>AgentManager<br/>Lifecycle & Pool]
        MAIN_AG[main_agent.rs<br/>MainAgent<br/>Repository Setup]
        REPO_AG[repo_agent.rs<br/>RepoAgent<br/>Code Generation]
        HOOKS[hooks.rs<br/>create_todo_hooks()<br/>PostToolUse Handler]
        AG_TYPES[types.rs<br/>Plan, Task<br/>TaskStatus]
    end

    subgraph "Storage Module"
        WS[workspace.rs<br/>Workspace<br/>Path Manager]
    end

    subgraph "Session Module"
        SESS[session.rs<br/>SessionId<br/>generate_session_id()]
    end

    subgraph "External Dependencies"
        CLAUDE[claude-agent-sdk-rs<br/>ClaudeClient<br/>ClaudeAgentOptions]
        SLACK_M[slack-morphism<br/>Socket Mode<br/>Events API]
        DASHMAP[dashmap<br/>DashMap<br/>Concurrent HashMap]
    end

    MAIN --> CONF
    MAIN --> CLIENT
    MAIN --> EVENTS
    MAIN --> MGR
    MAIN --> WS
    MAIN --> PROG

    EVENTS --> MSGS
    EVENTS --> FORMS
    EVENTS --> TYPES

    MSGS --> CMDS
    MSGS --> MGR
    MSGS --> MDCONV

    FORMS --> MGR

    MGR --> MAIN_AG
    MGR --> REPO_AG
    MGR --> DASHMAP

    MAIN_AG --> HOOKS
    MAIN_AG --> CLAUDE
    MAIN_AG --> WS

    REPO_AG --> HOOKS
    REPO_AG --> CLAUDE
    REPO_AG --> WS
    REPO_AG --> SESS

    HOOKS --> AG_TYPES
    HOOKS --> PROG

    PROG --> CLIENT

    CLIENT --> SLACK_M
    EVENTS --> SLACK_M

    style MAIN fill:#e1f5ff
    style MGR fill:#ffe1f5
    style CLAUDE fill:#ffeb3b
    style SLACK_M fill:#4a9eff
```

## Project Structure

```
slack-coder/
├── Cargo.toml                      # Project dependencies and metadata
├── README.md                       # This file
├── README_zh.md                    # Chinese version
├── .env.example                    # Environment variables template
│
├── src/
│   ├── main.rs                     # Application entry point
│   │                               # - Initialize tracing/logging
│   │                               # - Load configuration
│   │                               # - Create workspace, SlackClient
│   │                               # - Start EventHandler
│   │
│   ├── lib.rs                      # Public module exports
│   ├── error.rs                    # Error types (SlackCoderError, Result)
│   │
│   ├── config/
│   │   ├── mod.rs
│   │   └── settings.rs             # Configuration loading from .env
│   │                               # - SlackConfig, ClaudeConfig
│   │                               # - WorkspaceConfig, AgentConfig
│   │
│   ├── session.rs                  # Session ID generation
│   │                               # - SessionId type
│   │                               # - generate_session_id()
│   │
│   ├── slack/                      # Slack integration layer
│   │   ├── mod.rs
│   │   ├── client.rs               # SlackClient - HTTP API wrapper
│   │   │                           # - send_message(), list_channels()
│   │   │                           # - update_message()
│   │   │
│   │   ├── events.rs               # EventHandler - Socket Mode listener
│   │   │                           # - handle_push_event()
│   │   │                           # - Event deduplication
│   │   │                           # - Route to FormHandler/MessageProcessor
│   │   │
│   │   ├── forms.rs                # FormHandler - Repository setup
│   │   │                           # - show_repo_setup_form()
│   │   │                           # - handle_repo_setup()
│   │   │
│   │   ├── messages.rs             # MessageProcessor - Message routing
│   │   │                           # - process_message()
│   │   │                           # - forward_to_agent()
│   │   │                           # - Stream and format responses
│   │   │
│   │   ├── commands.rs             # CommandHandler - Slash commands
│   │   │                           # - /help, /new-session
│   │   │
│   │   ├── progress.rs             # ProgressTracker - TodoWrite hook display
│   │   │                           # - update_progress()
│   │   │                           # - Format task progress messages
│   │   │
│   │   ├── markdown.rs             # Markdown to Slack mrkdwn converter
│   │   │                           # - markdown_to_slack()
│   │   │
│   │   └── types.rs                # Slack domain types
│   │                               # - ChannelId, UserId, MessageTs, ThreadTs
│   │
│   ├── agent/                      # Claude agent management
│   │   ├── mod.rs
│   │   ├── manager.rs              # AgentManager - Lifecycle management
│   │   │                           # - setup_channel()
│   │   │                           # - get_repo_agent()
│   │   │                           # - DashMap<ChannelId, RepoAgent>
│   │   │
│   │   ├── main_agent.rs           # MainAgent - Repository setup
│   │   │                           # - setup_repository()
│   │   │                           # - Validate, clone, analyze, generate prompt
│   │   │
│   │   ├── repo_agent.rs           # RepoAgent - Code generation
│   │   │                           # - query(), receive_response()
│   │   │                           # - Session management
│   │   │                           # - Loads repo-specific system prompt
│   │   │
│   │   ├── hooks.rs                # TodoWrite hook implementation
│   │   │                           # - create_todo_hooks()
│   │   │                           # - PostToolUse handler
│   │   │                           # - Update Plan and ProgressTracker
│   │   │
│   │   └── types.rs                # Agent domain types
│   │                               # - Plan, Task, TaskStatus
│   │                               # - Timing tracking
│   │
│   └── storage/
│       ├── mod.rs
│       └── workspace.rs            # Workspace - File system paths
│                                   # - repo_path(), system_prompt_path()
│                                   # - load_system_prompt()
│
├── prompts/
│   ├── main-agent-system-prompt.md    # MainAgent instructions
│   └── repo-agent-workflow.md         # RepoAgent workflow instructions
│
├── specs/                          # Technical specifications
│   ├── README.md
│   ├── 0001-slack-bot-spec.md
│   ├── 0002-slack-bot-design.md
│   ├── 0003-system-prompt.md
│   ├── 0004-initial-plan.md
│   ├── 0005-slack-new-session-command.md
│   └── instructions.md
│
├── docs/                           # User documentation
│   ├── QUICK_START.md
│   ├── SLACK_SETUP.md
│   └── DEBUGGING.md
│
├── examples/
│   └── agent.rs                    # Simple Claude agent example
│
└── vendors/                        # Vendored dependencies
    ├── claude-agent-sdk-rs/
    └── slack-morphism-rust/
```

### Key Files Reference

| File | Purpose | Key Exports |
|------|---------|-------------|
| `src/main.rs` | Application entry point | `main()` |
| `src/slack/events.rs` | Socket Mode event handling | `EventHandler`, `handle_push_event()` |
| `src/slack/messages.rs` | Message processing | `MessageProcessor`, `process_message()` |
| `src/agent/manager.rs` | Agent lifecycle | `AgentManager`, `setup_channel()` |
| `src/agent/repo_agent.rs` | Code generation agent | `RepoAgent`, `query()`, `start_new_session()` |
| `src/agent/hooks.rs` | TodoWrite hook | `create_todo_hooks()` |
| `src/slack/progress.rs` | Progress display | `ProgressTracker`, `update_progress()` |
| `src/storage/workspace.rs` | File paths | `Workspace`, path helpers |
| `src/session.rs` | Session IDs | `SessionId`, `generate_session_id()` |

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
