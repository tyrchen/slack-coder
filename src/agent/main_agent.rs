use crate::agent::{Plan, create_todo_hooks};
use crate::config::Settings;
use crate::error::{Result, SlackCoderError};
use crate::slack::{ChannelId, ProgressTracker};
use crate::storage::Workspace;
use claude_agent_sdk_rs::{ClaudeAgentOptions, ClaudeClient, PermissionMode, SystemPrompt};
use futures::StreamExt;
use std::sync::{Arc, Mutex};

pub struct MainAgent {
    client: ClaudeClient,
    plan: Arc<Mutex<Plan>>,
}

impl MainAgent {
    /// Create new main agent with TodoWrite hook
    pub async fn new(
        settings: Arc<Settings>,
        workspace: Arc<Workspace>,
        progress_tracker: Arc<ProgressTracker>,
        channel_id: ChannelId,
    ) -> Result<Self> {
        let plan = Arc::new(Mutex::new(Plan::new()));

        // Load main agent system prompt
        let system_prompt = tokio::fs::read_to_string(&settings.agent.main_agent_prompt_path)
            .await
            .map_err(|e| {
                SlackCoderError::Config(format!(
                    "Failed to load main agent prompt from {:?}: {}",
                    settings.agent.main_agent_prompt_path, e
                ))
            })?;

        // Create hooks
        let hooks = create_todo_hooks(Arc::clone(&plan), progress_tracker, channel_id.clone());

        // Build agent options
        let options = ClaudeAgentOptions::builder()
            .permission_mode(PermissionMode::BypassPermissions)
            .system_prompt(SystemPrompt::Text(system_prompt))
            .cwd(workspace.repo_path(&channel_id))
            .hooks(hooks.build())
            .build();

        let client = ClaudeClient::new(options);

        Ok(Self { client, plan })
    }

    /// Connect to Claude API
    pub async fn connect(&mut self) -> Result<()> {
        self.client
            .connect()
            .await
            .map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;
        Ok(())
    }

    /// Run repository setup process
    pub async fn setup_repository(
        &mut self,
        repo_name: &str,
        channel_id: &ChannelId,
    ) -> Result<()> {
        let prompt = format!(
            r#"Please set up the repository {} for channel {}.

Tasks:
1. Validate the repository exists and is accessible using gh CLI
2. Clone it to ~/.slack_coder/repos/{}
3. Analyze the codebase comprehensively
4. Generate a system prompt for this repository
5. Save the system prompt to ~/.slack_coder/system/{}/system_prompt.md

The repository name provided by the user is: {}"#,
            repo_name,
            channel_id.as_str(),
            channel_id.as_str(),
            channel_id.as_str(),
            repo_name
        );

        self.client
            .query(&prompt)
            .await
            .map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

        // Receive response stream
        let mut stream = self.client.receive_response();
        let mut final_result = String::new();

        while let Some(message) = stream.next().await {
            let message = message.map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;

            if let claude_agent_sdk_rs::Message::Result(res) = message {
                final_result = res.result.unwrap_or_default();
                break;
            }
        }

        tracing::info!("Setup completed: {}", final_result);
        Ok(())
    }

    /// Get current plan state
    pub fn get_plan(&self) -> Plan {
        self.plan.lock().unwrap().clone()
    }

    /// Disconnect from Claude API
    pub async fn disconnect(mut self) -> Result<()> {
        self.client
            .disconnect()
            .await
            .map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;
        Ok(())
    }
}
