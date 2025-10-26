# Slack Coder Bot - Design and Implementation Plan

## 1. Architecture Overview

### 1.1 Design Philosophy

The Slack Coder Bot is designed as a **thin message broker** between Slack and Claude agents:

- **Bot's Role**: Slack API communication, message routing, progress visualization
- **Agent's Role**: All heavy lifting (git, gh, file operations, code generation)
- **Separation**: Bot never performs repository operations; agents do everything

### 1.2 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Slack API                            │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      │ Events & User Messages
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                 Slack Bot (Message Broker)                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ Event Handler│  │ Msg Processor│  │ Form Handler │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
│  ┌──────────────────────────────────────────────────┐     │
│  │  PostToolUse Hook (TodoWrite Progress Tracking)  │     │
│  └──────────────────────────────────────────────────┘     │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      │ Forward messages, extract todos
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                    Agent Manager                             │
│  ┌──────────────────────────────────────────────────┐       │
│  │  Main Agent (singleton, setup only)              │       │
│  │  DashMap<ChannelId, RepoAgent> (long-lived)      │       │
│  └──────────────────────────────────────────────────┘       │
└─────┬────────────────────────────────┬──────────────────────┘
      │                                │
      │ Setup operations               │ Coding operations
      ▼                                ▼
┌──────────────────┐         ┌──────────────────────────┐
│  Main Claude     │         │  Repo-Specific Claude    │
│     Agent        │         │       Agents             │
│  (one instance)  │         │   (one per channel)      │
│                  │         │                          │
│ Uses gh & git    │         │ Uses gh & git            │
│ to validate,     │         │ to generate code,        │
│ clone, analyze,  │         │ commit, push, create     │
│ and generate     │         │ PRs, run tests, etc.     │
│ system prompt    │         │                          │
└──────────────────┘         └──────────────────────────┘
      │                                │
      │ All file ops                   │ All file ops
      ▼                                ▼
┌─────────────────────────────────────────────────────────────┐
│                    File System                               │
│  ~/.slack_coder/                                             │
│    ├── repos/                                                │
│    │   └── {channel_id}/      # Per-channel repo            │
│    │       └── [git repo]                                    │
│    └── system/                                               │
│        └── {channel_id}/                                     │
│            └── system_prompt.md                              │
└─────────────────────────────────────────────────────────────┘
```

### 1.3 Component Responsibilities

**Slack Bot Service**

- Listen to Slack events (mentions, threads, DMs)
- Display interactive forms for repository setup
- Forward user messages to appropriate agents
- Stream agent responses back to Slack
- Extract and display TodoWrite progress updates via PostToolUse hook
- Format code blocks and markdown for Slack

**Agent Manager**

- Maintain one main agent instance for setup operations
- Maintain DashMap of channel-specific repository agents
- On startup, scan channels and restore agents from disk
- Create new repo agents after successful setup
- Handle agent lifecycle and cleanup

**Main Claude Agent** (One-time use per channel)

- Validate repository using `gh repo view`
- Clone repository using git to `~/.slack_coder/repos/{channel_id}/`
- Analyze codebase comprehensively
- Generate system prompt for repository
- Save system prompt to `~/.slack_coder/system/{channel_id}/system_prompt.md`

**Repository-Specific Agents** (Long-lived, one per channel)

- Initialize with system prompt from disk
- Working directory: `~/.slack_coder/repos/{channel_id}/`
- Handle all coding operations (generate, modify, delete files)
- Handle all git operations (branch, commit, push, PR)
- Handle all slash commands from Claude Agent SDK
- Maintain conversation context within threads

**File System Storage**

- Channel-based directory structure
- Isolated repositories per channel
- Persistent system prompts
- No shared state between channels

## 2. Project Structure

```
slack-coder/
├── Cargo.toml
├── src/
│   ├── lib.rs                      # Library exports
│   ├── main.rs                     # Application entry point
│   │
│   ├── config/
│   │   ├── mod.rs                  # Configuration module
│   │   └── settings.rs             # Settings struct and loading
│   │
│   ├── slack/
│   │   ├── mod.rs                  # Slack module exports
│   │   ├── client.rs               # Slack API client wrapper
│   │   ├── events.rs               # Event handling
│   │   ├── messages.rs             # Message formatting and sending
│   │   ├── forms.rs                # Interactive form handling
│   │   ├── progress.rs             # Progress tracking and display
│   │   └── types.rs                # Slack-specific types
│   │
│   ├── agent/
│   │   ├── mod.rs                  # Agent module exports
│   │   ├── manager.rs              # Agent lifecycle management
│   │   ├── main_agent.rs           # Main Claude agent wrapper
│   │   ├── repo_agent.rs           # Repository-specific agent wrapper
│   │   ├── hooks.rs                # PostToolUse hook implementation
│   │   └── types.rs                # Agent-specific types
│   │
│   ├── storage/
│   │   ├── mod.rs                  # Storage module exports
│   │   ├── workspace.rs            # Workspace path management
│   │   └── channel_config.rs       # Channel configuration persistence
│   │
│   └── error.rs                    # Error types
│
├── specs/
│   ├── 0001-slack-bot-spec.md
│   ├── 0002-slack-bot-design.md
│   └── 0003-system-prompt.md
│
└── tests/
    ├── integration/
    │   ├── mod.rs
    │   └── slack_bot_test.rs
    └── fixtures/
        └── sample_repo/
