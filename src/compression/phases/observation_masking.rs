use crate::config::CompressionConfig;
use crate::llm::Message as LlmMessage;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservationMaskingResult {
    pub masked_message_indices: Vec<usize>,
}

pub struct ObservationMasker<'a> {
    config: &'a CompressionConfig,
}

impl<'a> ObservationMasker<'a> {
    #[must_use]
    pub const fn new(config: &'a CompressionConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn mask_observations(&self, messages: &mut [LlmMessage]) -> ObservationMaskingResult {
        let cutoff = messages.len().saturating_sub(self.config.mask_recent_count);
        let mut masked_message_indices = Vec::new();

        for (index, message) in messages.iter_mut().enumerate().take(cutoff) {
            if message.tool_results.is_empty() {
                continue;
            }

            let total_chars: usize = message
                .tool_results
                .iter()
                .map(|result| result.content.chars().count())
                .sum();
            if total_chars < self.config.mask_size_threshold {
                continue;
            }

            let tool_name = message
                .tool_uses
                .last()
                .map_or_else(|| "tool".to_string(), |tool| tool.name.clone());
            let placeholder = format!(
                "[Tool result from {tool_name}: {total_chars} chars, output masked for context efficiency]"
            );

            for result in &mut message.tool_results {
                result.content.clone_from(&placeholder);
            }
            if message.content.is_empty() {
                message.content = placeholder;
            }
            masked_message_indices.push(index);
        }

        ObservationMaskingResult {
            masked_message_indices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::tools::{ToolResult, ToolUse};

    #[test]
    fn masks_old_large_tool_results_but_keeps_recent_tail() {
        let config = CompressionConfig {
            mask_recent_count: 1,
            mask_size_threshold: 10,
            ..CompressionConfig::default()
        };
        let mut messages = vec![
            LlmMessage::assistant("")
                .with_tool_uses(vec![ToolUse::new(
                    "tool-1",
                    "read_file",
                    serde_json::json!({}),
                )])
                .with_tool_results(vec![ToolResult::success("tool-1", "12345678910")]),
            LlmMessage::assistant("")
                .with_tool_uses(vec![ToolUse::new(
                    "tool-2",
                    "search",
                    serde_json::json!({}),
                )])
                .with_tool_results(vec![ToolResult::success("tool-2", "12345678910")]),
        ];

        let result = ObservationMasker::new(&config).mask_observations(&mut messages);

        assert_eq!(result.masked_message_indices, vec![0]);
        assert!(messages[0].tool_results[0]
            .content
            .contains("masked for context efficiency"));
        assert_eq!(messages[1].tool_results[0].content, "12345678910");
    }
}
