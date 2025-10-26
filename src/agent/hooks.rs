use crate::agent::{Plan, TaskStatus};
use crate::slack::{ChannelId, ProgressTracker};
use claude_agent_sdk_rs::{HookContext, HookInput, HookJsonOutput, Hooks, SyncHookJsonOutput};
use std::sync::{Arc, Mutex};

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

/// Format plan for display with timing information
#[allow(dead_code)]
pub fn format_plan_summary(plan: &Plan) -> String {
    let completed = plan.get_completed_count();
    let total = plan.get_total_count();
    let current = plan.get_current_task();

    let mut lines = vec![format!("Progress: {}/{}", completed, total)];

    if let Some(task) = current {
        lines.push(format!("Current: {}", task.active_form));
    }

    for task in &plan.todos {
        let emoji = match task.status {
            TaskStatus::Completed => "✅",
            TaskStatus::InProgress => "⏳",
            TaskStatus::Pending => "⬜",
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