```

**Note**: Removed `repo/` module (validator, cloner, analyzer) since all repository
operations are now handled by Claude agents, not the bot application.

## 3. Dependencies

### 3.1 Core Dependencies

```toml
[dependencies]
# Async runtime
tokio = { version = "1.47", features = ["rt-multi-thread", "macros", "fs", "process", "sync"] }

# Slack SDK
slack-morphism = "2.0"
slack-morphism-hyper = "2.0"

# HTTP client
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }

# Claude Agent SDK
claude-agent-sdk-rs = "0.2.0"

# Concurrent data structures
dashmap = "6.0"
arc-swap = "1.7"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
anyhow = "1.0"
thiserror = "2.0"

# Logging and tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Configuration
config = "0.14"
dotenvy = "0.15"

# File system operations
walkdir = "2.4"

# Git operations (via process)
# Uses external 'gh' and 'git' CLI tools

# Utilities
chrono = "0.4"
uuid = { version = "1.0", features = ["v4", "serde"] }
regex = "1.10"
```

### 3.2 Development Dependencies

```toml
[dev-dependencies]
tokio-test = "0.4"
mockito = "1.0"
temp-env = "0.3"
tempfile = "3.0"
```

## 4. Core Data Structures

### 4.1 Configuration

```rust
// src/config/settings.rs

pub struct Settings {
    pub slack: SlackConfig,
    pub claude: ClaudeConfig,
    pub workspace: WorkspaceConfig,
    pub agent: AgentConfig,
}

pub struct SlackConfig {
    pub bot_token: String,
    pub app_token: String,
    pub signing_secret: String,
}

pub struct ClaudeConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: usize,
}

pub struct WorkspaceConfig {
    pub base_path: PathBuf,
    pub max_repo_size_mb: u64,
    pub cleanup_interval_secs: u64,
}

pub struct AgentConfig {
    pub main_agent_prompt_path: PathBuf,
    pub agent_timeout_secs: u64,
    pub max_concurrent_requests: usize,
}

pub fn load_settings() -> Result<Settings>;
```

### 4.2 Slack Types

```rust
// src/slack/types.rs

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelId(pub String);

#[derive(Debug, Clone)]
pub struct UserId(pub String);

#[derive(Debug, Clone)]
pub struct ThreadTs(pub String);

#[derive(Debug, Clone)]
pub struct MessageTs(pub String);

pub struct SlackMessage {
    pub channel: ChannelId,
    pub user: UserId,
    pub text: String,
    pub thread_ts: Option<ThreadTs>,
    pub ts: MessageTs,
}

pub struct RepoSetupForm {
    pub channel_id: ChannelId,
    pub repo_name: String,
}
```

### 4.3 Agent Types

```rust
// src/agent/types.rs

use claude_agent_sdk_rs::{ClaudeClient, ClaudeAgentOptions, Hooks, HookInput, HookContext, HookJsonOutput};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

pub struct AgentManager {
    main_agent: Arc<MainAgent>,
    repo_agents: Arc<DashMap<ChannelId, RepoAgent>>,
    workspace: Arc<Workspace>,
    settings: Arc<Settings>,
    progress_tracker: Arc<ProgressTracker>,
}

