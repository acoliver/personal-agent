//! Model capability detection.
//!
//! Determines what features a local model supports based on its
//! architecture and size. Used to set appropriate defaults and
//! manage user expectations.

use serde::{Deserialize, Serialize};

/// Capabilities of a local model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelCapabilities {
    /// Whether the model supports tool/function calling.
    pub supports_tools: bool,
    /// Recommended context window size in tokens.
    pub recommended_context: usize,
    /// Maximum supported context window size in tokens.
    pub max_context: usize,
    /// Human-readable model name.
    pub display_name: String,
    /// Model identifier.
    pub model_id: String,
}

impl ModelCapabilities {
    /// Get capabilities for a known model by ID.
    ///
    /// Returns conservative defaults for unknown models.
    #[must_use]
    pub fn for_model(model_id: &str) -> Self {
        match model_id.to_lowercase().as_str() {
            // Qwen3.5-4B
            "qwen3.5-4b" | "qwen3.5-4b-instruct" | "qwen3.5-4b-q4_k_m" => Self {
                // 4B model: tool calling is unreliable, disabled by default
                supports_tools: false,
                recommended_context: 32_768, // 32K
                max_context: 262_144,        // 256K
                display_name: "Qwen3.5-4B (Q4_K_M)".to_string(),
                model_id: "qwen3.5-4b".to_string(),
            },

            // Qwen3.5-9B and larger
            "qwen3.5-9b" | "qwen3.5-14b" | "qwen3.5-32b" => Self {
                supports_tools: true, // Larger models support tools reliably
                recommended_context: 32_768,
                max_context: 262_144,
                display_name: model_id.to_string(),
                model_id: model_id.to_string(),
            },

            // Qwen2.5 series
            "qwen2.5-3b" | "qwen2.5-7b" => Self {
                supports_tools: false,
                recommended_context: 32_768,
                max_context: 131_072, // 128K for Qwen2.5
                display_name: model_id.to_string(),
                model_id: model_id.to_string(),
            },

            // Default: conservative settings
            _ => Self {
                supports_tools: false,
                recommended_context: 32_768,
                max_context: 32_768,
                display_name: model_id.to_string(),
                model_id: model_id.to_string(),
            },
        }
    }

    /// Get capabilities for the default bundled model.
    #[must_use]
    pub fn for_default() -> Self {
        Self::for_model("qwen3.5-4b")
    }

    /// Check if a context window size is valid for this model.
    #[must_use]
    pub const fn is_valid_context(&self, context: usize) -> bool {
        context > 0 && context <= self.max_context
    }

    /// Clamp a context window size to the valid range.
    #[must_use]
    pub fn clamp_context(&self, context: usize) -> usize {
        context.clamp(1024, self.max_context)
    }
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self::for_default()
    }
}

/// Tool calling reliability level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolReliability {
    /// Not supported - tool calls will likely fail.
    Unsupported,
    /// Low reliability (~40% for multi-tool).
    Low,
    /// Medium reliability (~70%).
    Medium,
    /// High reliability (~90%+).
    High,
}

impl ToolReliability {
    /// Get a human-readable description.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Unsupported => "Tool calling not supported",
            Self::Low => "Tool calling is experimental and may be unreliable",
            Self::Medium => "Tool calling works but may occasionally fail",
            Self::High => "Tool calling is reliable",
        }
    }

    /// Whether tools should be enabled by default.
    #[must_use]
    pub const fn should_enable_by_default(&self) -> bool {
        matches!(self, Self::High)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model_capabilities() {
        let caps = ModelCapabilities::for_default();
        assert_eq!(caps.model_id, "qwen3.5-4b");
        assert!(!caps.supports_tools); // Disabled by default for 4B
        assert_eq!(caps.recommended_context, 32_768);
        assert_eq!(caps.max_context, 262_144);
    }

    #[test]
    fn test_large_model_supports_tools() {
        let caps = ModelCapabilities::for_model("qwen3.5-14b");
        assert!(caps.supports_tools);
    }

    #[test]
    fn test_unknown_model_defaults() {
        let caps = ModelCapabilities::for_model("unknown-model");
        assert!(!caps.supports_tools);
        assert_eq!(caps.recommended_context, 32_768);
    }

    #[test]
    fn test_context_validation() {
        let caps = ModelCapabilities::for_default();
        assert!(caps.is_valid_context(32_768));
        assert!(caps.is_valid_context(262_144));
        assert!(!caps.is_valid_context(300_000)); // Exceeds max
        assert!(!caps.is_valid_context(0));
    }

    #[test]
    fn test_context_clamping() {
        let caps = ModelCapabilities::for_default();
        assert_eq!(caps.clamp_context(500_000), 262_144); // Clamped to max
        assert_eq!(caps.clamp_context(0), 1024); // Clamped to min
        assert_eq!(caps.clamp_context(65_536), 65_536); // Unchanged
    }
}
