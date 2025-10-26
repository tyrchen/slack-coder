use crate::agent::{Plan, create_todo_hooks};
use crate::config::Settings;
use crate::error::{Result, SlackCoderError};
use crate::slack::{ChannelId, ProgressTracker};
use crate::storage::Workspace;
use claude_agent_sdk_rs::{
    ClaudeAgentOptions, ClaudeClient, Message, PermissionMode, SystemPrompt,
};
use futures::Stream;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

pub struct RepoAgent {
    client: ClaudeClient,
    plan: Arc<Mutex<Plan>>,
    channel_id: ChannelId,
    last_activity: Arc<RwLock<Instant>>,
}

impl RepoAgent {
    /// Create new repository-specific agent with TodoWrite hook
    pub async fn new(
        channel_id: ChannelId,
        workspace: Arc<Workspace>,
        _settings: Arc<Settings>,
        progress_tracker: Arc<ProgressTracker>,
    ) -> Result<Self> {
        let plan = Arc::new(Mutex::new(Plan::new()));

        // Start with common workflow requirements (so they're seen first!)
        let mut system_prompt = String::new();
        system_prompt.push_str(include_str!("../../prompts/repo-agent-workflow.md"));
        system_prompt.push_str("\n\n---\n\n");

        // Append repository-specific system prompt from disk
        let repo_prompt = workspace
            .load_system_prompt(&channel_id)
            .await
            .map_err(|e| {
                SlackCoderError::Config(format!(
                    "Failed to load system prompt for channel {}: {}",
                    channel_id.as_str(),
                    e
                ))
            })?;
        system_prompt.push_str(&repo_prompt);

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

        Ok(Self {
            client,
            plan,
            channel_id,
            last_activity: Arc::new(RwLock::new(Instant::now())),
        })
    }

    /// Connect to Claude API
    pub async fn connect(&mut self) -> Result<()> {
        self.client
            .connect()
            .await
            .map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;
        self.update_activity();
        Ok(())
    }

    /// Send query to agent
    pub async fn query(&mut self, message: &str) -> Result<()> {
        self.client
            .query(message)
            .await
            .map_err(|e| SlackCoderError::ClaudeAgent(e.to_string()))?;
        self.update_activity();
        Ok(())
    }

    /// Get response stream from agent
    pub fn receive_response(
        &mut self,
    ) -> impl Stream<Item = std::result::Result<Message, claude_agent_sdk_rs::ClaudeError>> + '_
    {
        self.client.receive_response()
    }

    /// Get current plan state
    pub fn get_plan(&self) -> Plan {
        self.plan.lock().unwrap().clone()
    }

    /// Get plan Arc for concurrent access
    pub fn get_plan_arc(&self) -> Arc<Mutex<Plan>> {
        Arc::clone(&self.plan)
    }

    /// Update last activity timestamp
    fn update_activity(&self) {
        *self.last_activity.write().unwrap() = Instant::now();
    }

    /// Check if agent is expired based on timeout
    pub fn is_expired(&self, timeout: Duration) -> bool {
        let last = *self.last_activity.read().unwrap();
        last.elapsed() > timeout
    }

    /// Get channel ID
    pub fn channel_id(&self) -> &ChannelId {
        &self.channel_id
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
