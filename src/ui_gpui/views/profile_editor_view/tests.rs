//! Profile Editor View tests
//!
//! @plan PLAN-20250130-GPUIREDUX.P08

#![allow(clippy::future_not_send)]

use super::*;
use flume;
use gpui::{AppContext, EntityInputHandler, TestAppContext};

use crate::config::default_api_base_url_for_provider;
use crate::events::types::UserEvent;
use crate::presentation::view_command::{ApiKeyInfo, ViewCommand};

pub(super) fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

pub(super) fn clear_navigation_requests() {
    while crate::ui_gpui::navigation_channel()
        .take_pending()
        .is_some()
    {}
}

#[gpui::test]
async fn set_bridge_requests_api_keys_and_model_selection_can_be_saved(
    cx: &mut TestAppContext,
) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.handle_command(
            ViewCommand::ModelSelected {
                provider_id: "openai".to_string(),
                model_id: "gpt-4.1".to_string(),
                provider_api_url: None,
                context_length: Some(256_000),
            },
            cx,
        );
        view.state.data.key_label = "openai-key".to_string();
        view.emit_save_profile();
    });

    assert_eq!(
        user_rx.recv().expect("refresh api keys event"),
        UserEvent::RefreshApiKeys
    );
    match user_rx.recv().expect("save profile event") {
        UserEvent::SaveProfile { profile } => {
            assert_eq!(profile.name, "gpt-4.1");
            assert_eq!(profile.provider_id.as_deref(), Some("openai"));
            assert_eq!(profile.model_id.as_deref(), Some("gpt-4.1"));
            assert_eq!(
                profile.base_url.as_deref(),
                Some(default_api_base_url_for_provider("openai").as_str())
            );
            assert!(matches!(
                profile.auth,
                Some(ModelProfileAuth::Keychain { ref label }) if label == "openai-key"
            ));
            let parameters = profile.parameters.expect("parameters included");
            assert_eq!(parameters.max_tokens, Some(4096));
            assert_eq!(parameters.enable_thinking, Some(false));
            assert_eq!(parameters.thinking_budget, None);
        }
        other => panic!("expected SaveProfile event, got {other:?}"),
    }
}

#[gpui::test]
async fn profile_editor_load_and_api_key_listing_replace_visible_editor_state(
    cx: &mut TestAppContext,
) {
    let profile_id = Uuid::new_v4();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.state.active_field = Some(ActiveField::Name);
        view.handle_command(
            ViewCommand::ProfileEditorLoad {
                id: profile_id,
                name: "Existing Profile".to_string(),
                provider_id: "anthropic".to_string(),
                model_id: "claude-sonnet-4-20250514".to_string(),
                base_url: "https://api.anthropic.com/v1".to_string(),
                api_key_label: "anthropic-key".to_string(),
                temperature: 0.25,
                max_tokens: Some(8192),
                max_tokens_field_name: "max_tokens".to_string(),
                extra_request_fields: "{}".to_string(),

                context_limit: Some(200_000),
                show_thinking: false,
                enable_thinking: true,
                thinking_budget: None,
                system_prompt: "Use tools when helpful".to_string(),
            },
            cx,
        );
        view.handle_command(
            ViewCommand::ApiKeysListed {
                keys: vec![
                    ApiKeyInfo {
                        label: "anthropic-key".to_string(),
                        masked_value: "••••1234".to_string(),
                        used_by: vec!["Existing Profile".to_string()],
                    },
                    ApiKeyInfo {
                        label: "backup-key".to_string(),
                        masked_value: "••••5678".to_string(),
                        used_by: vec![],
                    },
                ],
            },
            cx,
        );

        assert!(!view.state.is_new);
        assert_eq!(
            view.state.data.id.as_deref(),
            Some(profile_id.to_string().as_str())
        );
        assert_eq!(view.state.data.name, "Existing Profile");
        assert_eq!(view.state.data.model_id, "claude-sonnet-4-20250514");
        assert_eq!(view.state.data.api_type, ApiType::Anthropic);
        assert_eq!(view.state.data.base_url, "https://api.anthropic.com/v1");
        assert_eq!(view.state.data.key_label, "anthropic-key");
        assert!((view.state.data.temperature - 0.25_f32).abs() < f32::EPSILON);
        assert_eq!(view.state.data.max_tokens, "8192");
        assert_eq!(view.state.data.max_tokens_field_name, "max_tokens");
        assert!(!view.state.advanced_request_parameters_expanded);
        assert_eq!(view.state.data.context_limit, 200_000);
        assert!(!view.state.data.show_thinking);
        assert!(view.state.data.enable_extended_thinking);
        assert_eq!(view.state.data.thinking_budget, 10_000);
        assert_eq!(view.state.data.system_prompt, "Use tools when helpful");
        assert_eq!(view.state.active_field, None);
    });
}