pub struct MainAgent {
    client: ClaudeClient,
    plan: Arc<Mutex<Plan>>,
}

pub struct RepoAgent {
    client: ClaudeClient,
    plan: Arc<Mutex<Plan>>,
    channel_id: ChannelId,
    last_activity: Arc<RwLock<Instant>>,
}

pub struct AgentContext {
    pub channel_id: ChannelId,
    pub thread_ts: Option<ThreadTs>,
    pub user_id: UserId,
}

// PostToolUse hook payload for TodoWrite (matches claude-agent-sdk-rs format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub content: String,
    #[serde(rename = "activeForm")]
    pub active_form: String,
    pub status: TaskStatus,
    #[serde(skip)]
    pub start_time: Option<Instant>,
    #[serde(skip)]
    pub completion_time: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Plan {
    pub todos: Vec<Task>,
}

impl Plan {
    pub fn new() -> Self;
    pub fn update(&mut self, new_plan: Plan);
    pub fn get_current_task(&self) -> Option<&Task>;
    pub fn get_completed_count(&self) -> usize;
    pub fn get_total_count(&self) -> usize;
    pub fn is_complete(&self) -> bool;
}
```

### 4.4 Storage Types

```rust
// src/storage/workspace.rs

pub struct Workspace {
    base_path: PathBuf,
}

impl Workspace {
    pub fn new(base_path: PathBuf) -> Self;

    /// Returns path to channel's repository: ~/.slack_coder/repos/{channel_id}/
    pub fn repo_path(&self, channel_id: &ChannelId) -> PathBuf;

    /// Returns path to channel's system prompt: ~/.slack_coder/system/{channel_id}/system_prompt.md
    pub fn system_prompt_path(&self, channel_id: &ChannelId) -> PathBuf;

    /// Check if channel has an existing repository setup
    pub async fn is_channel_setup(&self, channel_id: &ChannelId) -> bool;

    /// Load system prompt from disk
    pub async fn load_system_prompt(&self, channel_id: &ChannelId) -> Result<String>;
}

// src/storage/channel_config.rs

#[derive(Serialize, Deserialize)]
pub struct ChannelConfig {
    pub channel_id: ChannelId,
    pub repo_name: String,
    pub setup_at: DateTime<Utc>,
}

