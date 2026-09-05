//! Local-model tests for the profile editor.
//!
//! Split from `tests.rs` to stay under this repo's 1000-line file cap.

use super::super::*;
use super::make_bridge;
use gpui::{AppContext, TestAppContext};

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

/// @plan:PLAN-20260903-LOCALMODEL.P03
/// @requirement:REQ-LM-007
#[gpui::test]
async fn local_profiles_save_with_no_endpoint_and_survive_provider_sync(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.data.name = "Granite (local)".to_string();
        view.state.data.model_id = "granite-4.2-3b".to_string();
        view.state.data.api_type = ApiType::Local;
        // A stale endpoint carried over from a previous API type is wiped,
        // not defaulted to OpenAI.
        view.state.data.base_url = "https://api.openai.com/v1".to_string();

        view.state.data.apply_api_type_change();

        assert_eq!(
            view.state.data.base_url, "",
            "local profiles must persist no base_url"
        );
        assert!(
            view.state.data.can_save(),
            "an empty endpoint is the correct state for local"
        );

        view.emit_save_profile();
    });

    // Drain the key-refresh event the save flow emits first.
    let mut saw_save = false;
    for _ in 0..2 {
        if let Ok(UserEvent::SaveProfile { profile }) = user_rx.try_recv() {
            saw_save = true;
            assert_eq!(profile.provider_id.as_deref(), Some("local"));
            assert_eq!(
                profile.base_url.as_deref(),
                Some(""),
                "persisted JSON must carry no endpoint"
            );
            assert!(matches!(profile.auth, Some(ModelProfileAuth::None)));
        }
    }
    assert!(saw_save, "SaveProfile event was not emitted");
}

/// @plan:PLAN-20260903-LOCALMODEL.P03
/// @requirement:REQ-LM-007
#[gpui::test]
async fn legacy_local_profile_with_baked_openai_url_normalizes_to_empty(cx: &mut TestAppContext) {
    let (bridge, _user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.handle_command(
            ViewCommand::ProfileEditorLoad {
                id: Uuid::new_v4(),
                name: "Old Local".to_string(),
                provider_id: "local".to_string(),
                model_id: "granite-4.2-3b".to_string(),
                // Profiles written before REQ-LM-007 baked this in.
                base_url: "https://api.openai.com/v1".to_string(),
                api_key_label: String::new(),
                oauth_account: String::new(),
                temperature: 0.7,
                max_tokens: None,
                max_tokens_field_name: String::new(),
                extra_request_fields: String::new(),
                context_limit: None,
                show_thinking: false,
                enable_thinking: false,
                thinking_budget: None,
                reasoning_effort: String::new(),
                system_prompt: String::new(),
            },
            cx,
        );

        assert_eq!(view.state.data.api_type, ApiType::Local);
        view.state.data.apply_api_type_change();
        assert_eq!(
            view.state.data.base_url, "",
            "legacy local profile must migrate to an empty endpoint"
        );
        assert!(view.state.data.can_save());
    });
}

fn local_load_command() -> ViewCommand {
    ViewCommand::ProfileEditorLoad {
        id: Uuid::new_v4(),
        name: "Granite (local)".to_string(),
        provider_id: "local".to_string(),
        model_id: "granite-4.2-3b".to_string(),
        base_url: String::new(),
        api_key_label: String::new(),
        oauth_account: String::new(),
        temperature: 0.1,
        max_tokens: Some(1024),
        max_tokens_field_name: String::new(),
        extra_request_fields: "{}".to_string(),
        context_limit: Some(128_000),
        show_thinking: false,
        enable_thinking: false,
        thinking_budget: None,
        reasoning_effort: String::new(),
        system_prompt: String::new(),
    }
}

// ── LOCAL ENGINE (shared) section (PLAN-20260903-LOCALMODEL.P05) ──────────

/// @plan:PLAN-20260903-LOCALMODEL.P05
/// @requirement:REQ-LM-006 REQ-LM-007
#[gpui::test]
async fn local_variant_captures_engine_status_and_requests_shared_settings(
    cx: &mut TestAppContext,
) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.handle_command(local_load_command(), cx);

        // The engine status row has data, and the editor asked for the
        // shared settings snapshot behind the GGUF path row.
        assert!(
            view.state.local_engine_status.is_some(),
            "the Local variant must carry an engine status snapshot"
        );
    });
    let mut saw_load = false;
    while let Ok(event) = user_rx.try_recv() {
        if event == UserEvent::LoadLocalModelSettings {
            saw_load = true;
        }
    }
    assert!(
        saw_load,
        "loading a local profile must request the shared local-model settings"
    );

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        // Leaving Local removes the row's data source.
        view.state.data.api_type = ApiType::Anthropic;
        view.refresh_local_engine_state();
        assert!(view.state.local_engine_status.is_none());
    });
}

