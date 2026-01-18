//! Type definitions for the models.dev registry

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Custom deserializer for fields that can be bool or object (interleaved)
fn deserialize_bool_or_object<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct BoolOrObjectVisitor;

    impl<'de> Visitor<'de> for BoolOrObjectVisitor {
        type Value = bool;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a boolean or an object")
        }

        fn visit_bool<E>(self, v: bool) -> Result<bool, E>
        where
            E: de::Error,
        {
            Ok(v)
        }

        fn visit_map<M>(self, mut map: M) -> Result<bool, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            // If it's an object (like {"field": "reasoning_content"}), treat as true
            // Just consume the map
            while let Some((_, _)) = map.next_entry::<String, serde_json::Value>()? {}
            Ok(true)
        }
    }

    deserializer.deserialize_any(BoolOrObjectVisitor)
}

/// Custom deserializer for provider field that can be string, object, or null
fn deserialize_provider<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct ProviderVisitor;

    impl<'de> Visitor<'de> for ProviderVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string, an object, or null")
        }

        fn visit_none<E>(self) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Option<String>, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_any(Self)
        }

        fn visit_str<E>(self, v: &str) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(Some(v.to_string()))
        }

        fn visit_string<E>(self, v: String) -> Result<Option<String>, E>
        where
            E: de::Error,
        {
            Ok(Some(v))
        }

        fn visit_map<M>(self, mut map: M) -> Result<Option<String>, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            // If it's an object (like {"npm": "@ai-sdk/anthropic"}), ignore it
            while let Some((_, _)) = map.next_entry::<String, serde_json::Value>()? {}
            Ok(None)
        }
    }

    deserializer.deserialize_any(ProviderVisitor)
}

/// Top-level registry containing all providers
///
/// When loading from cache, the data structure is:
/// ```json
/// {
///   "cached_at": "...",
///   "data": {
///     "provider_id": { ... provider ... },
///     ...
///   }
/// }
/// ```
///
/// The `data` field deserializes into this struct via `#[serde(flatten)]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelRegistry {
    /// Map of provider ID to provider information
    #[serde(flatten)]
    pub providers: HashMap<String, Provider>,
}

/// Provider information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provider {
    /// Provider ID
    pub id: String,
    /// Human-readable provider name
    pub name: String,
    /// Environment variable names for authentication
    #[serde(default)]
    pub env: Vec<String>,
    /// NPM package name (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm: Option<String>,
    /// API endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    /// Documentation URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    /// Models available from this provider
    pub models: HashMap<String, ModelInfo>,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ModelInfo {
    /// Model ID
    pub id: String,
    /// Human-readable model name
    pub name: String,
    /// Model family (e.g., "gpt", "claude")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// Whether the model supports attachments
    #[serde(default)]
    pub attachment: bool,
    /// Whether the model has reasoning capabilities
    #[serde(default)]
    pub reasoning: bool,
    /// Whether the model supports tool/function calling
    #[serde(default)]
    pub tool_call: bool,
    /// Whether the model supports structured output
    #[serde(default)]
    pub structured_output: bool,
    /// Whether the model supports temperature parameter
    #[serde(default)]
    pub temperature: bool,
    /// Whether the model supports interleaved content
    /// Can be a boolean or an object with a "field" key
    #[serde(default, deserialize_with = "deserialize_bool_or_object")]
    pub interleaved: bool,
    /// Provider name for the model (can be string or object with npm field)
    #[serde(default, deserialize_with = "deserialize_provider")]
    pub provider: Option<String>,
    /// Model status (e.g., "active", "deprecated")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Knowledge cutoff date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub knowledge: Option<String>,
    /// Model release date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    /// Last update date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    /// Input and output modalities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Modalities>,
    /// Whether model has open weights
    #[serde(default)]
    pub open_weights: bool,
    /// Cost information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<Cost>,
    /// Context and output limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<Limit>,
}

/// Modalities supported by a model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Modalities {
    /// Input modalities (e.g., "text", "image", "audio")
    #[serde(default)]
    pub input: Vec<String>,
    /// Output modalities
    #[serde(default)]
    pub output: Vec<String>,
}

/// Cost information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cost {
    /// Input cost (per token or unit)
    pub input: f64,
    /// Output cost (per token or unit)
    pub output: f64,
    /// Cache read cost (per token or unit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
}

/// Context and output limits
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Limit {
    /// Maximum context window size
    pub context: u64,
    /// Maximum output tokens
    pub output: u64,
}

impl ModelRegistry {
    /// Get all provider IDs
    #[must_use]
    pub fn get_provider_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.providers.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Get a provider by ID
    #[must_use]
    pub fn get_provider(&self, provider_id: &str) -> Option<&Provider> {
        self.providers.get(provider_id)
    }

    /// Get all models for a specific provider
    #[must_use]
    pub fn get_models_for_provider(&self, provider_id: &str) -> Option<Vec<&ModelInfo>> {
        self.providers.get(provider_id).map(|provider| {
            let mut models: Vec<&ModelInfo> = provider.models.values().collect();
            models.sort_by(|a, b| a.id.cmp(&b.id));
            models
        })
    }

