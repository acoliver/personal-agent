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

        Self { client, api_url }
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
