use crate::compression::token_estimation::TokenEstimator;
use crate::config::CompressionConfig;
use crate::llm::{Message as LlmMessage, Role};

const FRACTION_SCALE: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncationResult {
    pub dropped_messages: usize,
}

#[derive(Debug, Default, Clone)]
pub struct TopDownTruncator {
    estimator: TokenEstimator,
}

impl TopDownTruncator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            estimator: TokenEstimator::new(),
        }
    }

    #[must_use]
    pub fn truncate(
        &self,
        messages: &mut Vec<LlmMessage>,
        _target_ratio: f64,
        context_window: usize,
        config: &CompressionConfig,
    ) -> TruncationResult {
        let target_tokens = scaled_fraction_count(context_window, config.truncation_target);
        let preserve_system = messages
            .first()
            .is_some_and(|message| matches!(message.role, Role::System));
        let mut dropped = 0;

        while messages.len() > config.min_keep_messages
            && self.estimator.estimate_llm_context_tokens(messages) > target_tokens
        {
            let remove_index = usize::from(preserve_system);
            if remove_index >= messages.len() {
                break;
            }

            let remove_count = tool_exchange_prefix_len(&messages[remove_index..]);
            if messages.len().saturating_sub(remove_count) < config.min_keep_messages {
                break;
            }

            messages.drain(remove_index..remove_index + remove_count);
            dropped += remove_count;
        }

        TruncationResult {
            dropped_messages: dropped,
        }
    }
}

#[must_use]
fn scaled_fraction_count(total: usize, fraction: f64) -> usize {
    let fraction = fraction.clamp(0.0, 1.0);
    let scaled = (fraction * 1_000.0).round().to_string();
    let scaled = scaled.parse::<usize>().unwrap_or(FRACTION_SCALE);
    total.saturating_mul(scaled) / FRACTION_SCALE
}

fn tool_exchange_prefix_len(messages: &[LlmMessage]) -> usize {
    match messages {
        [first, second, ..]
            if matches!(first.role, Role::Assistant)
                && first.has_tool_uses()
                && matches!(second.role, Role::User)
                && second.has_tool_results() =>
        {
            2
        }
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_preserves_system_message() {
        let config = CompressionConfig {
            truncation_target: 0.01,
            min_keep_messages: 2,
            ..CompressionConfig::default()
        };
        let mut messages = vec![
            LlmMessage::system("system prompt"),
            LlmMessage::user("one".repeat(200)),
            LlmMessage::assistant("two".repeat(200)),
            LlmMessage::user("three".repeat(200)),
        ];

        let result = TopDownTruncator::new().truncate(&mut messages, 0.5, 100, &config);

        assert!(matches!(messages[0].role, Role::System));
        assert!(result.dropped_messages > 0);
    }

    #[test]
    fn truncation_preserves_tool_exchange_boundaries() {
        let config = CompressionConfig {
            truncation_target: 0.01,
            min_keep_messages: 2,
            ..CompressionConfig::default()
        };
        let mut messages = vec![
            LlmMessage::assistant("tool call").with_tool_uses(vec![
                crate::llm::tools::ToolUse::new(
                    "tool-1",
                    "read_file",
                    serde_json::json!({"path":"/tmp/file.txt"}),
                ),
            ]),
            LlmMessage::user("tool result").with_tool_results(vec![
                crate::llm::tools::ToolResult::success("tool-1", "result payload"),
            ]),
            LlmMessage::assistant("tail".repeat(200)),
            LlmMessage::user("keep".repeat(200)),
        ];

        let result = TopDownTruncator::new().truncate(&mut messages, 0.5, 100, &config);

        assert!(result.dropped_messages >= 2);
        assert!(
            messages
                .first()
                .is_some_and(|message| !message.has_tool_results()),
            "truncation should not leave a tool result orphaned at the front of history"
        );
        assert!(
            messages
                .first()
                .is_none_or(|message| !message.has_tool_uses()),
            "truncation should not leave a tool call without its paired tool result"
        );
    }
}
