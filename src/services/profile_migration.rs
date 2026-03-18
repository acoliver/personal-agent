use crate::models::{AuthConfig, ModelParameters, ModelProfile};
use serde_json::{Map, Value};
use std::path::Path;
use uuid::Uuid;

pub(crate) fn parse_legacy_profile<LegacyId, ParseKeyName, ParseAuth, ParseParameters>(
    value: &Value,
    path: &Path,
    legacy_profile_id_for_path: LegacyId,
    parse_legacy_auth_key_name: ParseKeyName,
    parse_auth_from_legacy: ParseAuth,
    parse_parameters_from_legacy: ParseParameters,
) -> Option<ModelProfile>
where
    LegacyId: Fn(&Path) -> Uuid,
    ParseKeyName: Fn(Option<&Value>) -> Option<String>,
    ParseAuth: Fn(Option<&Value>, Option<String>) -> AuthConfig,
    ParseParameters: Fn(Option<&Value>, Option<&Value>) -> ModelParameters,
{
    let obj = value.as_object()?;

    if obj.contains_key("provider_id") || obj.contains_key("model_id") {
        return Some(parse_modern_legacy_profile(
            obj,
            path,
            &legacy_profile_id_for_path,
            &parse_legacy_auth_key_name,
            &parse_auth_from_legacy,
            &parse_parameters_from_legacy,
        ));
    }

    if obj.contains_key("provider") && obj.contains_key("model") {
        return Some(parse_classic_legacy_profile(
            obj,
            path,
            &legacy_profile_id_for_path,
            &parse_legacy_auth_key_name,
            &parse_auth_from_legacy,
            &parse_parameters_from_legacy,
        ));
    }

    None
}

fn parse_modern_legacy_profile<LegacyId, ParseKeyName, ParseAuth, ParseParameters>(
    obj: &Map<String, Value>,
    path: &Path,
    legacy_profile_id_for_path: &LegacyId,
    parse_legacy_auth_key_name: &ParseKeyName,
    parse_auth_from_legacy: &ParseAuth,
    parse_parameters_from_legacy: &ParseParameters,
) -> ModelProfile
where
    LegacyId: Fn(&Path) -> Uuid,
    ParseKeyName: Fn(Option<&Value>) -> Option<String>,
    ParseAuth: Fn(Option<&Value>, Option<String>) -> AuthConfig,
    ParseParameters: Fn(Option<&Value>, Option<&Value>) -> ModelParameters,
{
    let ephemeral = obj.get("ephemeralSettings");
    ModelProfile {
        id: legacy_profile_id_for_path(path),
        name: modern_legacy_name(obj, path),
        provider_id: modern_legacy_provider_id(obj),
        model_id: modern_legacy_model_id(obj),
        base_url: modern_legacy_base_url(obj),
        auth: parse_auth_from_legacy(
            obj.get("auth"),
            legacy_keyfile_hint(ephemeral, parse_legacy_auth_key_name),
        ),
        parameters: parse_parameters_from_legacy(obj.get("parameters"), ephemeral),
        system_prompt: obj
            .get("system_prompt")
            .and_then(Value::as_str)
            .unwrap_or(crate::models::profile::DEFAULT_SYSTEM_PROMPT)
            .to_string(),
    }
}

fn parse_classic_legacy_profile<LegacyId, ParseKeyName, ParseAuth, ParseParameters>(
    obj: &Map<String, Value>,
    path: &Path,
    legacy_profile_id_for_path: &LegacyId,
    parse_legacy_auth_key_name: &ParseKeyName,
    parse_auth_from_legacy: &ParseAuth,
    parse_parameters_from_legacy: &ParseParameters,
) -> ModelProfile
where
    LegacyId: Fn(&Path) -> Uuid,
    ParseKeyName: Fn(Option<&Value>) -> Option<String>,
    ParseAuth: Fn(Option<&Value>, Option<String>) -> AuthConfig,
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
        auth: parse_auth_from_legacy(
            obj.get("auth"),
            legacy_keyfile_hint(ephemeral, parse_legacy_auth_key_name),
        ),
        parameters: parse_parameters_from_legacy(obj.get("modelParams"), ephemeral),
        system_prompt: crate::models::profile::DEFAULT_SYSTEM_PROMPT.to_string(),
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

fn legacy_keyfile_hint<ParseKeyName>(
    ephemeral: Option<&Value>,
    parse_legacy_auth_key_name: &ParseKeyName,
) -> Option<String>
where
    ParseKeyName: Fn(Option<&Value>) -> Option<String>,
{
    ephemeral
        .and_then(Value::as_object)
        .and_then(|e| e.get("auth-keyfile"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| parse_legacy_auth_key_name(ephemeral))
}
