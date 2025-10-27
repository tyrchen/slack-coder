//! Usage metrics tracking and formatting for Slack notifications

use claude_agent_sdk_rs::ResultMessage;
use serde::{Deserialize, Serialize};

/// Usage statistics extracted from ResultMessage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMetrics {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cost_usd: Option<f64>,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    pub num_turns: u32,
    pub session_id: String,
}

/// Usage data structure from Claude API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageData {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
}

impl UsageMetrics {
    /// Extract usage metrics from a ResultMessage
    pub fn from_result_message(result: &ResultMessage) -> Self {
        let (input_tokens, output_tokens, cache_creation, cache_read) =
            if let Some(usage) = &result.usage {
                // Try to parse usage data
                match serde_json::from_value::<UsageData>(usage.clone()) {
                    Ok(data) => (
                        data.input_tokens,
                        data.output_tokens,
                        data.cache_creation_input_tokens,
                        data.cache_read_input_tokens,
                    ),
                    Err(e) => {
                        tracing::warn!("Failed to parse usage data: {}", e);
                        (0, 0, 0, 0)
                    }
                }
            } else {
                (0, 0, 0, 0)
            };

        let total_tokens = input_tokens + output_tokens;

        Self {
            input_tokens,
            output_tokens,
            total_tokens,
            cache_creation_input_tokens: cache_creation,
            cache_read_input_tokens: cache_read,
            cost_usd: result.total_cost_usd,
            duration_ms: result.duration_ms,
            duration_api_ms: result.duration_api_ms,
            num_turns: result.num_turns,
            session_id: result.session_id.clone(),
        }
    }

    /// Format metrics as a Slack message
    pub fn format_slack_message(&self) -> String {
        let cost_str = if let Some(cost) = self.cost_usd {
            format!("${:.4} USD", cost)
        } else {
            "N/A".to_string()
        };

        let duration_sec = self.duration_ms as f64 / 1000.0;
        let api_duration_sec = self.duration_api_ms as f64 / 1000.0;

        let mut message = format!(
            "📊 *Query Metrics*\n\
             • Tokens: {} input + {} output = *{} total*\n\
             • Cost: {}\n\
             • Duration: {:.2}s (API: {:.2}s)\n\
             • Turns: {}\n\
             • Session: `{}`",
            self.input_tokens,
            self.output_tokens,
            self.total_tokens,
            cost_str,
            duration_sec,
            api_duration_sec,
            self.num_turns,
            self.session_id
        );

        // Add cache info if present
        if self.cache_creation_input_tokens > 0 || self.cache_read_input_tokens > 0 {
            message.push_str(&format!(
                "\n• Cache: {} created, {} read",
                self.cache_creation_input_tokens, self.cache_read_input_tokens
            ));
        }

        message
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_usage_metrics_from_result_message() {
        let result = ResultMessage {
            subtype: "query_complete".to_string(),
            duration_ms: 1500,
            duration_api_ms: 1200,
            is_error: false,
            num_turns: 3,
            session_id: "test-session-123".to_string(),
            total_cost_usd: Some(0.0042),
            usage: Some(json!({
                "input_tokens": 2095,
                "output_tokens": 503,
                "cache_creation_input_tokens": 100,
                "cache_read_input_tokens": 50
            })),
            result: None,
        };

        let metrics = UsageMetrics::from_result_message(&result);

        assert_eq!(metrics.input_tokens, 2095);
        assert_eq!(metrics.output_tokens, 503);
        assert_eq!(metrics.total_tokens, 2598);
        assert_eq!(metrics.cache_creation_input_tokens, 100);
        assert_eq!(metrics.cache_read_input_tokens, 50);
        assert_eq!(metrics.cost_usd, Some(0.0042));
        assert_eq!(metrics.duration_ms, 1500);
        assert_eq!(metrics.num_turns, 3);
        assert_eq!(metrics.session_id, "test-session-123");
    }

    #[test]
    fn test_usage_metrics_missing_usage() {
        let result = ResultMessage {
            subtype: "query_complete".to_string(),
            duration_ms: 1500,
            duration_api_ms: 1200,
            is_error: false,
            num_turns: 3,
            session_id: "test-session-123".to_string(),
            total_cost_usd: None,
            usage: None,
            result: None,
        };

        let metrics = UsageMetrics::from_result_message(&result);

        assert_eq!(metrics.input_tokens, 0);
        assert_eq!(metrics.output_tokens, 0);
        assert_eq!(metrics.total_tokens, 0);
        assert_eq!(metrics.cost_usd, None);
    }

    #[test]
    fn test_format_slack_message() {
        let metrics = UsageMetrics {
            input_tokens: 2095,
            output_tokens: 503,
            total_tokens: 2598,
            cache_creation_input_tokens: 100,
            cache_read_input_tokens: 50,
            cost_usd: Some(0.0042),
            duration_ms: 1500,
            duration_api_ms: 1200,
            num_turns: 3,
            session_id: "test-session-123".to_string(),
        };

        let message = metrics.format_slack_message();

        assert!(message.contains("2095 input"));
        assert!(message.contains("503 output"));
        assert!(message.contains("2598 total"));
        assert!(message.contains("$0.0042 USD"));
        assert!(message.contains("1.50s"));
        assert!(message.contains("1.20s"));
        assert!(message.contains("Turns: 3"));
        assert!(message.contains("test-session-123"));
        assert!(message.contains("100 created"));
        assert!(message.contains("50 read"));
    }

    #[test]
    fn test_format_slack_message_no_cost() {
        let metrics = UsageMetrics {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            cost_usd: None,
            duration_ms: 500,
            duration_api_ms: 400,
            num_turns: 1,
            session_id: "test-session".to_string(),
        };

        let message = metrics.format_slack_message();

        assert!(message.contains("N/A"));
        assert!(!message.contains("Cache:"));
    }
}