impl ChannelConfig {
    pub async fn save(&self, workspace: &Workspace) -> Result<()>;
    pub async fn load(workspace: &Workspace, channel_id: &ChannelId) -> Result<Option<Self>>;
}
```

### 4.5 Error Types

```rust
// src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum SlackCoderError {
    #[error("Slack API error: {0}")]
    SlackApi(#[from] slack_morphism::errors::SlackClientError),

    #[error("Claude agent error: {0}")]
    ClaudeAgent(String),

    #[error("Agent not found for channel: {0}")]
    AgentNotFound(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Channel not setup: {0}")]
    ChannelNotSetup(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, SlackCoderError>;
```

## 5. Core Module Designs

### 5.1 Main Entry Point

```rust
// src/main.rs

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Load configuration
    let settings = config::load_settings()?;

    // Create workspace
    let workspace = Workspace::new(settings.workspace.base_path.clone());

    // Create agent manager (sets up main agent + scans for existing channels)
    let agent_manager = AgentManager::new(settings.clone(), workspace).await?;

    // Create Slack client
    let slack_client = SlackClient::new(settings.slack.clone())?;

    // Scan Slack channels and restore agents
    agent_manager.scan_and_restore_channels(&slack_client).await?;

    // Start event handler
    let event_handler = EventHandler::new(
        slack_client.clone(),
        agent_manager.clone(),
    );

    // Start listening
    event_handler.start().await?;

    Ok(())
}
```

### 5.2 Slack Client

```rust
// src/slack/client.rs

pub struct SlackClient {
    client: slack_morphism::SlackClient,
    bot_token: String,
}

impl SlackClient {
    pub fn new(config: SlackConfig) -> Result<Self>;

    pub async fn send_message(
        &self,
        channel: &ChannelId,
        text: &str,
        thread_ts: Option<&ThreadTs>,
    ) -> Result<MessageTs>;

    pub async fn send_form(
        &self,
        channel: &ChannelId,
        form: &RepoSetupForm,
    ) -> Result<()>;

    pub async fn update_message(
        &self,
        channel: &ChannelId,
        ts: &MessageTs,
        text: &str,
    ) -> Result<()>;

    pub async fn send_code_block(
        &self,
        channel: &ChannelId,
        code: &str,
        language: &str,
        thread_ts: Option<&ThreadTs>,
    ) -> Result<MessageTs>;
}
```

### 5.3 Event Handler

```rust
// src/slack/events.rs

pub struct EventHandler {
    slack_client: Arc<SlackClient>,
    agent_manager: Arc<AgentManager>,
    message_processor: MessageProcessor,
}

impl EventHandler {
    pub fn new(
        slack_client: Arc<SlackClient>,
        agent_manager: Arc<AgentManager>,
    ) -> Self;

    pub async fn start(self) -> Result<()>;

    async fn handle_event(&self, event: SlackEvent) -> Result<()>;

    async fn handle_app_mention(
        &self,
        channel: ChannelId,
        user: UserId,
        text: String,
        thread_ts: Option<ThreadTs>,
    ) -> Result<()>;

    async fn handle_message(
        &self,
        message: SlackMessage,
    ) -> Result<()>;

    async fn handle_app_home_opened(
        &self,
        user: UserId,
    ) -> Result<()>;
}
```

### 5.4 Message Processor

```rust
// src/slack/messages.rs

pub struct MessageProcessor {
    slack_client: Arc<SlackClient>,
    agent_manager: Arc<AgentManager>,
}

impl MessageProcessor {
    pub fn new(
        slack_client: Arc<SlackClient>,
        agent_manager: Arc<AgentManager>,
    ) -> Self;

    /// Process user message - forward to appropriate agent
    pub async fn process_message(
        &self,
        message: SlackMessage,
    ) -> Result<()>;

    /// Forward message to repository agent and stream response
    async fn forward_to_agent(
        &self,
        text: &str,
        context: AgentContext,
    ) -> Result<()>;
}
```

### 5.5 Form Handler

```rust
// src/slack/forms.rs

pub struct FormHandler {
    slack_client: Arc<SlackClient>,
    agent_manager: Arc<AgentManager>,
}

impl FormHandler {
    pub fn new(
        slack_client: Arc<SlackClient>,
        agent_manager: Arc<AgentManager>,
    ) -> Self;

    pub async fn show_repo_setup_form(
        &self,
        channel: &ChannelId,
    ) -> Result<()>;

    /// Handle form submission - triggers main agent to setup repository
    pub async fn handle_form_submission(
        &self,
        form_data: RepoSetupForm,
    ) -> Result<()>;

    fn validate_repo_name_format(name: &str) -> Result<(String, String)>;
}
```

### 5.6 Progress Tracker

```rust
// src/slack/progress.rs

pub struct ProgressTracker {
    slack_client: Arc<SlackClient>,
    active_progress: Arc<DashMap<ChannelId, MessageTs>>,
}

impl ProgressTracker {
    pub fn new(slack_client: Arc<SlackClient>) -> Self;

    /// Display initial progress message
    pub async fn start_progress(
        &self,
        channel: &ChannelId,
        initial_plan: &Plan,
    ) -> Result<()>;

    /// Update progress message with new plan state
    pub async fn update_progress(
        &self,
        channel: &ChannelId,
        plan: &Plan,
    ) -> Result<()>;

    /// Clear progress tracking for channel
    pub async fn clear_progress(&self, channel: &ChannelId);

    /// Format plan as Slack message with emojis
    fn format_plan(plan: &Plan) -> String;
}
```

### 5.7 Agent Manager

```rust
// src/agent/manager.rs

impl AgentManager {
    /// Create new agent manager with main agent and empty repo agent pool
    pub async fn new(settings: Arc<Settings>, workspace: Arc<Workspace>) -> Result<Self>;

    /// Scan Slack channels and restore existing agents from disk
    pub async fn scan_and_restore_channels(&self, slack_client: &SlackClient) -> Result<()>;

    /// Setup a new channel - invokes main agent to validate, clone, analyze, generate prompt
    pub async fn setup_channel(
        &self,
        channel_id: ChannelId,
        repo_name: String,
        slack_client: Arc<SlackClient>,
    ) -> Result<()>;

    /// Get repository agent for a channel
    pub async fn get_repo_agent(&self, channel_id: &ChannelId) -> Result<Arc<RepoAgent>>;

    /// Remove agent for a channel
    pub async fn remove_agent(&self, channel_id: &ChannelId) -> Result<()>;

    /// Cleanup inactive agents (background task)
    pub async fn cleanup_inactive_agents(&self) -> Result<()>;

    /// Check if channel has a configured agent
    pub fn has_agent(&self, channel_id: &ChannelId) -> bool;
}
```

### 5.8 Main Agent

```rust
// src/agent/main_agent.rs

use claude_agent_sdk_rs::{ClaudeClient, ClaudeAgentOptions, SystemPrompt, PermissionMode, Hooks};

impl MainAgent {
    /// Create new main agent with TodoWrite hook
    pub async fn new(
        settings: Arc<Settings>,
        workspace: Arc<Workspace>,
        progress_tracker: Arc<ProgressTracker>,
        channel_id: ChannelId,
    ) -> Result<Self>;

    /// Connect to Claude API
    pub async fn connect(&mut self) -> Result<()>;

    /// Run repository setup process (validate, clone, analyze, generate prompt)
    /// This invokes the Claude agent with a detailed setup prompt
    pub async fn setup_repository(
        &mut self,
        repo_name: &str,
    ) -> Result<()>;

    /// Get current plan state
    pub fn get_plan(&self) -> Plan;

    /// Disconnect from Claude API
    pub async fn disconnect(self) -> Result<()>;
}

// Implementation pattern:
// 1. Create hooks with TodoWrite matcher
// 2. Hook updates internal Plan via Arc<Mutex<Plan>>
// 3. Hook calls progress_tracker.update_progress() to update Slack
// 4. Use ClaudeAgentOptions::builder() to configure agent
```

### 5.9 Repo Agent

```rust
// src/agent/repo_agent.rs

use claude_agent_sdk_rs::{ClaudeClient, Message};
use futures::Stream;

impl RepoAgent {
    /// Create new repository-specific agent with TodoWrite hook
    pub async fn new(
        channel_id: ChannelId,
        workspace: Arc<Workspace>,
        settings: Arc<Settings>,
        progress_tracker: Arc<ProgressTracker>,
    ) -> Result<Self>;

    /// Connect to Claude API
    pub async fn connect(&mut self) -> Result<()>;

    /// Send query to agent
    pub async fn query(&mut self, message: &str) -> Result<()>;

    /// Get response stream from agent
    pub fn receive_response(&mut self) -> impl Stream<Item = Result<Message, claude_agent_sdk_rs::ClaudeError>> + '_;

    /// Get current plan state
    pub fn get_plan(&self) -> Plan;

    /// Get plan Arc for concurrent access
    pub fn get_plan_arc(&self) -> Arc<Mutex<Plan>>;

    fn update_activity(&self);

    pub fn is_expired(&self, timeout: Duration) -> bool;

    /// Disconnect from Claude API
    pub async fn disconnect(self) -> Result<()>;
}

// Usage pattern:
// 1. agent.connect().await
// 2. agent.query(user_message).await
// 3. let mut stream = agent.receive_response()
// 4. while let Some(msg) = stream.next().await { ... }
// 5. Meanwhile, TodoWrite hook updates plan and Slack
```

### 5.10 Agent Hooks

```rust
// src/agent/hooks.rs

use claude_agent_sdk_rs::{Hooks, HookInput, HookContext, HookJsonOutput, SyncHookJsonOutput};

/// Create hooks for TodoWrite tracking
pub fn create_todo_hooks(
    plan: Arc<Mutex<Plan>>,
    progress_tracker: Arc<ProgressTracker>,
    channel_id: ChannelId,
) -> Hooks {
    let mut hooks = Hooks::new();

    // Clone Arcs for the closure
    let plan_clone = Arc::clone(&plan);
    let tracker_clone = Arc::clone(&progress_tracker);

    hooks.add_post_tool_use_with_matcher(
        "TodoWrite",
        move |input: HookInput, _tool_use_id: Option<String>, _context: HookContext| {
            let plan = Arc::clone(&plan_clone);
            let tracker = Arc::clone(&tracker_clone);
            let channel = channel_id.clone();

            Box::pin(async move {
                if let HookInput::PostToolUse(post_tool) = input {
                    // Parse TodoWrite tool input
                    if let Ok(new_plan) = serde_json::from_value::<Plan>(post_tool.tool_input) {
                        // Update internal plan
                        if let Ok(mut p) = plan.lock() {
                            p.update(new_plan.clone());
                        }

                        // Update Slack progress display
                        let _ = tracker.update_progress(&channel, &new_plan).await;
                    }
                }
                HookJsonOutput::Sync(SyncHookJsonOutput::default())
            })
        },
    );

    hooks
}
```

**Note**: Sections 5.9-5.12 (Repository Validator, Cloner, Analyzer, Prompt Generator) are
removed since all repository operations are now handled by Claude agents, not the bot application.

### 5.11 Workspace (Storage)

Already covered in section 4.4 Storage Types above. The Workspace provides path utilities
for the bot to locate repositories and system prompts, but does not perform any git/file operations.

## 6. Implementation Phases

### Phase 1: Foundation (Week 1)

- Project structure setup
- Configuration management
- Error handling framework
- Logging and tracing
- Workspace path management
- Storage utilities (ChannelConfig)

### Phase 2: Slack Integration (Week 2)

- Slack client wrapper
- Event handling
- Message processing and routing
- Interactive forms
- Message formatting and code blocks
- Threading support
- Progress tracking and display

### Phase 3: Agent Integration (Week 3)

- Main agent wrapper and setup prompt
- Repository agent wrapper
- Agent manager (lifecycle, pool management)
- PostToolUse hook for TodoWrite
- Agent response streaming
- Channel scanning and restoration

### Phase 4: End-to-End Workflows (Week 4)

- Channel setup workflow (form → main agent → repo agent creation)
- Message forwarding workflow (user → bot → repo agent → Slack)
- Progress tracking workflow (TodoWrite hook → progress updates)
- Error handling and user feedback
- Agent cleanup and resource management

### Phase 5: Testing and Polish (Week 5)

- Integration tests with mock Slack/Claude APIs
- Test channel setup and message processing flows
- Error handling refinement
- Performance optimization (concurrent channels)
- Documentation
- Deployment preparation (Docker, env vars)

## 7. Testing Strategy

### 7.1 Unit Tests

- Each module has its own test suite
- Mock external dependencies (Slack API, Claude API)
- Test error conditions
- Test edge cases

### 7.2 Integration Tests

- End-to-end message flow
- Repository setup workflow
- Agent lifecycle management
- Concurrent request handling

### 7.3 Manual Testing

- Real Slack workspace testing
- Various repository types
- Performance under load
- Error recovery scenarios

## 8. Deployment Considerations

### 8.1 Environment Variables

```bash
SLACK_BOT_TOKEN=xoxb-...
SLACK_APP_TOKEN=xapp-...
SLACK_SIGNING_SECRET=...
CLAUDE_API_KEY=...
WORKSPACE_BASE_PATH=~/.slack_coder
LOG_LEVEL=info
```

### 8.2 Docker Support

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y git gh
COPY --from=builder /app/target/release/slack-coder /usr/local/bin/
CMD ["slack-coder"]
```

### 8.3 Monitoring

- Structured logging with tracing
- Metrics for agent operations
- Health check endpoint
- Resource usage monitoring

## 9. Security Considerations

### 9.1 Secrets Management

- Use environment variables
- Support secrets managers (AWS Secrets Manager, etc.)
- Never log sensitive data

### 9.2 Input Validation

- Validate all user inputs
- Sanitize file paths
- Limit command execution

### 9.3 Access Control

- Verify Slack signatures
- Respect channel permissions
- Limit repository access

## 10. Performance Optimization

### 10.1 Concurrency

- Use Tokio for async operations
- Connection pooling for HTTP clients
- Parallel codebase analysis

### 10.2 Caching

- Cache system prompts
- Cache repository metadata
- Reuse agent instances

### 10.3 Resource Management

- Limit concurrent agents
- Cleanup inactive agents
- Monitor memory usage
