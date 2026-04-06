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
            messages.remove(remove_index);
            dropped += 1;
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
}
