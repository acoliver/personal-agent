//! Profile Editor View tests
//!
//! @plan PLAN-20250130-GPUIREDUX.P08

#![allow(clippy::future_not_send)]

use super::*;
use flume;
use gpui::{AppContext, TestAppContext};

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
async fn set_bridge_requests_api_keys_and_model_selection_can_be_saved(cx: &mut TestAppContext) {
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
async fn key_refresh_and_navigation_actions_emit_expected_events(cx: &mut TestAppContext) {
    clear_navigation_requests();
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
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

#[gpui::test]
async fn validate_advanced_request_json_sets_validation_message(cx: &mut TestAppContext) {
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        // Valid JSON object
        view.state.data.extra_request_fields = r#"{"reasoning":{"effort":"medium"}}"#.to_string();
        view.validate_advanced_request_json(cx);
        assert_eq!(
            view.state.advanced_json_validation_message,
            Some("Advanced request JSON is valid.".to_string())
        );

        // Invalid JSON
        view.state.data.extra_request_fields = "not json".to_string();
        view.validate_advanced_request_json(cx);
        assert!(view.state.advanced_json_validation_message.is_some());
        let msg = view
            .state
            .advanced_json_validation_message
            .as_ref()
            .unwrap();
        assert!(msg.starts_with("Advanced request JSON is invalid:"));

        // Non-object JSON (array)
        view.state.data.extra_request_fields = "[1, 2, 3]".to_string();
        view.validate_advanced_request_json(cx);
        assert_eq!(
            view.state.advanced_json_validation_message,
            Some("Advanced request JSON must be a JSON object.".to_string())
        );
    });
}

/// Regression test for issue #182 (Bug 1).
///
/// When a user is editing an existing profile and clicks Browse to pick a
/// different model, the editor must preserve the profile's identity
/// (`id`, `key_label`, `is_new = false`, `name`, `system_prompt`) so that:
///
///   1. The API KEY dropdown remains populated, keeping `can_save()` true.
///   2. Saving updates the original profile instead of silently creating a
///      duplicate (because `data.id` is still the original UUID).
#[gpui::test]
async fn model_selected_preserves_edit_state_for_issue_182(cx: &mut TestAppContext) {
    let profile_id = Uuid::new_v4();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        // Simulate the Edit flow: presenter loads an existing profile.
        view.handle_command(
            ViewCommand::ProfileEditorLoad {
                id: profile_id,
                name: "My Anthropic".to_string(),
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-5-sonnet".to_string(),
                base_url: "https://api.anthropic.com/v1".to_string(),
                api_key_label: "anthropic-key".to_string(),
                temperature: 0.7,
                max_tokens: Some(4096),
                max_tokens_field_name: "max_tokens".to_string(),
                extra_request_fields: "{}".to_string(),
                context_limit: Some(200_000),
                show_thinking: true,
                enable_thinking: false,
                thinking_budget: None,
                system_prompt: "Be helpful.".to_string(),
            },
            cx,
        );

        assert!(!view.state.is_new, "sanity: starting in edit mode");
        assert!(view.state.data.can_save(), "sanity: edit form is savable");

        // User clicks Browse → picks a new model. The editor receives
        // `ModelSelected`.
        view.handle_command(
            ViewCommand::ModelSelected {
                provider_id: "openai".to_string(),
                model_id: "gpt-4.1".to_string(),
                provider_api_url: Some("https://api.openai.com/v1".to_string()),
                context_length: Some(128_000),
            },
            cx,
        );

        // Identity of the profile must be preserved.
        assert!(
            !view.state.is_new,
            "is_new must remain false so Save issues an update, not a create"
        );
        assert_eq!(
            view.state.data.id.as_deref(),
            Some(profile_id.to_string().as_str()),
            "profile id must be preserved through model browse"
        );

        // Model fields must be updated.
        assert_eq!(view.state.data.model_id, "gpt-4.1");
        assert_eq!(view.state.data.api_type, ApiType::OpenAI);

        // Fields that had user-entered values must NOT be overwritten.
        assert_eq!(view.state.data.name, "My Anthropic");
        assert_eq!(view.state.data.base_url, "https://api.anthropic.com/v1");
        assert_eq!(view.state.data.system_prompt, "Be helpful.");
        // A user-customised context_limit (200_000) must not be clobbered by
        // the newly-selected model's native context length (128_000).
        assert_eq!(
            view.state.data.context_limit, 200_000,
            "user-customised context_limit must survive model Browse"
        );

        // The API key selection must survive, keeping Save enabled.
        assert_eq!(view.state.data.key_label, "anthropic-key");
        assert!(
            view.state.data.can_save(),
            "Save must remain enabled after Browse during an edit"
        );
    });
}

