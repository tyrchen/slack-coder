use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::time::Instant;

/// Represents the status of a task in the plan
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

/// Represents a single task in the plan
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

impl Hash for Task {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.content.hash(state);
        self.active_form.hash(state);
        self.status.hash(state);
        // Skip timing fields - they don't affect task identity
    }
}

/// Represents the overall plan with multiple tasks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Plan {
    pub todos: Vec<Task>,
}

impl Plan {
    pub fn new() -> Self {
        Self { todos: Vec::new() }
    }

    pub fn update(&mut self, new_plan: Plan) {
        let now = Instant::now();

        // Track timing for status changes
        for (i, new_task) in new_plan.todos.iter().enumerate() {
            if let Some(existing_task) = self.todos.get_mut(i) {
                // Track status transitions
                let old_status = existing_task.status.clone();
                let new_status = new_task.status.clone();

                // Task started (Pending → InProgress)
                if old_status != TaskStatus::InProgress && new_status == TaskStatus::InProgress {
                    existing_task.start_time = Some(now);
                }
                // Task completed from InProgress
                else if old_status == TaskStatus::InProgress
                    && new_status == TaskStatus::Completed
                {
                    if let Some(start_time) = existing_task.start_time {
                        existing_task.completion_time = Some(start_time.elapsed().as_secs_f64());
                    }
                }
                // Task completed directly from Pending (never went InProgress)
                else if old_status == TaskStatus::Pending && new_status == TaskStatus::Completed {
                    // Use a minimal time to indicate completion
                    existing_task.completion_time = Some(0.1);
                }

                existing_task.content = new_task.content.clone();
                existing_task.active_form = new_task.active_form.clone();
                existing_task.status = new_task.status.clone();
            }
        }

        // Add new tasks
        if new_plan.todos.len() > self.todos.len() {
            for new_task in new_plan.todos.iter().skip(self.todos.len()) {
                let mut task = new_task.clone();
                // Initialize timing based on current status
                if task.status == TaskStatus::InProgress {
                    task.start_time = Some(now);
                } else if task.status == TaskStatus::Completed {
                    task.completion_time = Some(0.1); // Default minimal time
                }
                self.todos.push(task);
            }
        }
    }

    pub fn get_current_task(&self) -> Option<&Task> {
        self.todos
            .iter()
            .find(|t| t.status == TaskStatus::InProgress)
    }

    pub fn get_completed_count(&self) -> usize {
        self.todos
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count()
    }

    pub fn get_total_count(&self) -> usize {
        self.todos.len()
    }

    pub fn is_complete(&self) -> bool {
        !self.todos.is_empty() && self.todos.iter().all(|t| t.status == TaskStatus::Completed)
    }
}
