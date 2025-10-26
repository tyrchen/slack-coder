use crate::error::{Result, SlackCoderError};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Settings {
    pub slack: SlackConfig,
    pub claude: ClaudeConfig,
    pub workspace: WorkspaceConfig,
    pub agent: AgentConfig,
}

#[derive(Debug, Clone)]
pub struct SlackConfig {
    pub bot_token: String,
    pub app_token: String,
    pub signing_secret: String,
}

#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    pub model: String,
    pub max_tokens: usize,
}

#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    pub base_path: PathBuf,
    pub max_repo_size_mb: u64,
    pub cleanup_interval_secs: u64,
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub main_agent_prompt_path: PathBuf,
    pub agent_timeout_secs: u64,
    pub max_concurrent_requests: usize,
}

pub fn load_settings() -> Result<Settings> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    // Load Slack config
    let slack = SlackConfig {
        bot_token: std::env::var("SLACK_BOT_TOKEN")
            .map_err(|_| SlackCoderError::Config("SLACK_BOT_TOKEN not set".to_string()))?,
        app_token: std::env::var("SLACK_APP_TOKEN")
            .map_err(|_| SlackCoderError::Config("SLACK_APP_TOKEN not set".to_string()))?,
        signing_secret: std::env::var("SLACK_SIGNING_SECRET")
            .map_err(|_| SlackCoderError::Config("SLACK_SIGNING_SECRET not set".to_string()))?,
    };

    // Load Claude config
    let claude = ClaudeConfig {
        model: std::env::var("CLAUDE_MODEL").unwrap_or_else(|_| "claude-sonnet-4".to_string()),
        max_tokens: std::env::var("CLAUDE_MAX_TOKENS")
            .unwrap_or_else(|_| "65536".to_string())
            .parse()
            .map_err(|_| SlackCoderError::Config("Invalid CLAUDE_MAX_TOKENS".to_string()))?,
    };

    // Load workspace config
    let workspace = WorkspaceConfig {
        base_path: std::env::var("WORKSPACE_BASE_PATH")
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                format!("{}/.slack_coder", home)
            })
            .into(),
        max_repo_size_mb: std::env::var("MAX_REPO_SIZE_MB")
            .unwrap_or_else(|_| "1024".to_string())
            .parse()
            .map_err(|_| SlackCoderError::Config("Invalid MAX_REPO_SIZE_MB".to_string()))?,
        cleanup_interval_secs: std::env::var("CLEANUP_INTERVAL_SECS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse()
            .map_err(|_| SlackCoderError::Config("Invalid CLEANUP_INTERVAL_SECS".to_string()))?,
    };

    // Load agent config
    let agent = AgentConfig {
        main_agent_prompt_path: std::env::var("MAIN_AGENT_PROMPT_PATH")
            .unwrap_or_else(|_| "specs/0003-system-prompt.md".to_string())
            .into(),
        agent_timeout_secs: std::env::var("AGENT_TIMEOUT_SECS")
            .unwrap_or_else(|_| "1800".to_string())
            .parse()
            .map_err(|_| SlackCoderError::Config("Invalid AGENT_TIMEOUT_SECS".to_string()))?,
        max_concurrent_requests: std::env::var("MAX_CONCURRENT_REQUESTS")
            .unwrap_or_else(|_| "10".to_string())
            .parse()
            .map_err(|_| SlackCoderError::Config("Invalid MAX_CONCURRENT_REQUESTS".to_string()))?,
    };

    Ok(Settings {
        slack,
        claude,
        workspace,
        agent,
    })
}
