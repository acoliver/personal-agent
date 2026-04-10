use crate::models::{AuthConfig, ModelParameters, ModelProfile};
use serde_json::{Map, Value};
use std::path::Path;
use uuid::Uuid;

pub(crate) fn parse_legacy_profile<LegacyId, ParseAuth, ParseParameters>(
    value: &Value,
    path: &Path,
    legacy_profile_id_for_path: LegacyId,
    parse_auth_from_legacy: ParseAuth,
    parse_parameters_from_legacy: ParseParameters,
) -> Option<ModelProfile>
where
    LegacyId: Fn(&Path) -> Uuid,
    ParseAuth: Fn(Option<&Value>) -> AuthConfig,
    ParseParameters: Fn(Option<&Value>, Option<&Value>) -> ModelParameters,
{
    let obj = value.as_object()?;

    if obj.contains_key("provider_id") || obj.contains_key("model_id") {
        return Some(parse_modern_legacy_profile(
            obj,
            path,
            &legacy_profile_id_for_path,
            &parse_auth_from_legacy,
            &parse_parameters_from_legacy,
        ));
    }

    if obj.contains_key("provider") && obj.contains_key("model") {
        return Some(parse_classic_legacy_profile(
            obj,
            path,
            &legacy_profile_id_for_path,
            &parse_auth_from_legacy,
            &parse_parameters_from_legacy,
        ));
    }

    None
}

fn parse_modern_legacy_profile<LegacyId, ParseAuth, ParseParameters>(
    obj: &Map<String, Value>,
    path: &Path,
    legacy_profile_id_for_path: &LegacyId,
    parse_auth_from_legacy: &ParseAuth,
    parse_parameters_from_legacy: &ParseParameters,
) -> ModelProfile
where
    LegacyId: Fn(&Path) -> Uuid,
    ParseAuth: Fn(Option<&Value>) -> AuthConfig,
    ParseParameters: Fn(Option<&Value>, Option<&Value>) -> ModelParameters,
{
    let ephemeral = obj.get("ephemeralSettings");
    ModelProfile {
        id: legacy_profile_id_for_path(path),
        name: modern_legacy_name(obj, path),
        provider_id: modern_legacy_provider_id(obj),
        model_id: modern_legacy_model_id(obj),
        base_url: modern_legacy_base_url(obj),
        auth: parse_auth_from_legacy(obj.get("auth")),
        parameters: parse_parameters_from_legacy(obj.get("parameters"), ephemeral),
        system_prompt: obj
            .get("system_prompt")
            .and_then(Value::as_str)
            .unwrap_or(crate::models::profile::DEFAULT_SYSTEM_PROMPT)
            .to_string(),
        context_window_size: 128_000,
    }
}

fn parse_classic_legacy_profile<LegacyId, ParseAuth, ParseParameters>(
    obj: &Map<String, Value>,
    path: &Path,
    legacy_profile_id_for_path: &LegacyId,
    parse_auth_from_legacy: &ParseAuth,
    parse_parameters_from_legacy: &ParseParameters,
) -> ModelProfile
where
    LegacyId: Fn(&Path) -> Uuid,
    ParseAuth: Fn(Option<&Value>) -> AuthConfig,
    ParseParameters: Fn(Option<&Value>, Option<&Value>) -> ModelParameters,
{
    let provider_id = obj
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("openai")
        .to_string();
    let model_id = obj
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("gpt-4")
        .to_string();
    let ephemeral = obj.get("ephemeralSettings");

    ModelProfile {
        id: legacy_profile_id_for_path(path),
        name: format!("{provider_id}:{model_id}"),
        provider_id,
        model_id,
        base_url: classic_legacy_base_url(ephemeral),
        auth: parse_auth_from_legacy(obj.get("auth")),
        parameters: parse_parameters_from_legacy(obj.get("modelParams"), ephemeral),
        system_prompt: crate::models::profile::DEFAULT_SYSTEM_PROMPT.to_string(),
        context_window_size: 128_000,
    }
}

fn modern_legacy_name(obj: &Map<String, Value>, path: &Path) -> String {
    obj.get("name").and_then(Value::as_str).map_or_else(
        || {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Legacy Profile")
                .to_string()
        },
        ToOwned::to_owned,
    )
}

fn modern_legacy_provider_id(obj: &Map<String, Value>) -> String {
    obj.get("provider_id")
        .and_then(Value::as_str)
        .map_or_else(|| "openai".to_string(), ToOwned::to_owned)
}

