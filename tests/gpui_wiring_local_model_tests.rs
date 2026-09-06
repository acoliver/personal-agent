//! Wiring tests for the Local Model settings panel (Phase 04).
//!
//! Logic-level tests: category enumeration, store-management classification,
//! command handling, and the save/unload event flow driven through a
//! `GpuiBridge` — no live engine and no rendering required.
//!
// @plan:PLAN-20260903-LOCALMODEL.P04
// @requirement:REQ-LM-006

#![allow(clippy::future_not_send, clippy::unused_async)]

use gpui::{AppContext, TestAppContext};
use personal_agent::events::types::UserEvent;
use personal_agent::llm::local::engine::EngineStatus;
use personal_agent::presentation::view_command::ViewCommand;
use personal_agent::services::local_model_settings::LocalModelSettings;
use personal_agent::ui_gpui::app_store::is_store_managed;
use personal_agent::ui_gpui::bridge::GpuiBridge;
use personal_agent::ui_gpui::views::settings_view::{SettingsCategory, SettingsView};
use std::sync::Arc;

fn sample_settings() -> LocalModelSettings {
    LocalModelSettings {
        model_path: "/tmp/local-model-test/fake.gguf".into(),
        n_ctx: 4096,
        gpu_layers: 41,
        idle_unload: true,
        idle_timeout_minutes: 7,
    }
}

fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

#[test]
fn category_all_contains_local_model_between_models_and_skills() {
    let all = SettingsCategory::ALL;
    assert_eq!(all.len(), 8, "Local Model should bring the count to 8");
    let pos = all
        .iter()
        .position(|c| *c == SettingsCategory::LocalModel)
        .expect("LocalModel must be enumerated");
    assert_eq!(all[pos - 1], SettingsCategory::Models);
    assert_eq!(all[pos + 1], SettingsCategory::Skills);
}

#[test]
fn local_model_category_display_name() {
    assert_eq!(SettingsCategory::LocalModel.display_name(), "Local Model");
}

/// The status enum travels inside `ViewCommand`s; a payload written before
/// `total_layers` existed (serde default 0) must still parse so the card can
/// degrade to its plain "N layers" rendering.
// @plan:PLAN-20260903-LOCALMODEL.P05
// @requirement:REQ-LM-006
#[test]
fn status_payload_without_total_layers_still_parses() {
    let parsed: EngineStatus =
        serde_json::from_str(r#"{"Loaded":{"layers":41,"n_ctx":8192,"last_tok_s":68.4}}"#)
            .expect("older payload without total_layers must deserialize");
    assert_eq!(
        parsed,
        EngineStatus::Loaded {
            layers: 41,
            total_layers: 0,
            n_ctx: 8192,
            last_tok_s: 68.4
        }
    );
}

#[test]
fn local_model_commands_are_not_store_managed() {
    // Settings-panel commands follow the Backup precedent: they are delivered
    // to the view directly, not through the app-store snapshot reducer.
    assert!(!is_store_managed(&ViewCommand::LocalModelSettingsLoaded {
        settings: sample_settings(),
    }));
    assert!(!is_store_managed(&ViewCommand::LocalModelStatusUpdated {
        status: EngineStatus::NotLoaded,
    }));
}

#[gpui::test]
async fn settings_loaded_populates_panel_edit_buffers(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);
    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::LocalModelSettingsLoaded {
                settings: sample_settings(),
            },
            cx,
        );
        let state = view.get_state();
        assert_eq!(
            state.local_model_path_input,
            "/tmp/local-model-test/fake.gguf"
        );
        assert_eq!(state.local_model_ctx_input, "4096");
        assert_eq!(state.local_model_gpu_layers_input, "41");
        assert_eq!(state.local_model_idle_minutes_input, "7");
        assert!(state.local_model_idle_unload);
        assert_eq!(
            state.local_model_settings.as_ref().map(|s| s.n_ctx),
            Some(4096)
        );
    });
}

#[gpui::test]
async fn status_updates_transition_the_status_card(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);
    view.update(cx, |view, _cx| {
        assert_eq!(
            view.get_state().local_model_status,
            EngineStatus::NotLoaded,
            "panel starts NotLoaded"
        );

        let changed = view.apply_local_model_status(EngineStatus::Loaded {
            layers: 41,
            total_layers: 41,
            n_ctx: 8192,
            last_tok_s: 68.4,
        });
        assert!(changed, "Loaded should change the card");
        assert_eq!(
            view.get_state().local_model_status,
            EngineStatus::Loaded {
                layers: 41,
                total_layers: 41,
                n_ctx: 8192,
                last_tok_s: 68.4
            }
        );
        assert!(view.get_state().local_model_error.is_none());

        // Re-applying an identical snapshot must not report a change.
        let changed = view.apply_local_model_status(EngineStatus::Loaded {
            layers: 41,
            total_layers: 41,
            n_ctx: 8192,
            last_tok_s: 68.4,
        });
        assert!(!changed, "identical status is a no-op");
    });
}

