use thiserror::Error;

#[derive(Debug, Error)]
pub enum SlackCoderError {
    #[error("Slack API error: {0}")]
    SlackApi(String),

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