fn modern_legacy_model_id(obj: &Map<String, Value>) -> String {
    obj.get("model_id")
        .or_else(|| obj.get("model"))
        .and_then(Value::as_str)
        .map_or_else(|| "gpt-4".to_string(), ToOwned::to_owned)
}

fn modern_legacy_base_url(obj: &Map<String, Value>) -> String {
    obj.get("base_url")
        .or_else(|| obj.get("api_base_url"))
        .and_then(Value::as_str)
        .map_or_else(
            || "https://api.openai.com/v1".to_string(),
            ToOwned::to_owned,
        )
}

fn classic_legacy_base_url(ephemeral: Option<&Value>) -> String {
    ephemeral
        .and_then(Value::as_object)
        .and_then(|e| e.get("base-url"))
        .and_then(Value::as_str)
        .unwrap_or("https://api.openai.com/v1")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_legacy_profile;
    use crate::models::{profile::DEFAULT_SYSTEM_PROMPT, AuthConfig, ModelParameters};
    use serde_json::{json, Value};
    use std::path::Path;
    use uuid::Uuid;

    fn legacy_id_for_path(path: &Path) -> Uuid {
        Uuid::new_v5(&Uuid::NAMESPACE_URL, path.to_string_lossy().as_bytes())
    }

    fn parse_auth(value: Option<&Value>) -> AuthConfig {
        let label = value
            .and_then(Value::as_object)
            .and_then(|obj| obj.get("Keychain"))
            .and_then(Value::as_object)
            .and_then(|obj| obj.get("label"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_default();

        AuthConfig::Keychain { label }
    }

    fn parse_parameters(value: Option<&Value>, ephemeral: Option<&Value>) -> ModelParameters {
        let mut params = ModelParameters::default();

        if let Some(obj) = value.and_then(Value::as_object) {
            if let Some(v) = obj.get("temperature").and_then(Value::as_f64) {
                params.temperature = v;
            }
            if let Some(v) = obj.get("top_p").and_then(Value::as_f64) {
                params.top_p = v;
            }
            if let Some(v) = obj
                .get("max_tokens")
                .or_else(|| obj.get("maxOutputTokens"))
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
            {
                params.max_tokens = v;
            }
            if let Some(v) = obj.get("enable_thinking").and_then(Value::as_bool) {
                params.enable_thinking = v;
            }
            if let Some(v) = obj.get("show_thinking").and_then(Value::as_bool) {
                params.show_thinking = v;
            }
            if let Some(v) = obj
                .get("thinking_budget")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
            {
                params.thinking_budget = Some(v);
            }
        }

        if let Some(obj) = ephemeral.and_then(Value::as_object) {
            if let Some(v) = obj
                .get("max_tokens")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
            {
                params.max_tokens = v;
            }
            if let Some(v) = obj.get("reasoning.enabled").and_then(Value::as_bool) {
                params.enable_thinking = v;
            }
            if let Some(v) = obj
                .get("reasoning.includeInResponse")
                .and_then(Value::as_bool)
            {
                params.show_thinking = v;
            }
        }

        params
    }

    #[test]
    fn parse_legacy_profile_returns_none_for_non_profile_payloads() {
        let parsed = parse_legacy_profile(
            &json!({ "unexpected": true }),
            Path::new("/tmp/unknown.json"),
            legacy_id_for_path,
            parse_auth,
            parse_parameters,
        );

        assert!(parsed.is_none());
    }

    #[test]
    fn parse_legacy_profile_maps_modern_shape() {
        let path = Path::new("/tmp/custom-profile.json");
        let parsed = parse_legacy_profile(
            &json!({
                "name": "Modern Profile",
                "provider_id": "anthropic",
                "model_id": "claude-3-7-sonnet",
                "base_url": "https://api.anthropic.com/v1",
                "system_prompt": "Be helpful.",
                "auth": {
                    "Keychain": {
                        "label": "anthropic-key"
                    }
                },
                "parameters": {
                    "temperature": 0.2,
                    "top_p": 0.8,
                    "max_tokens": 2048,
                    "enable_thinking": true,
                    "show_thinking": true,
                    "thinking_budget": 512
                }
            }),
            path,
            legacy_id_for_path,
            parse_auth,
            parse_parameters,
        )
        .expect("modern payload should parse");

        assert_eq!(parsed.id, legacy_id_for_path(path));
        assert_eq!(parsed.name, "Modern Profile");
        assert_eq!(parsed.provider_id, "anthropic");
        assert_eq!(parsed.model_id, "claude-3-7-sonnet");
        assert_eq!(parsed.base_url, "https://api.anthropic.com/v1");
        assert_eq!(parsed.system_prompt, "Be helpful.");
        assert_eq!(
            parsed.auth,
            AuthConfig::Keychain {
                label: "anthropic-key".to_string()
            }
        );
        assert!((parsed.parameters.temperature - 0.2).abs() < f64::EPSILON);
        assert!((parsed.parameters.top_p - 0.8).abs() < f64::EPSILON);
        assert_eq!(parsed.parameters.max_tokens, 2048);
        assert!(parsed.parameters.enable_thinking);
        assert!(parsed.parameters.show_thinking);
        assert_eq!(parsed.parameters.thinking_budget, Some(512));
    }

    #[test]
    fn parse_legacy_profile_uses_modern_fallbacks_from_path_and_ephemeral_key_name() {
        let path = Path::new("/tmp/fallback-name.json");
        let parsed = parse_legacy_profile(
            &json!({
                "provider_id": "",
                "model": "fallback-model",
                "api_base_url": "https://fallback.example/v1",
                "ephemeralSettings": {
                    "auth-key-name": "resolved-key"
                }
            }),
            path,
            legacy_id_for_path,
            parse_auth,
            parse_parameters,
        )
        .expect("fallback payload should parse");

        assert_eq!(parsed.name, "fallback-name");
        assert_eq!(parsed.provider_id, "");
        assert_eq!(parsed.model_id, "fallback-model");
        assert_eq!(parsed.base_url, "https://fallback.example/v1");
        assert_eq!(
            parsed.auth,
            AuthConfig::Keychain {
                label: String::new()
            }
        );
        assert_eq!(parsed.system_prompt, DEFAULT_SYSTEM_PROMPT);
    }

    #[test]
    fn parse_legacy_profile_maps_classic_shape_and_ephemeral_overrides() {
        let path = Path::new("/tmp/classic.json");
        let parsed = parse_legacy_profile(
            &json!({
                "provider": "openai",
                "model": "gpt-4o-mini",
                "modelParams": {
                    "temperature": 0.55,
                    "top_p": 0.91,
                    "maxOutputTokens": 777,
                    "thinking_budget": 128
                },
                "ephemeralSettings": {
                    "base-url": "https://classic.example/v1",
                    "auth-keyfile": "/tmp/keyfile.txt",
                    "max_tokens": 888,
                    "reasoning.enabled": true,
                    "reasoning.includeInResponse": true
                }
            }),
            path,
            legacy_id_for_path,
            parse_auth,
            parse_parameters,
        )
        .expect("classic payload should parse");

        assert_eq!(parsed.id, legacy_id_for_path(path));
        assert_eq!(parsed.name, "openai:gpt-4o-mini");
        assert_eq!(parsed.provider_id, "openai");
        assert_eq!(parsed.model_id, "gpt-4o-mini");
        assert_eq!(parsed.base_url, "https://classic.example/v1");
        assert_eq!(
            parsed.auth,
            AuthConfig::Keychain {
                label: String::new()
            }
        );
        assert!((parsed.parameters.temperature - 0.55).abs() < f64::EPSILON);
        assert!((parsed.parameters.top_p - 0.91).abs() < f64::EPSILON);
        assert_eq!(parsed.parameters.max_tokens, 888);
        assert!(parsed.parameters.enable_thinking);
        assert!(parsed.parameters.show_thinking);
        assert_eq!(parsed.parameters.thinking_budget, Some(128));
        assert_eq!(parsed.system_prompt, DEFAULT_SYSTEM_PROMPT);
    }

    #[test]
    fn parse_legacy_profile_uses_defaults_for_minimal_modern_payload() {
        let parsed = parse_legacy_profile(
            &json!({
                "provider_id": "",
                "name": "",
                "parameters": {}
            }),
            Path::new("/tmp/minimal.json"),
            legacy_id_for_path,
            parse_auth,
            parse_parameters,
        )
        .expect("minimal payload should parse");

        assert_eq!(parsed.name, "");
        assert_eq!(parsed.provider_id, "");
        assert_eq!(parsed.model_id, "gpt-4");
        assert_eq!(parsed.base_url, "https://api.openai.com/v1");
        assert_eq!(
            parsed.auth,
            AuthConfig::Keychain {
                label: String::new()
            }
        );
        assert_eq!(parsed.parameters, ModelParameters::default());
        assert_eq!(parsed.system_prompt, DEFAULT_SYSTEM_PROMPT);
    }
}
