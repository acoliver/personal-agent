use personal_agent::models::profile::DEFAULT_SYSTEM_PROMPT;
use personal_agent::presentation::view_command::{ApiKeyInfo, ViewCommand};
use personal_agent::ui_gpui::views::{ApiType, AuthMethod, ProfileEditorData, ProfileEditorState};
use uuid::Uuid;

fn apply_model_selected(
    state: &mut ProfileEditorState,
    provider_id: &str,
    model_id: &str,
    provider_api_url: Option<&str>,
    context_length: Option<u32>,
) {
    state.is_new = true;
    state.data.model_id = model_id.to_string();
    state.data.api_type = match provider_id {
        "anthropic" => ApiType::Anthropic,
        "openai" => ApiType::OpenAI,
        other => ApiType::Custom(other.to_string()),
    };
    if state.data.name.trim().is_empty() {
        state.data.name = model_id.to_string();
    }
    if state.data.base_url.trim().is_empty() {
        state.data.base_url = provider_api_url
            .filter(|url| !url.trim().is_empty())
            .map_or_else(
                || personal_agent::config::default_api_base_url_for_provider(provider_id),
                ToString::to_string,
            );
    }
    if let Some(limit) = context_length {
        state.data.context_limit = limit;
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_profile_editor_load(
    state: &mut ProfileEditorState,
    id: Uuid,
    name: &str,
    provider_id: &str,
    model_id: &str,
    base_url: &str,
    api_key_label: &str,
    temperature: f64,
    max_tokens: Option<u32>,
    max_tokens_field_name: &str,
    extra_request_fields: &str,
    context_limit: Option<u32>,
    show_thinking: bool,
    enable_thinking: bool,
    thinking_budget: Option<u32>,
    system_prompt: &str,
) {
    state.is_new = false;
    state.data.id = Some(id.to_string());
    state.data.name = name.to_string();
    state.data.model_id = model_id.to_string();
    state.data.base_url = base_url.to_string();
    state.data.api_type = match provider_id {
        "anthropic" => ApiType::Anthropic,
        "openai" => ApiType::OpenAI,
        other => ApiType::Custom(other.to_string()),
    };
    state.data.key_label = api_key_label.to_string();
    #[allow(clippy::cast_possible_truncation)]
    {
        state.data.temperature = temperature as f32;
    }
    state.data.max_tokens = max_tokens.map_or_else(String::new, |value| value.to_string());
    state.data.max_tokens_field_name = max_tokens_field_name.to_string();
    state.data.extra_request_fields = extra_request_fields.to_string();
    if let Some(limit) = context_limit {
        state.data.context_limit = limit;
    }
    state.data.show_thinking = show_thinking;
    state.data.enable_extended_thinking = enable_thinking;
    state.data.thinking_budget = thinking_budget.unwrap_or(10_000);
    state.data.system_prompt = system_prompt.to_string();
}

fn apply_api_keys_listed(state: &mut ProfileEditorState, keys: Vec<ApiKeyInfo>) {
    state.data.available_keys = keys.into_iter().map(|key| key.label).collect();
}

fn cycle_api_type(data: &mut ProfileEditorData) {
    data.api_type = match data.api_type.clone() {
        ApiType::Anthropic => ApiType::OpenAI,
        ApiType::OpenAI => ApiType::Local,
        ApiType::Local | ApiType::Custom(_) => ApiType::Anthropic,
    };

    if data.base_url.trim().is_empty() {
        data.base_url =
            personal_agent::config::default_api_base_url_for_provider(&match &data.api_type {
                ApiType::Anthropic => "anthropic".to_string(),
                ApiType::OpenAI => "openai".to_string(),
                ApiType::Local => "local".to_string(),
                ApiType::Custom(provider) => provider.clone(),
            });
    }
}

fn emit_save_payload(data: &ProfileEditorData) -> personal_agent::events::types::ModelProfile {
    let id = data
        .id
        .as_deref()
        .and_then(|raw| Uuid::parse_str(raw).ok())
        .unwrap_or_else(Uuid::new_v4);

    let auth = if data.api_type.requires_api_key() {
        Some(personal_agent::events::types::ModelProfileAuth::Keychain {
            label: data.key_label.clone(),
        })
    } else {
        Some(personal_agent::events::types::ModelProfileAuth::None)
    };

    let parsed_max_tokens = data.max_tokens.parse::<u32>().ok();

    // Normalize max_tokens_field_name: empty or the default "max_tokens" sentinel means None
    let max_tokens_field_name = {
        let name = data.max_tokens_field_name.trim();
        if name.is_empty() || name == "max_tokens" {
            None
        } else {
            Some(name.to_string())
        }
    };

    personal_agent::events::types::ModelProfile {
        id,
        name: data.name.clone(),
        provider_id: Some(match &data.api_type {
            ApiType::Anthropic => "anthropic".to_string(),
            ApiType::OpenAI => "openai".to_string(),
            ApiType::Local => "local".to_string(),
            ApiType::Custom(provider) => provider.clone(),
        }),
        model_id: Some(data.model_id.clone()),
        base_url: Some(data.base_url.clone()),
        auth,
        parameters: Some(personal_agent::events::types::ModelProfileParameters {
            temperature: Some(f64::from(data.temperature)),
            max_tokens: parsed_max_tokens,
            max_tokens_field_name,
            extra_request_fields: serde_json::from_str(&data.extra_request_fields).ok(),

            show_thinking: Some(data.show_thinking),
            enable_thinking: Some(data.enable_extended_thinking),
            thinking_budget: if data.enable_extended_thinking {
                Some(data.thinking_budget)
            } else {
                None
            },
            context_window_size: Some(data.context_limit as usize),
        }),
        system_prompt: Some(data.system_prompt.clone()),
    }
}

#[test]
fn auth_method_and_api_type_display_match_expected_labels() {
    assert_eq!(AuthMethod::default(), AuthMethod::Keychain);
    assert_eq!(AuthMethod::Keychain.display(), "Keychain");
    assert_eq!(ApiType::default(), ApiType::Anthropic);
    assert_eq!(ApiType::Anthropic.display(), "Anthropic");
    assert_eq!(ApiType::OpenAI.display(), "OpenAI");
    assert_eq!(ApiType::Custom("local".to_string()).display(), "local");
}

#[test]
fn profile_editor_data_new_sets_expected_defaults_and_can_save_validation() {
    let mut data = ProfileEditorData::new();

    assert_eq!(data.id, None);
    assert_eq!(data.name, "");
    assert_eq!(data.model_id, "");
    assert_eq!(data.api_type, ApiType::Anthropic);
    assert_eq!(data.base_url, "");
    assert_eq!(data.key_label, "");
    assert!(data.available_keys.is_empty());
    assert!((data.temperature - 1.0).abs() < f32::EPSILON);
    assert_eq!(data.max_tokens, "4096");
    assert_eq!(data.max_tokens_field_name, "max_tokens");
    assert_eq!(data.context_limit, 128_000);
    assert!(data.show_thinking);
    assert!(!data.enable_extended_thinking);
    assert_eq!(data.thinking_budget, 10_000);
    assert_eq!(data.system_prompt, DEFAULT_SYSTEM_PROMPT);
    assert!(!data.can_save());

    data.name = "Primary".to_string();
    data.model_id = "claude-sonnet-4".to_string();
    data.base_url = "https://api.anthropic.com".to_string();
    data.key_label = "anthropic-key".to_string();
    assert!(data.can_save());

    data.key_label = "  ".to_string();
    assert!(!data.can_save());
    data.key_label = "anthropic-key".to_string();
    data.base_url = String::new();
    assert!(!data.can_save());
    data.base_url = "https://api.anthropic.com".to_string();
    data.model_id = " ".to_string();
    assert!(!data.can_save());
    data.model_id = "claude-sonnet-4".to_string();
    data.name = " ".to_string();
    assert!(!data.can_save());
}

#[test]
fn profile_editor_state_construction_preserves_is_new_and_payloads() {
    let edit_data = ProfileEditorData {
        id: Some(Uuid::new_v4().to_string()),
        name: "Existing".to_string(),
        model_id: "gpt-4o".to_string(),
        api_type: ApiType::OpenAI,
        base_url: "https://api.openai.com/v1".to_string(),
        key_label: "openai-key".to_string(),
        available_keys: vec!["openai-key".to_string()],
        temperature: 0.2,
        max_tokens: "8192".to_string(),
        max_tokens_field_name: "max_completion_tokens".to_string(),
        extra_request_fields: "{}".to_string(),

        context_limit: 200_000,
        show_thinking: false,
        enable_extended_thinking: true,
        thinking_budget: 2048,
        system_prompt: "Be concise".to_string(),
    };

    let new_state = ProfileEditorState::new_profile();
    let edit_state = ProfileEditorState::edit_profile(edit_data.clone());

    assert!(new_state.is_new);
    assert!((new_state.data.temperature - 1.0).abs() < f32::EPSILON);
    assert!(!edit_state.is_new);
    assert_eq!(edit_state.data.id, edit_data.id);
    assert_eq!(edit_state.data.name, "Existing");
    assert_eq!(edit_state.data.api_type, ApiType::OpenAI);
    assert_eq!(
        edit_state.data.max_tokens_field_name,
        "max_completion_tokens"
    );
    assert_eq!(
        edit_state.data.available_keys,
        vec!["openai-key".to_string()]
    );
    assert_eq!(ProfileEditorState::default().data.name, "");
}

#[test]
fn model_selected_prefills_empty_fields_and_preserves_existing_name_and_base_url() {
    let mut state = ProfileEditorState::new_profile();

    apply_model_selected(
        &mut state,
        "anthropic",
        "claude-sonnet-4-20250514",
        Some("https://custom.anthropic.test/v1"),
        Some(200_000),
    );
    assert!(state.is_new);
    assert_eq!(state.data.name, "claude-sonnet-4-20250514");
    assert_eq!(state.data.model_id, "claude-sonnet-4-20250514");
    assert_eq!(state.data.api_type, ApiType::Anthropic);
    assert_eq!(state.data.base_url, "https://custom.anthropic.test/v1");
    assert_eq!(state.data.context_limit, 200_000);

    state.data.name = "Pinned Name".to_string();
    state.data.base_url = "https://already.set".to_string();
    apply_model_selected(
        &mut state,
        "openai",
        "gpt-4o",
        Some("https://ignored"),
        Some(128_000),
    );
    assert_eq!(state.data.name, "Pinned Name");
    assert_eq!(state.data.base_url, "https://already.set");
    assert_eq!(state.data.api_type, ApiType::OpenAI);
    assert_eq!(state.data.model_id, "gpt-4o");
    assert_eq!(state.data.context_limit, 128_000);

    let mut custom_provider = ProfileEditorState::new_profile();
    apply_model_selected(&mut custom_provider, "ollama", "llama3.2", None, None);
    assert_eq!(
        custom_provider.data.api_type,
        ApiType::Custom("ollama".to_string())
    );
    assert_eq!(
        custom_provider.data.base_url,
        personal_agent::config::default_api_base_url_for_provider("ollama")
    );
    assert_eq!(custom_provider.data.context_limit, 128_000);
}

#[test]
fn profile_editor_load_maps_existing_profile_fields_and_defaults_thinking_budget() {
    let id = Uuid::new_v4();
    let id_string = id.to_string();
    let mut state = ProfileEditorState::new_profile();

    apply_profile_editor_load(
        &mut state,
        id,
        "Existing Profile",
        "custom-provider",
        "model-x",
        "https://example.test/v1",
        "stored-key",
        0.35,
        Some(1234),
        "max_completion_tokens",
        "{\"reasoning\":{\"effort\":\"medium\"}}",
        Some(64_000),
        false,
        true,
        None,
        "System prompt body",
    );

    assert!(!state.is_new);
    assert_eq!(state.data.id.as_deref(), Some(id_string.as_str()));
    assert_eq!(state.data.name, "Existing Profile");
    assert_eq!(
        state.data.api_type,
        ApiType::Custom("custom-provider".to_string())
    );
    assert_eq!(state.data.model_id, "model-x");
    assert_eq!(state.data.base_url, "https://example.test/v1");
    assert_eq!(state.data.key_label, "stored-key");
    assert!((state.data.temperature - 0.35_f32).abs() < f32::EPSILON);
    assert_eq!(state.data.max_tokens, "1234");
    assert_eq!(state.data.max_tokens_field_name, "max_completion_tokens");
    assert_eq!(
        state.data.extra_request_fields,
        "{\"reasoning\":{\"effort\":\"medium\"}}"
    );
    assert_eq!(state.data.context_limit, 64_000);
    assert!(!state.data.show_thinking);
    assert!(state.data.enable_extended_thinking);
    assert_eq!(state.data.thinking_budget, 10_000);
    assert_eq!(state.data.system_prompt, "System prompt body");

    apply_profile_editor_load(
        &mut state,
        id,
        "OpenAI Profile",
        "openai",
        "gpt-4.1",
        "https://api.openai.com/v1",
        "openai-key",
        0.1,
        None,
        "max_tokens",
        "{}",
        None,
        true,
        false,
        Some(777),
        "Second prompt",
    );
    assert_eq!(state.data.api_type, ApiType::OpenAI);
    assert_eq!(state.data.max_tokens, "");
    assert_eq!(state.data.max_tokens_field_name, "max_tokens");
    assert_eq!(state.data.extra_request_fields, "{}");
    assert_eq!(state.data.context_limit, 64_000);
    assert_eq!(state.data.thinking_budget, 777);
    assert!(state.data.show_thinking);
    assert!(!state.data.enable_extended_thinking);
}

#[test]
fn api_keys_listed_replaces_available_keys_in_order() {
    let mut state = ProfileEditorState::new_profile();
    state.data.available_keys = vec!["stale".to_string()];

    apply_api_keys_listed(
        &mut state,
        vec![
            ApiKeyInfo {
                label: "anthropic".to_string(),
                masked_value: "sk-a••••".to_string(),
                used_by: vec!["Profile A".to_string()],
            },
            ApiKeyInfo {
                label: "openai".to_string(),
                masked_value: "sk-o••••".to_string(),
                used_by: vec![],
            },
        ],
    );

    assert_eq!(
        state.data.available_keys,
        vec!["anthropic".to_string(), "openai".to_string()]
    );
}

#[test]
fn api_type_cycling_updates_empty_base_url_only() {
    let mut data = ProfileEditorData::new();
    cycle_api_type(&mut data);
    assert_eq!(data.api_type, ApiType::OpenAI);
    assert_eq!(
        data.base_url,
        personal_agent::config::default_api_base_url_for_provider("openai")
    );

    let preserved_url = "https://custom.host/v1".to_string();
    data.base_url = preserved_url.clone();
    cycle_api_type(&mut data);
    assert_eq!(data.api_type, ApiType::Local);
    assert_eq!(data.base_url, preserved_url);

    data.base_url.clear();
    cycle_api_type(&mut data);
    assert_eq!(data.api_type, ApiType::Anthropic);
    assert_eq!(
        data.base_url,
        personal_agent::config::default_api_base_url_for_provider("anthropic")
    );
}

#[test]
fn save_payload_conversion_uses_existing_or_generated_ids_and_thinking_rules() {
    let existing_id = Uuid::new_v4();
    let existing = ProfileEditorData {
        id: Some(existing_id.to_string()),
        name: "Existing".to_string(),
        model_id: "claude-opus-4".to_string(),
        api_type: ApiType::Anthropic,
        base_url: "https://api.anthropic.com/v1".to_string(),
        key_label: "anthropic-key".to_string(),
        available_keys: vec![],
        temperature: 0.4,
        max_tokens: "9000".to_string(),
        max_tokens_field_name: "max_completion_tokens".to_string(),
        extra_request_fields: "{}".to_string(),

        context_limit: 200_000,
        show_thinking: true,
        enable_extended_thinking: true,
        thinking_budget: 512,
        system_prompt: "Use tools when appropriate".to_string(),
    };
    let created = ProfileEditorData {
        id: Some("not-a-uuid".to_string()),
        name: "Created".to_string(),
        model_id: "gpt-4o-mini".to_string(),
        api_type: ApiType::Custom("localai".to_string()),
        base_url: "http://localhost:1234/v1".to_string(),
        key_label: "local-key".to_string(),
        available_keys: vec![],
        temperature: 0.9,
        extra_request_fields: "{}".to_string(),

        max_tokens: String::new(),
        max_tokens_field_name: "max_tokens".to_string(),
        context_limit: 16_000,
        show_thinking: false,
        enable_extended_thinking: false,
        thinking_budget: 999,
        system_prompt: "Custom prompt".to_string(),
    };

    let existing_payload = emit_save_payload(&existing);
    let created_payload = emit_save_payload(&created);

    assert_eq!(existing_payload.id, existing_id);
    assert_eq!(existing_payload.name, "Existing");
    assert_eq!(existing_payload.provider_id.as_deref(), Some("anthropic"));
    assert_eq!(existing_payload.model_id.as_deref(), Some("claude-opus-4"));
    assert_eq!(
        existing_payload.base_url.as_deref(),
        Some("https://api.anthropic.com/v1")
    );
    assert!(matches!(
        existing_payload.auth,
        Some(personal_agent::events::types::ModelProfileAuth::Keychain { ref label }) if label == "anthropic-key"
    ));
    let existing_parameters = existing_payload
        .parameters
        .expect("parameters should exist");
    assert!(
        (existing_parameters
            .temperature
            .expect("temperature present")
            - 0.4)
            .abs()
            < 1e-6
    );
    assert_eq!(existing_parameters.max_tokens, Some(9000));
    assert_eq!(
        existing_parameters.max_tokens_field_name.as_deref(),
        Some("max_completion_tokens")
    );
    assert_eq!(existing_parameters.show_thinking, Some(true));
    assert_eq!(existing_parameters.enable_thinking, Some(true));
    assert_eq!(existing_parameters.thinking_budget, Some(512));
    assert_eq!(
        existing_payload.system_prompt.as_deref(),
        Some("Use tools when appropriate")
    );

    assert_ne!(created_payload.id, existing_id);
    assert_eq!(created_payload.name, "Created");
    assert_eq!(created_payload.provider_id.as_deref(), Some("localai"));
    assert_eq!(created_payload.model_id.as_deref(), Some("gpt-4o-mini"));
    let created_parameters = created_payload.parameters.expect("parameters should exist");
    assert!((created_parameters.temperature.expect("temperature present") - 0.9).abs() < 1e-6);
    assert_eq!(created_parameters.max_tokens, None);
    // "max_tokens" sentinel is normalized to None
    assert_eq!(created_parameters.max_tokens_field_name, None);
    assert_eq!(created_parameters.show_thinking, Some(false));
    assert_eq!(created_parameters.enable_thinking, Some(false));
    assert_eq!(created_parameters.thinking_budget, None);
}

#[test]
fn command_payload_shapes_used_by_profile_editor_match_expectations() {
    let profile_id = Uuid::new_v4();
    let model_selected = ViewCommand::ModelSelected {
        provider_id: "anthropic".to_string(),
        model_id: "claude-3-7-sonnet".to_string(),
        provider_api_url: Some("https://api.anthropic.com/v1".to_string()),
        context_length: Some(200_000),
    };
    let profile_load = ViewCommand::ProfileEditorLoad {
        id: profile_id,
        name: "Loaded".to_string(),
        provider_id: "openai".to_string(),
        model_id: "gpt-4.1".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        api_key_label: "openai-key".to_string(),
        temperature: 0.7,
        max_tokens: Some(8192),
        max_tokens_field_name: "max_completion_tokens".to_string(),
        extra_request_fields: "{\"reasoning\":{\"effort\":\"medium\"}}".to_string(),

        context_limit: Some(128_000),
        show_thinking: false,
        enable_thinking: false,
        thinking_budget: None,
        system_prompt: "prompt".to_string(),
    };

    assert!(matches!(
        model_selected,
        ViewCommand::ModelSelected {
            provider_id,
            model_id,
            provider_api_url: Some(provider_api_url),
            context_length: Some(200_000),
        } if provider_id == "anthropic" && model_id == "claude-3-7-sonnet" && provider_api_url == "https://api.anthropic.com/v1"
    ));
    assert!(matches!(
        profile_load,
        ViewCommand::ProfileEditorLoad {
            id,
            name,
            provider_id,
            model_id,
            api_key_label,
            max_tokens: Some(max_tokens),
            max_tokens_field_name,
            ..
        } if id == profile_id
            && name == "Loaded"
            && provider_id == "openai"
            && model_id == "gpt-4.1"
            && api_key_label == "openai-key"
            && max_tokens == 8192
            && max_tokens_field_name == "max_completion_tokens"
    ));
}

#[test]
fn profile_editor_load_with_whitespace_max_tokens_field_name_treated_as_empty() {
    let id = Uuid::new_v4();
    let mut state = ProfileEditorState::new_profile();

    apply_profile_editor_load(
        &mut state,
        id,
        "Whitespace Field",
        "openai",
        "gpt-4.1",
        "https://api.openai.com/v1",
        "openai-key",
        0.5,
        Some(1024),
        "   ", // whitespace-only max_tokens_field_name
        "{}",
        None,
        false,
        false,
        None,
        "prompt",
    );

    // Whitespace-only max_tokens_field_name should still be stored as-is
    // The normalization to None happens in emit_save_profile
    assert_eq!(state.data.max_tokens_field_name, "   ");
}

#[test]
fn profile_editor_load_with_empty_max_tokens_field_name() {
    let id = Uuid::new_v4();
    let mut state = ProfileEditorState::new_profile();

    apply_profile_editor_load(
        &mut state,
        id,
        "Empty Field",
        "anthropic",
        "claude-sonnet-4-20250514",
        "https://api.anthropic.com/v1",
        "anthropic-key",
        0.7,
        Some(4096),
        "", // empty max_tokens_field_name
        "{}",
        None,
        false,
        false,
        None,
        "prompt",
    );

    assert_eq!(state.data.max_tokens_field_name, "");
}

#[test]
fn profile_editor_load_with_custom_max_tokens_field_name() {
    let id = Uuid::new_v4();
    let mut state = ProfileEditorState::new_profile();

    apply_profile_editor_load(
        &mut state,
        id,
        "Custom Field",
        "openai",
        "o1-mini",
        "https://api.openai.com/v1",
        "openai-key",
        1.0,
        Some(100_000),
        "max_completion_tokens", // custom override
        "{}",
        None,
        false,
        true, // enable_thinking
        Some(10000),
        "Be thorough",
    );

    assert_eq!(state.data.max_tokens_field_name, "max_completion_tokens");
    assert!(state.data.enable_extended_thinking);
    assert_eq!(state.data.thinking_budget, 10000);
}

#[test]
fn emit_save_payload_normalizes_max_tokens_field_name_sentinel_to_none() {
    let mut data = ProfileEditorData::new();
    data.id = Some(Uuid::new_v4().to_string());
    data.name = "Test Profile".to_string();
    data.model_id = "claude-sonnet-4".to_string();
    data.base_url = "https://api.anthropic.com/v1".to_string();
    data.key_label = "anthropic-key".to_string();
    data.max_tokens_field_name = "max_tokens".to_string(); // sentinel value

    let payload = emit_save_payload(&data);
    let params = payload.parameters.expect("parameters should exist");
    // The view normalizes "max_tokens" sentinel to None
    assert_eq!(params.max_tokens_field_name, None);
}

#[test]
fn emit_save_payload_keeps_custom_max_tokens_field_name() {
    let mut data = ProfileEditorData::new();
    data.id = Some(Uuid::new_v4().to_string());
    data.name = "Test Profile".to_string();
    data.model_id = "claude-sonnet-4".to_string();
    data.base_url = "https://api.anthropic.com/v1".to_string();
    data.key_label = "anthropic-key".to_string();
    data.max_tokens_field_name = "max_completion_tokens".to_string(); // custom override

    let payload = emit_save_payload(&data);
    let params = payload.parameters.expect("parameters should exist");
    assert_eq!(
        params.max_tokens_field_name.as_deref(),
        Some("max_completion_tokens")
    );
}

#[test]
fn emit_save_payload_normalizes_empty_max_tokens_field_name_to_none() {
    let mut data = ProfileEditorData::new();
    data.id = Some(Uuid::new_v4().to_string());
    data.name = "Test Profile".to_string();
    data.model_id = "claude-sonnet-4".to_string();
    data.base_url = "https://api.anthropic.com/v1".to_string();
    data.key_label = "anthropic-key".to_string();
    data.max_tokens_field_name = String::new(); // empty

    let payload = emit_save_payload(&data);
    let params = payload.parameters.expect("parameters should exist");
    assert_eq!(params.max_tokens_field_name, None);
}

#[test]
fn emit_save_payload_normalizes_whitespace_max_tokens_field_name_to_none() {
    let mut data = ProfileEditorData::new();
    data.id = Some(Uuid::new_v4().to_string());
    data.name = "Test Profile".to_string();
    data.model_id = "claude-sonnet-4".to_string();
    data.base_url = "https://api.anthropic.com/v1".to_string();
    data.key_label = "anthropic-key".to_string();
    data.max_tokens_field_name = "   ".to_string(); // whitespace

    let payload = emit_save_payload(&data);
    let params = payload.parameters.expect("parameters should exist");
    assert_eq!(params.max_tokens_field_name, None);
}