/// @plan:PLAN-20260903-LOCALMODEL.P05
/// @requirement:REQ-LM-006
#[gpui::test]
async fn choosing_a_path_writes_the_shared_store_without_clobbering_other_knobs(
    cx: &mut TestAppContext,
) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.apply_local_model_settings(
            crate::services::local_model_settings::LocalModelSettings {
                model_path: "/models/old.gguf".into(),
                n_ctx: 4096,
                gpu_layers: 41,
                idle_unload: true,
                idle_timeout_minutes: 5,
            },
        );
        view.adopt_chosen_local_model_path("/models/new.gguf".to_string());
    });

    let mut saw_save = None;
    while let Ok(event) = user_rx.try_recv() {
        if let UserEvent::SaveLocalModelSettings { settings } = event {
            saw_save = Some(settings);
        }
    }
    let settings = saw_save.expect("choosing a file must save the shared settings");
    assert_eq!(settings.model_path.to_string_lossy(), "/models/new.gguf");
    // The untouched engine knobs travel from the loaded snapshot, not from
    // defaults — a pick must never reset what Settings → Local Model holds.
    assert_eq!(settings.n_ctx, 4096);
    assert_eq!(settings.gpu_layers, 41);
    assert!(settings.idle_unload);
    assert_eq!(settings.idle_timeout_minutes, 5);

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        // Saving the profile itself must not emit a second settings save:
        // an untouched path stays exactly as the user left it.
        view.state.data.name = "x".to_string();
        view.emit_save_profile();
    });
    while let Ok(event) = user_rx.try_recv() {
        assert!(
            !matches!(event, UserEvent::SaveLocalModelSettings { .. }),
            "profile save must not touch the shared engine settings, got {event:?}"
        );
    }
}

/// @plan:PLAN-20260903-LOCALMODEL.P05
/// @requirement:REQ-LM-006
#[gpui::test]
async fn path_picked_before_the_snapshot_arrives_is_saved_on_arrival(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.set_bridge(Arc::clone(&bridge));
        // Picked while the settings load was still in flight.
        view.adopt_chosen_local_model_path("/models/picked-early.gguf".to_string());
    });
    let mut saw_load = false;
    while let Ok(event) = user_rx.try_recv() {
        if event == UserEvent::LoadLocalModelSettings {
            saw_load = true;
        }
    }
    assert!(saw_load, "the editor must request the missing snapshot");

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.apply_local_model_settings(
            crate::services::local_model_settings::LocalModelSettings {
                model_path: "/models/persisted.gguf".into(),
                n_ctx: 8192,
                gpu_layers: 999,
                idle_unload: true,
                idle_timeout_minutes: 5,
            },
        );
        assert_eq!(
            view.state.local_model_path_input, "/models/picked-early.gguf",
            "the user's pick must win over the just-loaded snapshot"
        );
        assert!(!view.state.local_model_path_dirty, "the pick was saved");
    });
    let mut saw_save = None;
    while let Ok(event) = user_rx.try_recv() {
        if let UserEvent::SaveLocalModelSettings { settings } = event {
            saw_save = Some(settings);
        }
    }
    let settings = saw_save.expect("the arriving snapshot must complete the pending save");
    assert_eq!(
        settings.model_path.to_string_lossy(),
        "/models/picked-early.gguf"
    );
}

/// @plan:PLAN-20260903-LOCALMODEL.P05
/// @requirement:REQ-LM-007
#[gpui::test]
async fn switching_to_local_seeds_the_model_label_and_keeps_the_name_editable(
    cx: &mut TestAppContext,
) {
    let mut data = ProfileEditorData::new();
    data.model_id.clear();
    data.api_type = ApiType::Local;
    data.apply_api_type_change();

    assert_eq!(
        data.model_id,
        crate::services::profile_impl::SEED_MODEL_ID,
        "a fresh Local profile must not start with an empty model label"
    );

    // The NAME input path is type-independent: typing into the name field
    // works while the Local variant is selected (render_content renders the
    // name section unconditionally for every API type).
    let (bridge, _user_rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);
    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.data = data;
        view.state.active_field = Some(ActiveField::Name);
        view.append_to_active_field("My");
        view.append_to_active_field(" Granite");
        assert_eq!(view.state.data.name, "My Granite");
    });
}
