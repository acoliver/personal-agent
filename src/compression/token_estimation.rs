use crate::llm::Message as LlmMessage;
use crate::models::Message;
use tiktoken_rs::{cl100k_base, CoreBPE};

const MESSAGE_OVERHEAD_TOKENS: usize = 4;

#[derive(Clone)]
pub struct TokenEstimator {
    tokenizer: CoreBPE,
}

impl std::fmt::Debug for TokenEstimator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenEstimator").finish()
    }
}

impl TokenEstimator {
    /// Create a token estimator backed by the `cl100k_base` tokenizer.
    ///
    /// # Panics
    ///
    /// Panics if the tokenizer cannot be initialized.
    #[must_use]
    pub fn new() -> Self {
        let tokenizer = cl100k_base().expect("cl100k_base tokenizer should initialize");
        Self { tokenizer }
    }

    #[must_use]
    pub fn estimate_message_tokens(&self, message: &Message) -> usize {
        let tool_calls = message.tool_calls.as_deref().unwrap_or_default();
        let tool_results = message.tool_results.as_deref().unwrap_or_default();

        self.count_text_tokens(&message.content)
            + message
                .thinking_content
                .as_deref()
                .map_or(0, |thinking| self.count_text_tokens(thinking))
            + self.count_text_tokens(tool_calls)
            + self.count_text_tokens(tool_results)
            + MESSAGE_OVERHEAD_TOKENS
    }

    #[must_use]
    pub fn estimate_llm_message_tokens(&self, message: &LlmMessage) -> usize {
        let tool_calls = serde_json::to_string(&message.tool_uses).unwrap_or_default();
        let tool_results = serde_json::to_string(&message.tool_results).unwrap_or_default();

        self.count_text_tokens(&message.content)
            + message
                .thinking_content
                .as_deref()
                .map_or(0, |thinking| self.count_text_tokens(thinking))
            + self.count_text_tokens(&tool_calls)
            + self.count_text_tokens(&tool_results)
            + MESSAGE_OVERHEAD_TOKENS
    }

    #[must_use]
    pub fn estimate_context_tokens(&self, messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|message| self.estimate_message_tokens(message))
            .sum()
    }

    #[must_use]
    pub fn estimate_llm_context_tokens(&self, messages: &[LlmMessage]) -> usize {
        messages
            .iter()
            .map(|message| self.estimate_llm_message_tokens(message))
            .sum()
    }

    #[must_use]
    pub fn usage_ratio(&self, messages: &[LlmMessage], context_window: usize) -> f64 {
        if context_window == 0 {
            return 1.0;
        }

        let estimated_tokens = self.estimate_llm_context_tokens(messages);
        let estimated_tokens = estimated_tokens
            .to_string()
            .parse::<f64>()
            .unwrap_or(f64::MAX);
        let context_window = context_window.to_string().parse::<f64>().unwrap_or(1.0);
        estimated_tokens / context_window
    }

    #[must_use]
    fn count_text_tokens(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }

        self.tokenizer.encode_ordinary(text).len()
    }
}

impl Default for TokenEstimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{tools::ToolResult, tools::ToolUse, Message as LlmMessage};
    use crate::models::{Message, MessageRole};
    use chrono::Utc;

    #[test]
    fn estimate_message_tokens_counts_all_persisted_fields() {
        let estimator = TokenEstimator::new();
        let message = Message {
            role: MessageRole::Assistant,
            content: "hello world".to_string(),
            thinking_content: Some("internal reasoning".to_string()),
            timestamp: Utc::now(),
            model_id: None,
            tool_calls: Some(
                serde_json::to_string(&vec![ToolUse::new(
                    "tool-1",
                    "read_file",
                    serde_json::json!({"path":"/tmp/demo"}),
                )])
                .expect("tool calls should serialize"),
            ),
            tool_results: Some(
                serde_json::to_string(&vec![ToolResult::success("tool-1", "result body")])
                    .expect("tool results should serialize"),
            ),
        };

        assert!(estimator.estimate_message_tokens(&message) > MESSAGE_OVERHEAD_TOKENS);
    }

    #[test]
    fn estimate_llm_message_tokens_counts_tool_payloads_and_thinking() {
        let estimator = TokenEstimator::new();
        let message = LlmMessage::assistant("answer")
            .with_thinking("reasoning")
            .with_tool_uses(vec![ToolUse::new(
                "tool-2",
                "search",
                serde_json::json!({"query":"rust"}),
            )])
            .with_tool_results(vec![ToolResult::success("tool-2", "search output")]);

        assert!(estimator.estimate_llm_message_tokens(&message) > MESSAGE_OVERHEAD_TOKENS);
    }
}