    /// Get a specific model from a provider
    #[must_use]
    pub fn get_model(&self, provider_id: &str, model_id: &str) -> Option<&ModelInfo> {
        self.providers
            .get(provider_id)
            .and_then(|provider| provider.models.get(model_id))
    }

    /// Search for models by criteria
    pub fn search_models<F>(&self, predicate: F) -> Vec<(&str, &ModelInfo)>
    where
        F: Fn(&ModelInfo) -> bool,
    {
        let mut results = Vec::new();
        for (provider_id, provider) in &self.providers {
            for model in provider.models.values() {
                if predicate(model) {
                    results.push((provider_id.as_str(), model));
                }
            }
        }
        results.sort_by(|a, b| a.1.id.cmp(&b.1.id));
        results
    }

    /// Get all models with tool calling capability
    #[must_use]
    pub fn get_tool_call_models(&self) -> Vec<(&str, &ModelInfo)> {
        self.search_models(|model| model.tool_call)
    }

    /// Get all models with reasoning capability
    #[must_use]
    pub fn get_reasoning_models(&self) -> Vec<(&str, &ModelInfo)> {
        self.search_models(|model| model.reasoning)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_registry() -> ModelRegistry {
        let mut providers = HashMap::new();

        let mut models = HashMap::new();
        models.insert(
            "model-1".to_string(),
            ModelInfo {
                id: "model-1".to_string(),
                name: "Model 1".to_string(),
                family: Some("test".to_string()),
                attachment: false,
                reasoning: true,
                tool_call: true,
                structured_output: true,
                temperature: true,
                interleaved: false,
                provider: None,
                status: None,
                knowledge: Some("2024-01".to_string()),
                release_date: Some("2024-01-01".to_string()),
                last_updated: Some("2024-01-01".to_string()),
                modalities: Some(Modalities {
                    input: vec!["text".to_string()],
                    output: vec!["text".to_string()],
                }),
                open_weights: false,
                cost: Some(Cost {
                    input: 0.01,
                    output: 0.02,
                    cache_read: None,
                }),
                limit: Some(Limit {
                    context: 128000,
                    output: 4096,
                }),
            },
        );

        models.insert(
            "model-2".to_string(),
            ModelInfo {
                id: "model-2".to_string(),
                name: "Model 2".to_string(),
                family: Some("test".to_string()),
                attachment: true,
                reasoning: false,
                tool_call: false,
                structured_output: false,
                temperature: true,
                interleaved: false,
                provider: None,
                status: None,
                knowledge: Some("2024-02".to_string()),
                release_date: Some("2024-02-01".to_string()),
                last_updated: Some("2024-02-01".to_string()),
                modalities: Some(Modalities {
                    input: vec!["text".to_string(), "image".to_string()],
                    output: vec!["text".to_string()],
                }),
                open_weights: true,
                cost: Some(Cost {
                    input: 0.0,
                    output: 0.0,
                    cache_read: None,
                }),
                limit: Some(Limit {
                    context: 8192,
                    output: 2048,
                }),
            },
        );

        providers.insert(
            "test-provider".to_string(),
            Provider {
                id: "test-provider".to_string(),
                name: "Test Provider".to_string(),
                env: vec!["TEST_API_KEY".to_string()],
                npm: Some("@test/sdk".to_string()),
                api: Some("https://api.test.com/v1".to_string()),
                doc: Some("https://docs.test.com".to_string()),
                models,
            },
        );

        ModelRegistry { providers }
    }

    #[test]
    fn test_get_provider_ids() {
        let registry = create_test_registry();
        let ids = registry.get_provider_ids();
        assert_eq!(ids, vec!["test-provider"]);
    }

    #[test]
    fn test_get_provider() {
        let registry = create_test_registry();
        let provider = registry.get_provider("test-provider");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name, "Test Provider");
    }

    #[test]
    fn test_get_provider_not_found() {
        let registry = create_test_registry();
        let provider = registry.get_provider("nonexistent");
        assert!(provider.is_none());
    }

    #[test]
    fn test_get_models_for_provider() {
        let registry = create_test_registry();
        let models = registry.get_models_for_provider("test-provider");
        assert!(models.is_some());
        let models = models.unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "model-1");
        assert_eq!(models[1].id, "model-2");
    }

    #[test]
    fn test_get_model() {
        let registry = create_test_registry();
        let model = registry.get_model("test-provider", "model-1");
        assert!(model.is_some());
        assert_eq!(model.unwrap().name, "Model 1");
    }

    #[test]
    fn test_get_tool_call_models() {
        let registry = create_test_registry();
        let models = registry.get_tool_call_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].1.id, "model-1");
    }

    #[test]
    fn test_get_reasoning_models() {
        let registry = create_test_registry();
        let models = registry.get_reasoning_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].1.id, "model-1");
    }

    #[test]
    fn test_search_models() {
        let registry = create_test_registry();
        let models = registry.search_models(|model| model.attachment);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].1.id, "model-2");
    }
}
