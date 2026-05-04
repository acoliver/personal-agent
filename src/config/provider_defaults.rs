use crate::registry::RegistryCache;
use std::collections::HashMap;

use super::quirks_manifest::quirks_manifest;

pub const OPENAI_API_BASE_URL: &str = "https://api.openai.com/v1";

fn builtin_provider_api_url(provider_id: &str) -> Option<String> {
    quirks_manifest()
        .get(provider_id)
        .and_then(|entry| entry.base_url.clone())
}

pub fn provider_api_url(provider_id: &str) -> Option<String> {
    let provider_id = provider_id.trim();
    if provider_id.is_empty() {
        return None;
    }

    if let Ok(cache_path) = RegistryCache::default_path() {
        if let Ok(content) = std::fs::read_to_string(cache_path) {
            if let Ok(cached) = serde_json::from_str::<crate::registry::CachedRegistry>(&content) {
                if let Some(api) = cached
                    .data
                    .get_provider(provider_id)
                    .and_then(|provider| provider.api.as_deref())
                    .map(str::trim)
                    .filter(|url| !url.is_empty())
                {
                    return Some(api.to_string());
                }
            }
        }
    }

    builtin_provider_api_url(provider_id)
}

#[must_use]
pub fn default_api_base_url_for_provider(provider_id: &str) -> String {
    provider_api_url(provider_id).unwrap_or_else(|| OPENAI_API_BASE_URL.to_string())
}

pub fn provider_api_url_map(
    provider_ids: impl IntoIterator<Item = String>,
) -> HashMap<String, String> {
    provider_ids
        .into_iter()
        .filter_map(|provider_id| provider_api_url(&provider_id).map(|url| (provider_id, url)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_provider_defaults_include_zai_coding_endpoint() {
        assert_eq!(
            builtin_provider_api_url("zai-coding-plan").as_deref(),
            Some("https://api.z.ai/api/coding/paas/v4")
        );
    }

    #[test]
    fn bundled_provider_defaults_include_zai_general_endpoint() {
        assert_eq!(
            builtin_provider_api_url("zai").as_deref(),
            Some("https://api.z.ai/api/paas/v4")
        );
    }
}
