//! Client for fetching model registry from models.dev

use crate::error::{AppError, Result};
use crate::registry::types::ModelRegistry;

const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";

/// Client for interacting with the models.dev API
pub struct ModelsDevClient {
    client: reqwest::Client,
    api_url: String,
}

impl ModelsDevClient {
    /// Create a new client with the default API URL
    #[must_use] 
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        
        Self {
            client,
            api_url: MODELS_DEV_API_URL.to_string(),
        }
    }

    /// Create a new client with a custom API URL (useful for testing)
    #[must_use] 
    pub fn with_url(api_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        
        Self {
            client,
            api_url,
        }
    }

    /// Fetch the model registry from models.dev
    ///
    /// # Errors
    ///
    /// Returns `AppError::Network` if the HTTP request fails or the response status is not successful.
    /// Returns `AppError::Storage` if the response cannot be parsed as JSON.
    pub async fn fetch_registry(&self) -> Result<ModelRegistry> {
        let response = self
            .client
            .get(&self.api_url)
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Failed to fetch registry: {e}")))?;

        if !response.status().is_success() {
            return Err(AppError::Network(format!(
                "Failed to fetch registry: HTTP {}",
                response.status()
            )));
        }

        let registry: ModelRegistry = response
            .json()
            .await
            .map_err(|e| AppError::Storage(format!("Failed to parse registry: {e}")))?;

        Ok(registry)
    }
}

impl Default for ModelsDevClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_mock_response() -> serde_json::Value {
        serde_json::json!({
            "test-provider": {
                "id": "test-provider",
                "name": "Test Provider",
                "env": ["TEST_API_KEY"],
                "npm": "@test/sdk",
                "api": "https://api.test.com/v1",
                "doc": "https://docs.test.com",
                "models": {
                    "test-model-1": {
                        "id": "test-model-1",
                        "name": "Test Model 1",
                        "family": "test",
                        "attachment": true,
                        "reasoning": false,
                        "tool_call": true,
                        "structured_output": true,
                        "temperature": true,
                        "interleaved": false,
                        "provider": "test-provider",
                        "status": "active",
                        "knowledge": "2024-01",
                        "release_date": "2024-01-01",
                        "last_updated": "2024-01-01",
                        "modalities": {
                            "input": ["text", "image"],
                            "output": ["text"]
                        },
                        "open_weights": false,
                        "cost": {
                            "input": 0.01,
                            "output": 0.02,
                            "cache_read": 0.005
                        },
                        "limit": {
                            "context": 128000,
                            "output": 4096
                        }
                    },
                    "test-model-2": {
                        "id": "test-model-2",
                        "name": "Test Model 2",
                        "family": "test",
                        "attachment": false,
                        "reasoning": true,
                        "tool_call": false,
                        "structured_output": false,
                        "temperature": true,
                        "interleaved": false,
                        "knowledge": "2024-02",
                        "release_date": "2024-02-01",
                        "last_updated": "2024-02-01",
                        "modalities": {
                            "input": ["text"],
                            "output": ["text"]
                        },
                        "open_weights": true,
                        "cost": {
                            "input": 0.0,
                            "output": 0.0
                        },
                        "limit": {
                            "context": 8192,
                            "output": 2048
                        }
                    }
                }
            },
            "another-provider": {
                "id": "another-provider",
                "name": "Another Provider",
                "env": ["ANOTHER_API_KEY"],
                "api": "https://api.another.com",
                "models": {
                    "simple-model": {
                        "id": "simple-model",
                        "name": "Simple Model",
                        "attachment": false,
                        "reasoning": false,
                        "tool_call": false,
                        "structured_output": false,
                        "temperature": true,
                        "interleaved": false,
                        "open_weights": false
                    }
                }
            }
        })
    }

    #[tokio::test]
    async fn test_fetch_registry_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(create_mock_response()))
            .mount(&mock_server)
            .await;

        let client = ModelsDevClient::with_url(format!("{}/api.json", mock_server.uri()));
        let registry = client.fetch_registry().await.unwrap();

        assert_eq!(registry.providers.len(), 2);
        assert!(registry.providers.contains_key("test-provider"));
        assert!(registry.providers.contains_key("another-provider"));

        let test_provider = registry.get_provider("test-provider").unwrap();
        assert_eq!(test_provider.name, "Test Provider");
        assert_eq!(test_provider.models.len(), 2);

        let model1 = test_provider.models.get("test-model-1").unwrap();
        assert_eq!(model1.name, "Test Model 1");
        assert!(model1.attachment);
        assert!(model1.tool_call);
        assert_eq!(model1.family, Some("test".to_string()));
    }

    #[tokio::test]
    async fn test_fetch_registry_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api.json"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = ModelsDevClient::with_url(format!("{}/api.json", mock_server.uri()));
        let result = client.fetch_registry().await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("HTTP 500"));
    }

    #[tokio::test]
    async fn test_fetch_registry_invalid_json() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string("invalid json"))
            .mount(&mock_server)
            .await;

        let client = ModelsDevClient::with_url(format!("{}/api.json", mock_server.uri()));
        let result = client.fetch_registry().await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to parse registry"));
    }

    #[tokio::test]
    async fn test_fetch_registry_with_helper_methods() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(create_mock_response()))
            .mount(&mock_server)
            .await;

        let client = ModelsDevClient::with_url(format!("{}/api.json", mock_server.uri()));
        let registry = client.fetch_registry().await.unwrap();

        let provider_ids = registry.get_provider_ids();
        assert_eq!(provider_ids.len(), 2);
        assert!(provider_ids.contains(&"test-provider".to_string()));
        assert!(provider_ids.contains(&"another-provider".to_string()));

        let models = registry.get_models_for_provider("test-provider").unwrap();
        assert_eq!(models.len(), 2);

        let model = registry.get_model("test-provider", "test-model-1").unwrap();
        assert_eq!(model.name, "Test Model 1");

        let tool_models = registry.get_tool_call_models();
        assert_eq!(tool_models.len(), 1);
        assert_eq!(tool_models[0].1.id, "test-model-1");

        let reasoning_models = registry.get_reasoning_models();
        assert_eq!(reasoning_models.len(), 1);
        assert_eq!(reasoning_models[0].1.id, "test-model-2");
    }
}
