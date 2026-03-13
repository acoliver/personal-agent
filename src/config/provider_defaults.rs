use crate::registry::RegistryCache;
use std::collections::HashMap;

pub const OPENAI_API_BASE_URL: &str = "https://api.openai.com/v1";
pub const ANTHROPIC_API_BASE_URL: &str = "https://api.anthropic.com/v1";
pub const SYNTHETIC_API_BASE_URL: &str = "https://api.synthetic.new/v1";

fn builtin_provider_api_url(provider_id: &str) -> Option<&'static str> {
    match provider_id.trim() {
        "anthropic" => Some(ANTHROPIC_API_BASE_URL),
        "openai" => Some(OPENAI_API_BASE_URL),
        "synthetic" => Some(SYNTHETIC_API_BASE_URL),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "moonshotai" => Some("https://api.moonshot.ai/v1"),
        "moonshotai-cn" => Some("https://api.moonshot.cn/v1"),
        "kimi-for-coding" => Some("https://api.kimi.com/coding/v1"),
        _ => None,
    }
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

    builtin_provider_api_url(provider_id).map(str::to_string)
}

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
