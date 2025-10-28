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
    tracing::debug!(
        channel = %channel_id.as_str(),
        "Creating TodoWrite hooks"
    );
    let mut hooks = Hooks::new();

    // Clone Arcs for the closure
    let plan_clone = Arc::clone(&plan);
    let tracker_clone = Arc::clone(&progress_tracker);
    let channel_clone = channel_id.clone(); // Clone for logging later

    hooks.add_post_tool_use_with_matcher(
        "TodoWrite",
        move |input: HookInput, _tool_use_id: Option<String>, _context: HookContext| {
            let plan = Arc::clone(&plan_clone);
            let tracker = Arc::clone(&tracker_clone);
            let channel = channel_id.clone();

            Box::pin(async move {
                tracing::debug!("TodoWrite hook triggered");
                if let HookInput::PostToolUse(post_tool) = input {
                    tracing::debug!(
                        tool_name = %post_tool.tool_name,
                        input_len = post_tool.tool_input.to_string().len(),
                        "Tool use invocation"
                    );

                    // Parse TodoWrite tool input
                    match serde_json::from_value::<Plan>(post_tool.tool_input.clone()) {
                        Ok(new_plan) => {
                            let completed = new_plan.todos.iter().filter(|t| t.status == TaskStatus::Completed).count();
                            let in_progress = new_plan.todos.iter().filter(|t| t.status == TaskStatus::InProgress).count();
                            let pending = new_plan.todos.iter().filter(|t| t.status == TaskStatus::Pending).count();

                            tracing::info!(
                                total_tasks = new_plan.todos.len(),
                                completed = completed,
                                in_progress = in_progress,
                                pending = pending,
                                "Parsed TodoWrite plan"
                            );

                            // Update internal plan with timing tracking
                            let plan_to_display = if let Ok(mut p) = plan.lock() {
                                p.update(new_plan.clone());
                                tracing::debug!("Updated internal plan with timing");
                                p.clone() // Use the plan with timing data
                            } else {
                                tracing::warn!("Failed to lock plan, using new plan without timing");
                                new_plan // Fallback to new_plan if lock fails
                            };

                            // Update Slack progress display with plan that includes timing
                            tracing::debug!(
                                channel = %channel.as_str(),
                                "Updating Slack progress"
                            );
                            match tracker.update_progress(&channel, &plan_to_display).await {
                                Ok(_) => tracing::debug!("Progress updated in Slack"),
                                Err(e) => tracing::error!(error = %e, "Failed to update progress"),
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                input_preview = %post_tool.tool_input.to_string().chars().take(100).collect::<String>(),
                                "Failed to parse TodoWrite input"
                            );
                        }
                    }
                } else {
                    tracing::warn!(
                        input_type = std::any::type_name_of_val(&input),
                        "Hook called with unexpected input type"
                    );
                }
                HookJsonOutput::Sync(SyncHookJsonOutput::default())
            })
        },
    );

    tracing::debug!(
        channel = %channel_clone.as_str(),
        "TodoWrite hooks registered"
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
