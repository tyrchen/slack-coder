//! Logging utilities for structured tracing

use std::time::Instant;

/// Track operation timing and log on drop
pub struct Timer {
    start: Instant,
    operation: String,
}

impl Timer {
    /// Create a new timer for an operation
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            operation: operation.into(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let duration_ms = self.start.elapsed().as_millis() as u64;
        tracing::debug!(
            operation = %self.operation,
            duration_ms = duration_ms,
            "Operation completed"
        );
    }
}

/// Log an error with structured context
pub fn log_error(operation: &str, error: &impl std::error::Error) {
    tracing::error!(
        operation = %operation,
        error = %error,
        error_kind = std::any::type_name_of_val(error),
        "Operation failed"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_timer_tracks_duration() {
        let _timer = Timer::new("test_operation");
        thread::sleep(Duration::from_millis(10));
        // Timer will log on drop
    }
}
