use crate::compression::phases::observation_masking::ObservationMasker;
use crate::compression::phases::sandwich_summary::SandwichSummarizer;
use crate::compression::phases::truncation::TopDownTruncator;
use crate::compression::token_estimation::TokenEstimator;
use crate::config::CompressionConfig;
use crate::llm::Message as LlmMessage;
use crate::models::CompressionPhase;

#[derive(Debug, Clone)]
pub struct CompressionResult {
    pub messages: Vec<LlmMessage>,
    pub phase: CompressionPhase,
    pub masked_tool_seqs: Option<Vec<usize>>,
    pub summary_range: Option<(usize, usize)>,
    pub preserved_facts: Option<Vec<String>>,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone, Default)]
pub struct CompressionPipeline {
    estimator: TokenEstimator,
}

impl CompressionPipeline {
    #[must_use]
    pub fn new() -> Self {
        Self {
            estimator: TokenEstimator::new(),
        }
    }

    #[must_use]
    pub fn compress(
        &self,
        mut messages: Vec<LlmMessage>,
        context_window: usize,
        config: &CompressionConfig,
    ) -> CompressionResult {
        let mut phase = CompressionPhase::None;
        let mut masked_tool_seqs = None;
        let mut summary_range = None;
        let mut preserved_facts = None;

        let mut usage_ratio = self.estimator.usage_ratio(&messages, context_window);
        if usage_ratio >= config.observation_mask_threshold {
            let masking = ObservationMasker::new(config).mask_observations(&mut messages);
            if !masking.masked_message_indices.is_empty() {
                phase = CompressionPhase::ObservationMasked;
                masked_tool_seqs = Some(masking.masked_message_indices);
            }
            usage_ratio = self.estimator.usage_ratio(&messages, context_window);
        }

        if usage_ratio >= config.sandwich_summary_threshold {
            if let Ok(summary) = SandwichSummarizer::new().summarize(&messages, config) {
                messages = summary.messages;
                phase = CompressionPhase::Summarized;
                summary_range = summary.summary_range;
                if !summary.preserved_facts.is_empty() {
                    preserved_facts = Some(summary.preserved_facts);
                }
            }
            usage_ratio = self.estimator.usage_ratio(&messages, context_window);
        }

        if usage_ratio >= config.truncation_threshold {
            let mut truncated_messages = messages;
            let truncation = TopDownTruncator::new().truncate(
                &mut truncated_messages,
                usage_ratio,
                context_window,
                config,
            );
            if truncation.dropped_messages > 0 {
                phase = CompressionPhase::Truncated;
            }
            messages = truncated_messages;
        }

        CompressionResult {
            estimated_tokens: self.estimator.estimate_llm_context_tokens(&messages),
            messages,
            phase,
            masked_tool_seqs,
            summary_range,
            preserved_facts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::tools::{ToolResult, ToolUse};

    #[test]
    fn pipeline_masks_when_usage_crosses_first_threshold() {
        let config = CompressionConfig {
            observation_mask_threshold: 0.0,
            sandwich_summary_threshold: 2.0,
            truncation_threshold: 3.0,
            mask_recent_count: 0,
            mask_size_threshold: 1,
            ..CompressionConfig::default()
        };
        let messages = vec![LlmMessage::assistant("")
            .with_tool_uses(vec![ToolUse::new(
                "tool-1",
                "read_file",
                serde_json::json!({}),
            )])
            .with_tool_results(vec![ToolResult::success("tool-1", "12345678910")])];

        let result = CompressionPipeline::new().compress(messages, 1_000, &config);

        assert_eq!(result.phase, CompressionPhase::ObservationMasked);
        assert_eq!(result.masked_tool_seqs, Some(vec![0]));
    }
}