/// Regression test for issue #182 (Bug 1) — new-profile flow still adopts the
/// model's native context length.
///
/// When a user is starting a brand-new profile (editor at defaults), selecting
/// a model should populate `context_limit` from the model's `context_length`
/// so the form reflects that model's capabilities out of the box.
#[gpui::test]
async fn model_selected_on_blank_editor_adopts_context_length(cx: &mut TestAppContext) {
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        // Sanity: blank editor starts at the default context_limit.
        assert_eq!(
            view.state.data.context_limit,
            ProfileEditorData::DEFAULT_CONTEXT_LIMIT
        );

        view.handle_command(
            ViewCommand::ModelSelected {
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-5-sonnet".to_string(),
                provider_api_url: Some("https://api.anthropic.com/v1".to_string()),
                context_length: Some(200_000),
            },
            cx,
        );

        assert_eq!(
            view.state.data.context_limit, 200_000,
            "blank editor should adopt the selected model's native context length"
        );
    });
}

/// Regression test for issue #182 (Bug 2).
///
/// The `ProfileEditorReset` view command must clear the editor to a blank
/// new-profile state while preserving the cached `available_keys` list.
#[gpui::test]
async fn profile_editor_reset_clears_state_but_preserves_keys_for_issue_182(
    cx: &mut TestAppContext,
) {
    let (bridge, user_rx) = make_bridge();
    let stale_profile_id = Uuid::new_v4();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        // Drop the `RefreshApiKeys` emitted by `set_bridge` so only the
        // reset-triggered refresh remains.
        let _ = user_rx.recv().expect("refresh api keys from set_bridge");

        // Populate editor with stale data as if we had been editing a profile.
        view.handle_command(
            ViewCommand::ProfileEditorLoad {
                id: stale_profile_id,
                name: "Stale".to_string(),
                provider_id: "anthropic".to_string(),
                model_id: "claude".to_string(),
                base_url: "https://api.anthropic.com/v1".to_string(),
                api_key_label: "anthropic-key".to_string(),
                temperature: 0.7,
                max_tokens: Some(4096),
                max_tokens_field_name: "max_tokens".to_string(),
                extra_request_fields: "{}".to_string(),
                context_limit: Some(200_000),
                show_thinking: true,
                enable_thinking: false,
                thinking_budget: None,
                system_prompt: "Old prompt".to_string(),
            },
            cx,
        );
        view.handle_command(
            ViewCommand::ApiKeysListed {
                keys: vec![
                    ApiKeyInfo {
                        label: "anthropic-key".to_string(),
                        masked_value: "••••1234".to_string(),
                        used_by: vec!["Stale".to_string()],
                    },
                    ApiKeyInfo {
                        label: "openai-key".to_string(),
                        masked_value: "••••5678".to_string(),
                        used_by: vec![],
                    },
                ],
            },
            cx,
        );

        // Fire the reset command — as if the user clicked `+` and the
        // presenter forwarded `UserEvent::OpenNewProfile` back as
        // `ProfileEditorReset`.
        view.handle_command(ViewCommand::ProfileEditorReset, cx);

        assert!(
            view.state.is_new,
            "reset must flip back to new-profile mode"
        );
        assert!(view.state.data.id.is_none(), "profile id must be cleared");
        assert_eq!(view.state.data.name, "", "name must be cleared");
        assert_eq!(view.state.data.model_id, "", "model must be cleared");
        assert_eq!(view.state.data.base_url, "", "base_url must be cleared");
        assert_eq!(
            view.state.data.key_label, "",
            "key selection must be cleared"
        );
        assert_eq!(
            view.state.data.system_prompt,
            crate::models::profile::DEFAULT_SYSTEM_PROMPT,
            "system prompt must fall back to the default"
        );

        // Available keys must survive so the dropdown doesn't flicker empty.
        assert_eq!(
            view.state.data.available_keys,
            vec!["anthropic-key".to_string(), "openai-key".to_string()],
        );
    });

    // Reset requests a refresh so we pick up any newly-stored keys.
    assert_eq!(
        user_rx.recv().expect("reset requests api key refresh"),
        UserEvent::RefreshApiKeys
    );
}

/// Regression test for issue #182: Cancel must drop stale editor state so
/// that the next `+` (new profile) flow starts blank.
#[gpui::test]
async fn reset_to_new_profile_helper_clears_edit_state_for_issue_182(cx: &mut TestAppContext) {
    let profile_id = Uuid::new_v4();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.handle_command(
            ViewCommand::ProfileEditorLoad {
                id: profile_id,
                name: "My Anthropic".to_string(),
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-5-sonnet".to_string(),
                base_url: "https://api.anthropic.com/v1".to_string(),
                api_key_label: "anthropic-key".to_string(),
                temperature: 0.7,
                max_tokens: Some(4096),
                max_tokens_field_name: "max_tokens".to_string(),
                extra_request_fields: "{}".to_string(),
                context_limit: Some(200_000),
                show_thinking: true,
                enable_thinking: false,
                thinking_budget: None,
                system_prompt: "Be helpful.".to_string(),
            },
            cx,
        );
        view.handle_command(
            ViewCommand::ApiKeysListed {
                keys: vec![ApiKeyInfo {
                    label: "anthropic-key".to_string(),
                    masked_value: "••••1234".to_string(),
                    used_by: vec!["My Anthropic".to_string()],
                }],
            },
            cx,
        );

        // Cancel handler behaviour: reset before navigating.
        view.reset_to_new_profile();

        assert!(view.state.is_new);
        assert!(view.state.data.id.is_none());
        assert_eq!(view.state.data.name, "");
        assert_eq!(view.state.data.key_label, "");
        assert_eq!(
            view.state.data.available_keys,
            vec!["anthropic-key".to_string()],
            "available_keys must survive the reset"
        );
    });
}

