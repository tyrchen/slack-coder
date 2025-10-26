use crate::agent::{Plan, TaskStatus};
use crate::error::Result;
use crate::slack::{ChannelId, MessageTs, SlackClient};
use dashmap::DashMap;
use std::sync::Arc;

pub struct ProgressTracker {
    slack_client: Arc<SlackClient>,
    active_progress: Arc<DashMap<ChannelId, MessageTs>>,
}

impl ProgressTracker {
    pub fn new(slack_client: Arc<SlackClient>) -> Self {
        Self {
            slack_client,
            active_progress: Arc::new(DashMap::new()),
        }
    }

    /// Display initial progress message
    pub async fn start_progress(&self, channel: &ChannelId, initial_plan: &Plan) -> Result<()> {
        let formatted = Self::format_plan(initial_plan);
        let ts = self
            .slack_client
            .send_message(channel, &formatted, None)
            .await?;

        self.active_progress.insert(channel.clone(), ts);
        Ok(())
    }

    /// Update progress message with new plan state
    pub async fn update_progress(&self, channel: &ChannelId, plan: &Plan) -> Result<()> {
        let formatted = Self::format_plan(plan);

        if let Some(ts) = self.active_progress.get(channel) {
            self.slack_client
                .update_message(channel, &ts, &formatted)
                .await?;
        } else {
            // If no active progress message, create one
            let ts = self
                .slack_client
                .send_message(channel, &formatted, None)
                .await?;
            self.active_progress.insert(channel.clone(), ts);
        }

        Ok(())
    }

    /// Clear progress tracking for channel
    pub async fn clear_progress(&self, channel: &ChannelId) {
        self.active_progress.remove(channel);
    }

    /// Format plan as Slack message with emojis
    fn format_plan(plan: &Plan) -> String {
        let completed = plan.get_completed_count();
        let total = plan.get_total_count();

        let mut lines = vec![format!("*Progress:* {} / {}", completed, total)];

        for task in &plan.todos {
            let emoji = match task.status {
                TaskStatus::Completed => ":white_check_mark:",
                TaskStatus::InProgress => ":hourglass_flowing_sand:", // Animated emoji
                TaskStatus::Pending => ":white_medium_square:",
            };

            let text = if task.status == TaskStatus::InProgress {
                &task.active_form
            } else {
                &task.content
            };

            lines.push(format!("{} {}", emoji, text));
        }

        lines.join("\n")
    }
}
