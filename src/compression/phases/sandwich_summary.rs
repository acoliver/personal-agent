use crate::config::CompressionConfig;
use crate::llm::{Message as LlmMessage, Role};
use anyhow::{anyhow, Result};

const SUMMARY_CONTINUATION_DIRECTIVE: &str =
    "Continue the conversation using the preserved facts, context, and tool history below.";
const FRACTION_SCALE: usize = 1_000;

#[derive(Debug, Clone)]
pub struct SummaryResult {
    pub messages: Vec<LlmMessage>,
    pub summary_range: Option<(usize, usize)>,
    pub preserved_facts: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct SandwichSummarizer;

impl SandwichSummarizer {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Summarize the middle portion of a message list into a structured system message.
    ///
    /// # Errors
    ///
    /// Returns an error when there are too few messages to summarize or when a valid
    /// middle window cannot be formed from the configured preservation rules.
    pub fn summarize(
        &self,
        messages: &[LlmMessage],
        config: &CompressionConfig,
    ) -> Result<SummaryResult> {
        if messages.len() < config.min_middle_messages + 2 {
            return Err(anyhow!("not enough messages to summarize"));
        }

        let top_count = scaled_fraction_count(messages.len(), config.preserve_top_fraction).max(1);
        let bottom_count =
            scaled_fraction_count(messages.len(), config.preserve_bottom_fraction).max(1);
        let mut middle_start = top_count.min(messages.len());
        let mut middle_end = messages.len().saturating_sub(bottom_count);

        if middle_end <= middle_start {
            return Err(anyhow!("no middle range available for summarization"));
        }

        while middle_end.saturating_sub(middle_start) < config.min_middle_messages {
            if middle_start > 0 {
                middle_start -= 1;
            } else if middle_end < messages.len() {
                middle_end += 1;
            } else {
                return Err(anyhow!("unable to form summary window"));
            }
        }

        let middle_messages = &messages[middle_start..middle_end];
        let preserved_facts = collect_preserved_facts(middle_messages);
        let summary_text = render_structured_summary(middle_messages, &preserved_facts);
        let summary_message = LlmMessage {
            role: Role::System,
            content: summary_text,
            thinking_content: None,
            tool_uses: Vec::new(),
            tool_results: Vec::new(),
        };

        let mut summarized = Vec::new();
        summarized.extend_from_slice(&messages[..middle_start]);
        summarized.push(summary_message);
        summarized.extend_from_slice(&messages[middle_end..]);

        Ok(SummaryResult {
            messages: summarized,
            summary_range: Some((middle_start, middle_end)),
            preserved_facts,
        })
    }
}

#[must_use]
fn scaled_fraction_count(total: usize, fraction: f64) -> usize {
    let fraction = fraction.clamp(0.0, 1.0);
    let scaled = (fraction * 1_000.0).round().to_string();
    let scaled = scaled.parse::<usize>().unwrap_or(FRACTION_SCALE);
    total.saturating_mul(scaled) / FRACTION_SCALE
}

fn collect_preserved_facts(messages: &[LlmMessage]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| {
            let content = message.content.trim();
            if content.is_empty() {
                None
            } else {
                Some(content.lines().next().unwrap_or(content).to_string())
            }
        })
        .take(8)
        .collect()
}

fn render_structured_summary(messages: &[LlmMessage], preserved_facts: &[String]) -> String {
    let context = messages
        .iter()
        .filter(|message| !message.content.trim().is_empty())
        .map(|message| format!("- {}", message.content.trim()))
        .collect::<Vec<_>>()
        .join("\n");

    let tool_history = messages
        .iter()
        .flat_map(|message| message.tool_uses.iter())
        .map(|tool| format!("- {} {}", tool.name, tool.input))
        .collect::<Vec<_>>()
        .join("\n");

    let facts = if preserved_facts.is_empty() {
        "- None captured".to_string()
    } else {
        preserved_facts
            .iter()
            .map(|fact| format!("- {fact}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "<facts>\n{facts}\n</facts>\n<context>\n{context}\n</context>\n<tool_history>\n{}\n</tool_history>\n<summary>\n{}\n</summary>",
        if tool_history.is_empty() {
            "- None".to_string()
        } else {
            tool_history
        },
        SUMMARY_CONTINUATION_DIRECTIVE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_middle_messages_with_structured_sections() {
        let config = CompressionConfig {
            preserve_top_fraction: 0.2,
            preserve_bottom_fraction: 0.2,
            min_middle_messages: 2,
            ..CompressionConfig::default()
        };
        let messages = vec![
            LlmMessage::system("system"),
            LlmMessage::user("user one"),
            LlmMessage::assistant("assistant one"),
            LlmMessage::user("user two"),
            LlmMessage::assistant("assistant two"),
        ];

        let result = SandwichSummarizer::new()
            .summarize(&messages, &config)
            .expect("summary should succeed");

        assert!(result
            .messages
            .iter()
            .any(|message| message.content.contains("<facts>")));
        assert!(result.summary_range.is_some());
        assert!(!result.preserved_facts.is_empty());
    }
}