/// Regression test for issue #182 (Bug 3 — local-provider profiles): loading a
/// profile whose `provider_id` is `"local"` must classify it as
/// `ApiType::Local` (not `ApiType::Custom`), so the editor knows it does not
/// require an API key and Save stays enabled even though the profile has no
/// keychain entry.
///
/// User repro: edit "autoshot model remote" (a local profile), change the
/// model name in the MODEL field, then try to Save → Save was disabled.
#[gpui::test]
async fn local_profile_load_keeps_save_enabled_for_issue_182(cx: &mut TestAppContext) {
    let profile_id = Uuid::new_v4();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.handle_command(
            ViewCommand::ProfileEditorLoad {
                id: profile_id,
                name: "autoshot model remote".to_string(),
                provider_id: "local".to_string(),
                model_id: "dpo_merged_q4_k_m.gguf".to_string(),
                base_url: "http://fed-net.internet-box.ch:8080/".to_string(),
                // Local profiles have no keychain label.
                api_key_label: String::new(),
                temperature: 0.7,
                max_tokens: Some(4096),
                max_tokens_field_name: "max_tokens".to_string(),
                extra_request_fields: "{}".to_string(),
                context_limit: Some(8_192),
                show_thinking: true,
                enable_thinking: false,
                thinking_budget: None,
                system_prompt: "Be helpful.".to_string(),
            },
            cx,
        );

        // Provider-id "local" must map to ApiType::Local, not Custom.
        assert_eq!(view.state.data.api_type, ApiType::Local);
        assert!(
            !view.state.data.api_type.requires_api_key(),
            "local profiles must not require an API key"
        );
        assert!(
            view.state.data.can_save(),
            "Save must be enabled for a local profile loaded from disk"
        );

        // The user changes the model name in the MODEL field — Save must
        // remain enabled.
        view.state.data.model_id = "different_model.gguf".to_string();
        assert!(
            view.state.data.can_save(),
            "Save must remain enabled after editing the model field on a local profile"
        );
    });
}

/// Regression test: `ApiType::from_provider_id` must round-trip the four
/// canonical provider ids correctly so that load / model-selection paths can
/// never drift out of sync again (root cause of the local-profile bug).
#[test]
fn api_type_from_provider_id_covers_all_known_providers() {
    assert_eq!(ApiType::from_provider_id("anthropic"), ApiType::Anthropic);
    assert_eq!(ApiType::from_provider_id("openai"), ApiType::OpenAI);
    assert_eq!(ApiType::from_provider_id("local"), ApiType::Local);
    assert_eq!(
        ApiType::from_provider_id("ollama"),
        ApiType::Custom("ollama".to_string()),
    );
}

/// Regression test for issue #182: the editor's CONTEXT LIMIT field must be
/// included in the save payload so the presenter can persist it. Before the
/// fix, `emit_save_profile` built a `ModelProfileParameters` without
/// `context_window_size`, so any edit to the field was silently dropped.
#[gpui::test]
async fn emit_save_profile_carries_context_window_size_for_issue_182(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.handle_command(
            ViewCommand::ModelSelected {
                provider_id: "openai".to_string(),
                model_id: "gpt-4.1".to_string(),
                provider_api_url: None,
                context_length: Some(128_000),
            },
            cx,
        );
        view.state.data.key_label = "openai-key".to_string();
        // Simulate the user typing into the CONTEXT LIMIT field after
        // selecting the model.
        view.state.data.context_limit = 64_000;
        view.emit_save_profile();
    });

    // Drain the RefreshApiKeys event the bridge always emits first.
    let _ = user_rx.recv().expect("refresh api keys event");

    let event = user_rx.recv().expect("save profile event");
    let UserEvent::SaveProfile { profile } = event else {
        panic!("expected SaveProfile, got {event:?}");
    };
    let parameters = profile
        .parameters
        .as_ref()
        .expect("save payload must include parameters");
    assert_eq!(
        parameters.context_window_size,
        Some(64_000),
        "CONTEXT LIMIT must round-trip through the save payload"
    );
}