#[gpui::test]
async fn error_status_surfaces_the_engine_message(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);
    view.update(cx, |view, _cx| {
        view.apply_local_model_status(EngineStatus::Error {
            message: "model file not found: /tmp/missing.gguf".to_string(),
        });
        assert_eq!(
            view.get_state().local_model_error.as_deref(),
            Some("model file not found: /tmp/missing.gguf"),
            "the card must show the engine's error message"
        );

        // Recovering back to NotLoaded clears the error line.
        view.apply_local_model_status(EngineStatus::NotLoaded);
        assert!(view.get_state().local_model_error.is_none());
    });
}

#[gpui::test]
async fn save_emits_settings_built_from_edit_buffers(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(SettingsView::new);
    view.update(cx, |view, _cx| {
        view.set_bridge(bridge);
        view.apply_local_model_settings(sample_settings());
        // Edit two buffers, then save.
        view.state.local_model_ctx_input = "2048".to_string();
        view.state.local_model_idle_unload = false;
        view.save_local_model_edits();
    });

    let event = user_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("Save must emit SaveLocalModelSettings");
    match event {
        UserEvent::SaveLocalModelSettings { settings } => {
            assert_eq!(settings.n_ctx, 2048);
            assert_eq!(settings.gpu_layers, 41);
            assert!(!settings.idle_unload);
            assert_eq!(settings.idle_timeout_minutes, 7);
            assert_eq!(
                settings.model_path.to_string_lossy(),
                "/tmp/local-model-test/fake.gguf"
            );
        }
        other => panic!("expected SaveLocalModelSettings, got {other:?}"),
    }
}

#[gpui::test]
async fn save_with_invalid_numeric_input_emits_nothing(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(SettingsView::new);
    view.update(cx, |view, _cx| {
        view.set_bridge(bridge);
        view.apply_local_model_settings(sample_settings());
        view.state.local_model_ctx_input = "not-a-number".to_string();
        view.save_local_model_edits();

        let state = view.get_state();
        assert!(state.status_is_error, "invalid input is an inline error");
        let message = state.status_message.clone().unwrap_or_default();
        assert!(
            message.contains("Context size"),
            "error should name the field, got: {message}"
        );
    });
    assert!(
        user_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err(),
        "invalid input must not emit a save event"
    );
}

#[gpui::test]
async fn save_with_empty_path_is_rejected_inline(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(SettingsView::new);
    view.update(cx, |view, _cx| {
        view.set_bridge(bridge);
        view.apply_local_model_settings(sample_settings());
        view.state.local_model_path_input = "   ".to_string();
        view.save_local_model_edits();
        assert!(view.get_state().status_is_error);
    });
    assert!(
        user_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err(),
        "empty path must not emit a save event"
    );
}

#[gpui::test]
async fn entering_local_model_category_requests_load(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(SettingsView::new);
    view.update(cx, |view, _cx| {
        view.set_bridge(bridge);
        view.select_category(SettingsCategory::LocalModel);
    });
    let event = user_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("entering the panel must emit LoadLocalModelSettings");
    assert_eq!(event, UserEvent::LoadLocalModelSettings);
}

#[gpui::test]
async fn unload_emits_unload_event(_cx: &mut TestAppContext) {
    // The unload emitter lives behind the panel's button; drive the event
    // through the bridge directly to pin the wire format.
    let (bridge, user_rx) = make_bridge();
    bridge.emit(UserEvent::UnloadLocalModel);
    let event = user_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("UnloadLocalModel must traverse the bridge");
    assert_eq!(event, UserEvent::UnloadLocalModel);
}

// ── one-click local profile (PLAN-20260903-LOCALMODEL.P05, REQ-LM-002) ────

fn profile_summary(id: uuid::Uuid, name: &str, provider: &str) -> ViewCommand {
    ViewCommand::ShowSettings {
        profiles: vec![personal_agent::presentation::view_command::ProfileSummary {
            id,
            name: name.to_string(),
            provider_id: provider.to_string(),
            model_id: "some-model".to_string(),
            is_default: true,
        }],
        selected_profile_id: Some(id),
    }
}

/// @plan:PLAN-20260903-LOCALMODEL.P05
/// @requirement:REQ-LM-002
#[gpui::test]
async fn create_button_tracks_whether_a_local_profile_exists(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);
    view.update(cx, |view, cx| {
        assert!(
            !view.has_local_profile(),
            "a fresh view has no local profile"
        );

        // An existing install with a remote-only profile list still shows
        // the button.
        view.handle_command(
            profile_summary(uuid::Uuid::new_v4(), "Remote", "anthropic"),
            cx,
        );
        assert!(!view.has_local_profile());

        // Once any profile routes to the local engine, the button's reason
        // to exist is gone.
        view.handle_command(
            profile_summary(uuid::Uuid::new_v4(), "Granite (local)", "local"),
            cx,
        );
        assert!(view.has_local_profile());
    });
}

/// @plan:PLAN-20260903-LOCALMODEL.P05
/// @requirement:REQ-LM-002
#[gpui::test]
async fn create_button_emits_create_local_profile(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(SettingsView::new);
    view.update(cx, |view, _cx| {
        view.set_bridge(bridge);
        assert!(!view.has_local_profile(), "precondition for the button");
        view.emit_create_local_profile();
    });
    let event = user_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("the create button must emit CreateLocalProfile");
    assert_eq!(event, UserEvent::CreateLocalProfile);
}