#[gpui::test]
async fn input_handling_updates_fields_and_backspace_removes_chars(cx: &mut TestAppContext) {
    clear_navigation_requests();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, window, cx| {
        view.state.active_field = Some(ActiveField::Name);
        view.replace_text_in_range(None, "Preset Δ", window, cx);
        assert_eq!(view.state.data.name, "Preset Δ");
        assert_eq!(view.marked_text_range(window, cx), None);
        view.unmark_text(window, cx);

        view.state.active_field = Some(ActiveField::MaxTokens);
        view.state.data.max_tokens = "0".to_string();
        view.replace_text_in_range(None, "16", window, cx);
        assert_eq!(view.state.data.max_tokens, "16");
        view.replace_text_in_range(None, "k", window, cx);
        assert_eq!(view.state.data.max_tokens, "16");
        view.backspace_active_field();
        assert_eq!(view.state.data.max_tokens, "1");

        view.state.advanced_request_parameters_expanded = true;
        view.state.active_field = Some(ActiveField::MaxTokensFieldName);
        view.state.data.max_tokens_field_name = "max_tokens".to_string();
        view.replace_text_in_range(None, "_override", window, cx);
        assert_eq!(view.state.data.max_tokens_field_name, "max_tokens_override");
        assert_eq!(
            view.text_for_range(0..10, &mut None, window, cx),
            Some("max_tokens".to_string())
        );
        view.backspace_active_field();
        assert_eq!(view.state.data.max_tokens_field_name, "max_tokens_overrid");

        view.state.active_field = Some(ActiveField::SystemPrompt);
        let prompt_before = view.state.data.system_prompt.clone();
        view.replace_and_mark_text_in_range(None, " plan", None, window, cx);
        assert_eq!(view.state.data.system_prompt, prompt_before + " plan");
        view.unmark_text(window, cx);

        view.state.active_field = Some(ActiveField::BaseUrl);
        view.state.data.base_url = "https://api.example".to_string();
        view.replace_text_in_range(None, "https://api.example", window, cx);
        view.replace_and_mark_text_in_range(None, "/v", None, window, cx);
        assert_eq!(view.state.data.base_url, "https://api.example/v");
        view.replace_text_in_range(None, "/v1", window, cx);
        assert_eq!(view.state.data.base_url, "https://api.example/v1");
        view.backspace_active_field();
        assert_eq!(view.state.data.base_url, "https://api.example/v");

        view.state.active_field = Some(ActiveField::ContextLimit);
        view.state.data.context_limit = 0;
        view.replace_text_in_range(None, "32", window, cx);
        assert_eq!(view.state.data.context_limit, 32);
        view.replace_text_in_range(None, "k", window, cx);
        assert_eq!(view.state.data.context_limit, 32);
        view.backspace_active_field();
        assert_eq!(view.state.data.context_limit, 3);

        view.state.active_field = Some(ActiveField::ThinkingBudget);
        view.state.data.thinking_budget = 0;
        view.replace_text_in_range(None, "4096", window, cx);
        assert_eq!(view.state.data.thinking_budget, 4096);
        view.backspace_active_field();
        assert_eq!(view.state.data.thinking_budget, 409);

        view.state.active_field = None;
        view.cycle_active_field();
        assert_eq!(view.state.active_field, Some(ActiveField::Name));
        for _ in 0..8 {
            view.cycle_active_field();
        }

        assert_eq!(view.state.active_field, Some(ActiveField::SystemPrompt));
        view.cycle_active_field();
        assert_eq!(view.state.active_field, Some(ActiveField::Name));
    });
}

