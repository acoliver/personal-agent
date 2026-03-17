use crate::config::provider_api_url;
use crate::models::ModelProfile;
use crate::registry::{ModelRegistry, Provider, RegistryCache};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use std::collections::HashMap;

const KIMI_PROVIDER_ID: &str = "kimi-for-coding";
const KIMI_USER_AGENT: &str = "RooCode/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ProviderQuirks {
    pub serdes_provider: Option<String>,
    pub base_url_override: Option<String>,
    pub headers: HashMap<String, String>,
}

impl ProviderQuirks {
    pub fn has_custom_headers(&self) -> bool {
        !self.headers.is_empty()
    }

    pub fn header_map(&self) -> Result<HeaderMap, String> {
        let mut map = HeaderMap::new();

        for (name, value) in &self.headers {
            let header_value = HeaderValue::from_str(value).map_err(|err| err.to_string())?;

            if name.eq_ignore_ascii_case("user-agent") {
                map.insert(USER_AGENT, header_value);
                continue;
            }

            let header_name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|err| err.to_string())?;
            map.insert(header_name, header_value);
        }

        Ok(map)
    }
}

pub(crate) fn resolve_provider_quirks(profile: &ModelProfile) -> ProviderQuirks {
    let registry = load_registry();
    resolve_provider_quirks_with_registry(profile, registry.as_ref())
}

pub(crate) fn resolve_provider_quirks_with_registry(
    profile: &ModelProfile,
    registry: Option<&ModelRegistry>,
) -> ProviderQuirks {
    let mut quirks = ProviderQuirks::default();

    if profile.provider_id == KIMI_PROVIDER_ID {
        quirks.serdes_provider = Some("openai".to_string());
        quirks.base_url_override = provider_api_url(KIMI_PROVIDER_ID);
        quirks
            .headers
            .insert("User-Agent".to_string(), KIMI_USER_AGENT.to_string());
        return quirks;
    }

    if let Some(provider) = registry.and_then(|loaded| loaded.providers.get(&profile.provider_id)) {
        if provider_uses_openai_compatible_transport(provider) {
            quirks.serdes_provider = Some("openai".to_string());
        }
    }

    quirks
}

pub(crate) fn effective_serdes_provider(
    profile: &ModelProfile,
    registry: Option<&ModelRegistry>,
) -> &'static str {
    if let Some(provider) = resolve_provider_quirks_with_registry(profile, registry).serdes_provider
    {
        match provider.as_str() {
            "openai" => return "openai",
            "anthropic" => return "anthropic",
            "groq" => return "groq",
            "mistral" => return "mistral",
            _ => {}
        }
    }

    match profile.provider_id.as_str() {
        "anthropic" | "claude" => "anthropic",
        "groq" => "groq",
        "mistral" => "mistral",
        _ => "openai",
    }
}

fn provider_uses_openai_compatible_transport(provider: &Provider) -> bool {
    provider
        .npm
        .as_deref()
        .is_some_and(|npm| npm.contains("openai-compatible"))
}

fn load_registry() -> Option<ModelRegistry> {
    let cache_path = RegistryCache::default_path().ok()?;
    let cache = RegistryCache::new(cache_path, 24);
    cache.load().ok()?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, ModelProfile};
    use crate::registry::ModelInfo;

    fn profile(provider_id: &str, model_id: &str) -> ModelProfile {
        ModelProfile::new(
            "Test".to_string(),
            provider_id.to_string(),
            model_id.to_string(),
            String::new(),
            AuthConfig::Keychain {
                label: "test".to_string(),
            },
        )
    }

    fn registry_with_provider(provider_id: &str, npm: Option<&str>) -> ModelRegistry {
        let mut providers = HashMap::new();
        providers.insert(
            provider_id.to_string(),
            Provider {
                id: provider_id.to_string(),
                name: provider_id.to_string(),
                env: vec!["TEST_API_KEY".to_string()],
                npm: npm.map(str::to_string),
                api: None,
                doc: None,
                models: HashMap::<String, ModelInfo>::new(),
            },
        );
        ModelRegistry { providers }
    }

    #[test]
    fn kimi_provider_adds_required_user_agent_and_base_url() {
        let quirks = resolve_provider_quirks_with_registry(
            &profile(KIMI_PROVIDER_ID, KIMI_PROVIDER_ID),
            None,
        );

        assert_eq!(quirks.serdes_provider.as_deref(), Some("openai"));
        assert_eq!(
            quirks.base_url_override.as_deref(),
            Some("https://api.kimi.com/coding/v1")
        );
        assert_eq!(
            quirks.headers.get("User-Agent").map(String::as_str),
            Some(KIMI_USER_AGENT)
        );
    }

    #[test]
    fn openai_compatible_registry_provider_maps_to_openai_transport() {
        let registry = registry_with_provider("synthetic", Some("@ai-sdk/openai-compatible"));
        let quirks = resolve_provider_quirks_with_registry(
            &profile("synthetic", "foo-model"),
            Some(&registry),
        );

        assert_eq!(quirks.serdes_provider.as_deref(), Some("openai"));
        assert!(quirks.headers.is_empty());
        assert!(quirks.base_url_override.is_none());
    }

    #[test]
    fn native_anthropic_provider_keeps_native_transport() {
        let registry = registry_with_provider("anthropic", Some("@ai-sdk/anthropic"));

        assert_eq!(
            effective_serdes_provider(&profile("anthropic", "claude-sonnet"), Some(&registry)),
            "anthropic"
        );
    }

    #[test]
    fn quirks_header_map_preserves_user_agent_header() {
        let quirks = resolve_provider_quirks_with_registry(
            &profile(KIMI_PROVIDER_ID, KIMI_PROVIDER_ID),
            None,
        );
        let header_map = quirks.header_map().expect("header map");

        assert_eq!(
            header_map
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some(KIMI_USER_AGENT)
        );
    }
}
