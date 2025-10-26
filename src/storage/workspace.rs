use crate::error::Result;
use crate::slack::ChannelId;
use std::path::PathBuf;
use tokio::fs;

pub struct Workspace {
    base_path: PathBuf,
}

impl Workspace {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Returns path to channel's repository: ~/.slack_coder/repos/{channel_id}/
    pub fn repo_path(&self, channel_id: &ChannelId) -> PathBuf {
        self.base_path.join("repos").join(channel_id.as_str())
    }

    /// Returns path to channel's system prompt: ~/.slack_coder/system/{channel_id}/system_prompt.md
    pub fn system_prompt_path(&self, channel_id: &ChannelId) -> PathBuf {
        self.base_path
            .join("system")
            .join(channel_id.as_str())
            .join("system_prompt.md")
    }

    /// Check if channel has an existing repository setup
    pub async fn is_channel_setup(&self, channel_id: &ChannelId) -> bool {
        let repo_path = self.repo_path(channel_id);
        let system_prompt_path = self.system_prompt_path(channel_id);

        // Check if both repo directory and system prompt exist
        let repo_exists = fs::metadata(&repo_path).await.is_ok();
        let prompt_exists = fs::metadata(&system_prompt_path).await.is_ok();

        repo_exists && prompt_exists
    }

    /// Load system prompt from disk
    pub async fn load_system_prompt(&self, channel_id: &ChannelId) -> Result<String> {
        let path = self.system_prompt_path(channel_id);
        let content = fs::read_to_string(&path).await?;
        Ok(content)
    }

    /// Ensure workspace directories exist
    pub async fn ensure_workspace(&self) -> Result<()> {
        fs::create_dir_all(self.base_path.join("repos")).await?;
        fs::create_dir_all(self.base_path.join("system")).await?;
        Ok(())
    }
}
