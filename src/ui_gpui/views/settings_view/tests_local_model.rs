//! Local Model panel: window-rendered behavioral tests.
//!
//! Every test runs headless through GPUI's test window: the panel is drawn
//! for real, interactions are simulated mouse clicks at element bounds, and
//! assertions read rendered element bounds or the user-event channel. No
//! engine, no GGUF file, no display.
//!
// @plan:PLAN-20260903-LOCALMODEL.P05
// @requirement:REQ-LM-006

#![allow(clippy::future_not_send)]

use super::*;
use gpui::{
    px, size, Bounds, Entity, EntityInputHandler, Modifiers, Pixels, TestAppContext,
    VisualTestContext,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::events::types::UserEvent;
use crate::presentation::view_command::{ProfileSummary, ViewCommand};
use crate::services::local_model_settings::LocalModelSettings;

fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

fn sample_settings() -> LocalModelSettings {
    LocalModelSettings {
        model_path: "/tmp/local-model-tests/fake.gguf".into(),
        n_ctx: 4096,
        gpu_layers: 41,
        idle_unload: true,
        idle_timeout_minutes: 7,
    }
}

fn profile_summary(id: Uuid, name: &str, provider: &str) -> ViewCommand {
    ViewCommand::ShowSettings {
        profiles: vec![ProfileSummary {
            id,
            name: name.to_string(),
            provider_id: provider.to_string(),
            model_id: "some-model".to_string(),
            is_default: true,
        }],
        selected_profile_id: Some(id),
    }
}

/// Mount the settings view with a bridge on the Local Model panel, sized
/// tall enough that nothing is clipped. No frame is drawn: each test calls
/// [`redraw`] after arranging state, which also keeps gpui's per-frame
/// debug-bounds map free of entries from frames the test never asserts on.
fn mount_local_panel(
    cx: &mut TestAppContext,
) -> (
    Entity<SettingsView>,
    &mut VisualTestContext,
    flume::Receiver<UserEvent>,
) {
    let (bridge, user_rx) = make_bridge();
    let (view, window_cx) = cx.add_window_view({
        let bridge = Arc::clone(&bridge);
        move |_window, cx| {
            let mut view = SettingsView::new(cx);
            view.bridge = Some(bridge);
            view
        }
    });
    window_cx.update(|_window, app| {
        view.update(app, |view, _cx| {
            view.state.selected_category = SettingsCategory::LocalModel;
        });
    });
    window_cx.simulate_resize(size(px(900.0), px(2000.0)));
    (view, window_cx, user_rx)
}

/// Flush pending effects and paint a fresh frame from current view state.
fn redraw(window_cx: &mut VisualTestContext) {
    window_cx.run_until_parked();
    window_cx.update(|window, app| {
        let _ = window.draw(app);
    });
}

fn bounds_of(window_cx: &mut VisualTestContext, id: &'static str) -> Bounds<Pixels> {
    window_cx
        .debug_bounds(id)
        .unwrap_or_else(|| panic!("element `{id}` should be rendered"))
}

fn assert_rendered(window_cx: &mut VisualTestContext, ids: &[&'static str], present: bool) {
    for id in ids {
        let rendered = window_cx.debug_bounds(id).is_some();
        assert_eq!(
            rendered, present,
            "element `{id}`: expected present = {present}"
        );
    }
}

fn click(window_cx: &mut VisualTestContext, id: &'static str) {
    let center = bounds_of(window_cx, id).center();
    window_cx.simulate_click(center, Modifiers::none());
}

const ALL_PANEL_CONTROLS: &[&str] = &[
    "local-model-path-input",
    "btn-choose-local-model",
    "local-model-ctx-input",
    "local-model-gpu-input",
    "local-model-idle-toggle",
    "local-model-idle-input",
    "btn-save-local-model",
    "btn-unload-local-model",
];

#[gpui::test]
async fn panel_with_a_local_profile_shows_all_controls_but_not_one_click_create(
    cx: &mut TestAppContext,
) {
    let (bridge, _user_rx) = make_bridge();
    // Seed the profile in the window's build closure: gpui draws a first
    // frame while the window opens, so any profile arranged later would
    // still leave the create row in that initial (stale-map) frame.
    let (_view, window_cx) = cx.add_window_view({
        let bridge = Arc::clone(&bridge);
        move |_window, cx| {
            let mut view = SettingsView::new(cx);
            view.bridge = Some(bridge);
            view.state.selected_category = SettingsCategory::LocalModel;
            view.handle_command(
                profile_summary(Uuid::new_v4(), "Granite (local)", "local"),
                cx,
            );
            view
        }
    });
    window_cx.simulate_resize(size(px(900.0), px(2000.0)));
    redraw(window_cx);

    assert_rendered(window_cx, ALL_PANEL_CONTROLS, true);
    assert!(
        window_cx.debug_bounds("btn-create-local-profile").is_none(),
        "the one-click create row must disappear once a local profile exists"
    );
}

#[gpui::test]
async fn one_click_create_row_stops_responding_once_a_local_profile_arrives(
    cx: &mut TestAppContext,
) {
    let (view, window_cx, user_rx) = mount_local_panel(cx);
    redraw(window_cx);
    assert_rendered(window_cx, &["btn-create-local-profile"], true);

    click(window_cx, "btn-create-local-profile");
    assert_eq!(
        user_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("the create row click must reach the presenter"),
        UserEvent::CreateLocalProfile
    );

    window_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.handle_command(
                profile_summary(Uuid::new_v4(), "Granite (local)", "local"),
                cx,
            );
            assert!(
                view.has_local_profile(),
                "the profile must be in state after ShowSettings"
            );
        });
    });
    redraw(window_cx);

    // gpui (pinned rev c67328a) never clears `debug_bounds` between frames,
    // so a row painted in an earlier frame still reports bounds. What users
    // actually lose is the hitbox: clicking the row's last known position
    // must not create anything now that a profile exists. If a future gpui
    // starts clearing the map, the entry reads None and the row being absent
    // from the frame already satisfies this branch.
    if let Some(previous) = window_cx.debug_bounds("btn-create-local-profile") {
        window_cx.simulate_click(previous.center(), Modifiers::none());
    }
    assert!(
        user_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err(),
        "after a local profile exists the create row must not respond to clicks"
    );
}