#[gpui::test]
async fn profile_editor_load_toggles_advanced_request_parameter_visibility(
    cx: &mut TestAppContext,
) {
    let profile_id = Uuid::new_v4();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.handle_command(
            ViewCommand::ProfileEditorLoad {
                id: profile_id,
                name: "Existing Profile".to_string(),
                provider_id: "openai".to_string(),
                model_id: "gpt-4.1".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key_label: "openai-key".to_string(),
                temperature: 0.3,
                max_tokens: Some(4096),
                max_tokens_field_name: "max_completion_tokens".to_string(),
                extra_request_fields: "{}".to_string(),

                context_limit: Some(128_000),
                show_thinking: true,
                enable_thinking: false,
                thinking_budget: Some(2048),
                system_prompt: "Use tools when helpful".to_string(),
            },
            cx,
        );
        assert!(view.state.advanced_request_parameters_expanded);

        view.handle_command(
            ViewCommand::ProfileEditorLoad {
                id: profile_id,
                name: "Existing Profile".to_string(),
                provider_id: "anthropic".to_string(),
                model_id: "claude-sonnet-4-20250514".to_string(),
                base_url: "https://api.anthropic.com/v1".to_string(),
                api_key_label: "anthropic-key".to_string(),
                temperature: 0.25,
                max_tokens: Some(8192),
                max_tokens_field_name: "max_tokens".to_string(),
                extra_request_fields: "{}".to_string(),

                context_limit: Some(200_000),
                show_thinking: false,
                enable_thinking: true,
                thinking_budget: None,
                system_prompt: "Use tools when helpful".to_string(),
            },
            cx,
        );
        assert!(!view.state.advanced_request_parameters_expanded);
    });
}

#[gpui::test]
async fn key_refresh_and_navigation_actions_emit_expected_events(cx: &mut TestAppContext) {
    clear_navigation_requests();
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        // Trigger a refresh
        view.request_api_key_refresh();
    });

    assert_eq!(
        user_rx.recv().expect("refresh api keys event"),
        UserEvent::RefreshApiKeys
    );
}

#[gpui::test]
async fn local_api_type_requires_no_key_and_can_be_saved(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.data.name = "Local Profile".to_string();
        view.state.data.model_id = "qwen-3.5-4b".to_string();
        view.state.data.base_url = "http://localhost:8080/v1".to_string();
        view.state.data.api_type = ApiType::Local;

        // Local provider should not require API key
        assert!(!view.state.data.api_type.requires_api_key());
        assert!(view.state.data.key_label.is_empty());

        // Can save without key_label for Local provider
        assert!(view.state.data.can_save());

        view.emit_save_profile();
    });

    assert_eq!(
        user_rx.recv().expect("refresh api keys event"),
        UserEvent::RefreshApiKeys
    );
    match user_rx.recv().expect("save profile event") {
        UserEvent::SaveProfile { profile } => {
            assert_eq!(profile.name, "Local Profile");
            assert_eq!(profile.provider_id.as_deref(), Some("local"));
            assert_eq!(profile.model_id.as_deref(), Some("qwen-3.5-4b"));
            // Should emit None auth for Local provider
            assert!(matches!(profile.auth, Some(ModelProfileAuth::None)));
        }
        other => panic!("expected SaveProfile event, got {other:?}"),
    }
}

#[gpui::test]
async fn api_type_cycles_through_anthropic_openai_local_anthropic(cx: &mut TestAppContext) {
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.state.data.api_type = ApiType::Anthropic;
        assert_eq!(view.state.data.api_type.display(), "Anthropic");
        assert!(view.state.data.api_type.requires_api_key());

        // Cycle to OpenAI
        view.state.data.api_type = match view.state.data.api_type {
            ApiType::Anthropic => ApiType::OpenAI,
            ApiType::OpenAI => ApiType::Local,
            ApiType::Local | ApiType::Custom(_) => ApiType::Anthropic,
        };
        assert_eq!(view.state.data.api_type.display(), "OpenAI");
        assert!(view.state.data.api_type.requires_api_key());

        // Cycle to Local
        view.state.data.api_type = match view.state.data.api_type {
            ApiType::Anthropic => ApiType::OpenAI,
            ApiType::OpenAI => ApiType::Local,
            ApiType::Local | ApiType::Custom(_) => ApiType::Anthropic,
        };
        assert_eq!(view.state.data.api_type.display(), "Local Model");
        assert!(!view.state.data.api_type.requires_api_key());

        // Cycle back to Anthropic
        view.state.data.api_type = match view.state.data.api_type {
            ApiType::Anthropic => ApiType::OpenAI,
            ApiType::OpenAI => ApiType::Local,
            ApiType::Local | ApiType::Custom(_) => ApiType::Anthropic,
        };
        assert_eq!(view.state.data.api_type.display(), "Anthropic");
    });
}
