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

    /// Get reference to SlackClient for sending custom messages
    pub fn slack_client_ref(&self) -> Arc<SlackClient> {
        Arc::clone(&self.slack_client)
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

    /// Format duration in a human-readable way
    fn format_duration(seconds: f64) -> String {
        if seconds < 60.0 {
            format!("{:.1}s", seconds)
        } else if seconds < 3600.0 {
            let minutes = (seconds / 60.0).floor();
            let secs = seconds % 60.0;
            format!("{}m {:.0}s", minutes, secs)
        } else {
            let hours = (seconds / 3600.0).floor();
            let minutes = ((seconds % 3600.0) / 60.0).floor();
            format!("{}h {}m", hours, minutes)
        }
    }

    /// Format progress bar with visual indicator
    fn format_progress_bar(completed: usize, total: usize) -> String {
        let percentage = if total > 0 {
            (completed as f64 / total as f64 * 100.0) as usize
        } else {
            0
        };

        let filled = if total > 0 {
            (completed as f64 / total as f64 * 10.0).ceil() as usize
        } else {
            0
        };
        let empty = 10_usize.saturating_sub(filled);

        let bar = "█".repeat(filled) + &"░".repeat(empty);

        format!(
            "*Task Progress* — {} of {} complete ({}%)\n[{}]",
            completed, total, percentage, bar
        )
    }

    /// Format plan as Slack message with emojis and timing information
    fn format_plan(plan: &Plan) -> String {
        let completed = plan.get_completed_count();
        let total = plan.get_total_count();

        let mut lines = vec![Self::format_progress_bar(completed, total)];

        for task in &plan.todos {
            // Use checkbox-style emojis for better visual clarity
            let emoji = match task.status {
                TaskStatus::Completed => ":ballot_box_with_check:",
                TaskStatus::InProgress => ":arrows_counterclockwise:", // More dynamic animated emoji
                TaskStatus::Pending => ":white_medium_square:",
            };

            let text = if task.status == TaskStatus::InProgress {
                &task.active_form
            } else {
                &task.content
            };

            // Add timing information
            let timing = match task.status {
                TaskStatus::Completed => {
                    if let Some(duration) = task.completion_time {
                        format!(" `{}`", Self::format_duration(duration))
                    } else {
                        String::new()
                    }
                }
                TaskStatus::InProgress => {
                    if let Some(start) = task.start_time {
                        let elapsed = start.elapsed().as_secs_f64();
                        format!(" `{}`", Self::format_duration(elapsed))
                    } else {
                        String::new()
                    }
                }
                TaskStatus::Pending => String::new(),
            };

            lines.push(format!("{} {}{}", emoji, text, timing));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Task;

    #[test]
    fn test_format_duration() {
        assert_eq!(ProgressTracker::format_duration(0.5), "0.5s");
        assert_eq!(ProgressTracker::format_duration(1.2), "1.2s");
        assert_eq!(ProgressTracker::format_duration(45.8), "45.8s");
        assert_eq!(ProgressTracker::format_duration(65.0), "1m 5s");
        assert_eq!(ProgressTracker::format_duration(125.0), "2m 5s");
        assert_eq!(ProgressTracker::format_duration(3665.0), "1h 1m");
        assert_eq!(ProgressTracker::format_duration(7384.0), "2h 3m");
    }

    #[test]
    fn test_format_progress_bar() {
        assert_eq!(
            ProgressTracker::format_progress_bar(0, 5),
            "*Task Progress* — 0 of 5 complete (0%)\n[░░░░░░░░░░]"
        );
        assert_eq!(
            ProgressTracker::format_progress_bar(1, 5),
            "*Task Progress* — 1 of 5 complete (20%)\n[██░░░░░░░░]"
        );
        assert_eq!(
            ProgressTracker::format_progress_bar(2, 5),
            "*Task Progress* — 2 of 5 complete (40%)\n[████░░░░░░]"
        );
        assert_eq!(
            ProgressTracker::format_progress_bar(5, 5),
            "*Task Progress* — 5 of 5 complete (100%)\n[██████████]"
        );
    }

    #[test]
    fn test_format_plan_basic() {
        let mut plan = Plan::new();
        plan.todos = vec![
            Task {
                content: "Task 1".to_string(),
                active_form: "Doing task 1".to_string(),
                status: TaskStatus::Completed,
                start_time: None,
                completion_time: Some(1.5),
            },
            Task {
                content: "Task 2".to_string(),
                active_form: "Doing task 2".to_string(),
                status: TaskStatus::InProgress,
                start_time: Some(std::time::Instant::now()),
                completion_time: None,
            },
            Task {
                content: "Task 3".to_string(),
                active_form: "Doing task 3".to_string(),
                status: TaskStatus::Pending,
                start_time: None,
                completion_time: None,
            },
        ];

        let formatted = ProgressTracker::format_plan(&plan);

        // Verify structure
        assert!(formatted.contains("*Task Progress*"));
        assert!(formatted.contains("1 of 3 complete"));
        assert!(formatted.contains("33%"));
        assert!(formatted.contains(":ballot_box_with_check: Task 1"));
        assert!(formatted.contains(":arrows_counterclockwise: Doing task 2"));
        assert!(formatted.contains(":white_medium_square: Task 3"));
        assert!(formatted.contains("1.5s"));
    }

    #[test]
    fn test_format_plan_emoji_selection() {
        let mut plan = Plan::new();
        plan.todos = vec![
            Task {
                content: "Completed task".to_string(),
                active_form: "Doing completed task".to_string(),
                status: TaskStatus::Completed,
                start_time: None,
                completion_time: Some(0.8),
            },
            Task {
                content: "In progress task".to_string(),
                active_form: "Working on task".to_string(),
                status: TaskStatus::InProgress,
                start_time: Some(std::time::Instant::now()),
                completion_time: None,
            },
            Task {
                content: "Pending task".to_string(),
                active_form: "Will do pending task".to_string(),
                status: TaskStatus::Pending,
                start_time: None,
                completion_time: None,
            },
        ];

        let formatted = ProgressTracker::format_plan(&plan);

        // Check for checkbox-style emojis
        assert!(formatted.contains(":ballot_box_with_check:"));
        assert!(formatted.contains(":arrows_counterclockwise:"));
        assert!(formatted.contains(":white_medium_square:"));

        // Verify in-progress uses active form
        assert!(formatted.contains("Working on task"));
        assert!(!formatted.contains("In progress task"));

        // Verify others use content
        assert!(formatted.contains("Completed task"));
        assert!(formatted.contains("Pending task"));
    }
}