#[gpui::test]
async fn unload_now_button_emits_unload_event(cx: &mut TestAppContext) {
    let (_view, window_cx, user_rx) = mount_local_panel(cx);
    redraw(window_cx);

    click(window_cx, "btn-unload-local-model");

    assert_eq!(
        user_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("Unload now must reach the presenter"),
        UserEvent::UnloadLocalModel
    );
    assert!(
        user_rx.try_recv().is_err(),
        "the unload button must emit nothing else"
    );
}

#[gpui::test]
async fn idle_toggle_flips_local_state_and_emits_nothing(cx: &mut TestAppContext) {
    let (view, window_cx, user_rx) = mount_local_panel(cx);

    window_cx.update(|_window, app| {
        view.update(app, |view, _cx| {
            assert!(
                view.state.local_model_idle_unload,
                "idle-unload defaults to on"
            );
        });
    });
    redraw(window_cx);

    click(window_cx, "local-model-idle-toggle");
    redraw(window_cx);
    window_cx.update(|_window, app| {
        view.update(app, |view, _cx| {
            assert!(!view.state.local_model_idle_unload);
        });
    });
    assert!(
        user_rx.try_recv().is_err(),
        "the toggle edits locally; persistence happens only on Save"
    );

    click(window_cx, "local-model-idle-toggle");
    redraw(window_cx);
    window_cx.update(|_window, app| {
        view.update(app, |view, _cx| {
            assert!(view.state.local_model_idle_unload);
        });
    });
    assert!(user_rx.try_recv().is_err());
}

#[gpui::test]
async fn clicking_context_input_focuses_it_and_typed_digits_reach_the_save_event(
    cx: &mut TestAppContext,
) {
    let (view, window_cx, user_rx) = mount_local_panel(cx);
    window_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.handle_command(
                ViewCommand::LocalModelSettingsLoaded {
                    settings: sample_settings(),
                },
                cx,
            );
            view.state.local_model_ctx_input.clear();
        });
    });
    redraw(window_cx);

    click(window_cx, "local-model-ctx-input");
    window_cx.update(|_window, app| {
        view.update(app, |view, _cx| {
            assert_eq!(
                view.state.active_field,
                Some(ActiveField::LocalModelCtxInput),
                "clicking the Context size input must focus it"
            );
        });
    });
    redraw(window_cx);

    window_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.replace_text_in_range(None, "2048", window, cx);
            assert_eq!(view.state.local_model_ctx_input, "2048");
        });
    });

    click(window_cx, "btn-save-local-model");

    match user_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("Save must emit SaveLocalModelSettings")
    {
        UserEvent::SaveLocalModelSettings { settings } => {
            assert_eq!(settings.n_ctx, 2048, "the typed context size wins");
            assert_eq!(
                settings.gpu_layers, 41,
                "fields the user did not edit travel from the loaded snapshot"
            );
            assert_eq!(settings.idle_timeout_minutes, 7);
            assert!(settings.idle_unload);
            assert_eq!(
                settings.model_path.to_string_lossy(),
                "/tmp/local-model-tests/fake.gguf"
            );
        }
        other => panic!("expected SaveLocalModelSettings, got {other:?}"),
    }
    window_cx.update(|_window, app| {
        view.update(app, |view, _cx| {
            assert!(!view.state.status_is_error);
            assert_eq!(
                view.state.status_message.as_deref(),
                Some("Saving local model settings...")
            );
        });
    });
}

#[gpui::test]
async fn save_with_non_numeric_gpu_layers_is_rejected_inline(cx: &mut TestAppContext) {
    let (view, window_cx, user_rx) = mount_local_panel(cx);
    window_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.handle_command(
                ViewCommand::LocalModelSettingsLoaded {
                    settings: sample_settings(),
                },
                cx,
            );
            view.state.local_model_gpu_layers_input = "abc".to_string();
        });
    });
    redraw(window_cx);

    click(window_cx, "btn-save-local-model");

    window_cx.update(|_window, app| {
        view.update(app, |view, _cx| {
            assert!(view.state.status_is_error, "bad input is an inline error");
            let message = view.state.status_message.clone().unwrap_or_default();
            assert!(
                message.contains("GPU layers"),
                "the error must name the offending field, got: {message}"
            );
        });
    });
    assert!(
        user_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err(),
        "invalid input must not reach the presenter"
    );
}

#[gpui::test]
async fn save_floors_a_zero_idle_timeout_at_one_minute(cx: &mut TestAppContext) {
    let (view, window_cx, user_rx) = mount_local_panel(cx);
    window_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.handle_command(
                ViewCommand::LocalModelSettingsLoaded {
                    settings: sample_settings(),
                },
                cx,
            );
            view.state.local_model_idle_minutes_input = "0".to_string();
        });
    });
    redraw(window_cx);

    click(window_cx, "btn-save-local-model");

    match user_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("a zero idle timeout parses and saves")
    {
        UserEvent::SaveLocalModelSettings { settings } => {
            assert_eq!(
                settings.idle_timeout_minutes, 1,
                "the engine cannot take a zero idle timeout"
            );
        }
        other => panic!("expected SaveLocalModelSettings, got {other:?}"),
    }
}
