#![allow(clippy::future_not_send)]

use super::*;
use flume;
use gpui::{AppContext, TestAppContext};
use uuid::Uuid;

use crate::events::types::UserEvent;
use crate::presentation::view_command::{ErrorSeverity, ViewCommand};

mod oauth_and_save_gate;

fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

fn clear_navigation_requests() {
    while crate::ui_gpui::navigation_channel()
        .take_pending()
        .is_some()
    {}
}

#[gpui::test]
async fn draft_loaded_sets_auth_transport_and_save_payload_for_remote_http(
    cx: &mut TestAppContext,
) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.handle_command(
            ViewCommand::McpConfigureDraftLoaded {
                id: Uuid::nil().to_string(),
                name: "Exa Remote".to_string(),
                package: "exa-remote".to_string(),
                package_type: crate::mcp::McpPackageType::Http,
                runtime_hint: None,
                env_var_name: String::new(),
                command: String::new(),
                args: vec![],
                auth_type: crate::mcp::McpAuthType::None,
                oauth_connected: false,
                keyfile_path: String::new(),
                env: vec![],
                stored_secret_names: vec![],
                url: Some("https://exa.example/mcp".to_string()),
            },
            cx,
        );
        assert!(view.state.is_new);
        assert_eq!(view.state.data.auth_method, McpAuthMethod::None);
        assert_eq!(
            view.state.data.url.as_deref(),
            Some("https://exa.example/mcp")
        );
        assert!(view.state.data.can_save());
        view.emit_save_mcp_config();
    });

    match user_rx.recv().expect("save mcp config event") {
        UserEvent::SaveMcpConfig {
            id,
            config,
            secrets,
        } => {
            assert_eq!(id, Uuid::nil());
            assert_eq!(config.name, "Exa Remote");
            assert_eq!(
                config.package.package_type,
                crate::mcp::McpPackageType::Http
            );
            assert_eq!(config.transport, crate::mcp::McpTransport::Http);
            assert_eq!(
                config.source,
                crate::mcp::McpSource::Manual {
                    url: "https://exa.example/mcp".to_string()
                }
            );
            assert!(config.env_vars.is_empty());
            assert!(secrets.is_empty());
        }
        other => panic!("expected SaveMcpConfig event, got {other:?}"),
    }
}

#[gpui::test]
async fn draft_loaded_with_env_requires_api_key_and_status_commands_update_oauth_state(
    cx: &mut TestAppContext,
) {
    let view = cx.new(McpConfigureView::new);
    let saved_id = Uuid::new_v4();

    view.update(cx, |view: &mut McpConfigureView, cx| {
        view.handle_command(
            ViewCommand::McpConfigureDraftLoaded {
                id: saved_id.to_string(),
                name: "Filesystem".to_string(),
                package: "@modelcontextprotocol/server-filesystem".to_string(),
                package_type: crate::mcp::McpPackageType::Npm,
                runtime_hint: Some("npx".to_string()),
                env_var_name: "FILESYSTEM_TOKEN".to_string(),
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-filesystem".to_string(),
                ],
                auth_type: crate::mcp::McpAuthType::ApiKey,
                oauth_connected: false,
                keyfile_path: String::new(),
                env: vec![("FILESYSTEM_TOKEN".to_string(), String::new(), true)],
                stored_secret_names: vec![],
                url: None,
            },
            cx,
        );
        assert!(!view.state.is_new);
        assert_eq!(view.state.data.auth_method, McpAuthMethod::ApiKey);
        assert_eq!(view.state.data.env_var_name, "FILESYSTEM_TOKEN");
        assert_eq!(view.state.data.runtime_hint.as_deref(), Some("npx"));
        assert!(!view.state.data.can_save());

        view.handle_command(
            ViewCommand::ShowNotification {
                message: "alice".to_string(),
            },
            cx,
        );
        assert_eq!(
            view.state.data.oauth_status,
            OAuthStatus::Connected {
                username: "alice".to_string()
            }
        );

        view.handle_command(
            ViewCommand::ShowError {
                title: "oauth failed".to_string(),
                message: "denied".to_string(),
                severity: ErrorSeverity::Error,
            },
            cx,
        );
        assert_eq!(
            view.state.data.oauth_status,
            OAuthStatus::Error("denied".to_string())
        );

        view.handle_command(
            ViewCommand::McpConfigSaved {
                id: saved_id,
                name: Some("Filesystem Saved".to_string()),
            },
            cx,
        );
        assert_eq!(
            view.state.data.id.as_deref(),
            Some(saved_id.to_string().as_str())
        );
        assert_eq!(view.state.data.name, "Filesystem Saved");
        assert!(!view.state.is_new);
        // A save must not fake an OAuth connection.
        assert_eq!(
            view.state.data.oauth_status,
            OAuthStatus::Error("denied".to_string())
        );
    });
}

#[gpui::test]
async fn set_mcp_with_keyfile_auth_and_docker_package_emits_stdio_save_payload(
    cx: &mut TestAppContext,
) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);
    let saved_id = Uuid::new_v4();

    view.update(cx, |view: &mut McpConfigureView, _cx| {
        view.set_bridge(Arc::clone(&bridge));

        let mut data = McpConfigureData::new();
        data.id = Some(saved_id.to_string());
        data.name = "Docker Filesystem".to_string();
        data.package = "ghcr.io/example/filesystem-mcp:latest".to_string();
        data.package_type = crate::mcp::McpPackageType::Docker;
        data.command = "docker".to_string();
        data.args = vec!["run".to_string(), "--rm".to_string()];
        data.env = vec![
            ("FILESYSTEM_TOKEN".to_string(), String::new(), true),
            ("ROOT".to_string(), String::new(), true),
        ];
        data.auth_method = McpAuthMethod::Keyfile;
        data.keyfile_path = "/tmp/filesystem-key.json".to_string();

        view.set_mcp(data, false);
        assert!(!view.state.is_new);
        assert!(view.state.data.can_save());

        view.state.data.keyfile_path.clear();
        assert!(!view.state.data.can_save());
        view.state.data.keyfile_path = "/tmp/filesystem-key.json".to_string();
        assert!(view.state.data.can_save());

        view.emit_save_mcp_config();
    });

    match user_rx.recv().expect("save docker mcp event") {
        UserEvent::SaveMcpConfig {
            id,
            config,
            secrets,
        } => {
            assert_eq!(id, saved_id);
            assert_eq!(config.name, "Docker Filesystem");
            assert_eq!(
                config.package.package_type,
                crate::mcp::McpPackageType::Docker
            );
            assert_eq!(
                config.package.identifier,
                "ghcr.io/example/filesystem-mcp:latest"
            );
            assert_eq!(config.package.runtime_hint.as_deref(), Some("docker"));
            assert_eq!(config.transport, crate::mcp::McpTransport::Stdio);
            assert_eq!(
                config.source,
                crate::mcp::McpSource::Manual {
                    url: "docker run ghcr.io/example/filesystem-mcp:latest".to_string()
                }
            );
            assert_eq!(
                config.env_vars,
                vec![
                    crate::mcp::EnvVarConfig {
                        name: "FILESYSTEM_TOKEN".to_string(),
                        required: true,
                        is_secret: false,
                        value: Some(String::new()),
                    },
                    crate::mcp::EnvVarConfig {
                        name: "ROOT".to_string(),
                        required: true,
                        is_secret: false,
                        value: Some(String::new()),
                    },
                ]
            );
            assert_eq!(config.auth_type, crate::mcp::McpAuthType::Keyfile);
            assert_eq!(
                config.keyfile_path,
                Some(std::path::PathBuf::from("/tmp/filesystem-key.json"))
            );
            assert!(secrets.is_empty());
        }
        other => panic!("expected SaveMcpConfig event, got {other:?}"),
    }
}

#[gpui::test]
async fn helper_actions_and_key_shortcuts_emit_oauth_save_and_navigation_events(
    cx: &mut TestAppContext,
) {
    clear_navigation_requests();
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);
    let saved_id = Uuid::new_v4();

    view.update(cx, |view: &mut McpConfigureView, view_cx| {
        view.set_bridge(Arc::clone(&bridge));

        let mut data = McpConfigureData::new();
        data.id = Some(saved_id.to_string());
        data.name = "Weather MCP".to_string();
        data.package = "@example/weather-mcp".to_string();
        data.package_type = crate::mcp::McpPackageType::Npm;
        data.runtime_hint = Some("npx".to_string());
        data.command = "npx".to_string();
        data.auth_method = McpAuthMethod::ApiKey;
        data.env_var_name = "WEATHER_API_KEY".to_string();
        data.api_key = "secret-token".to_string();
        data.oauth_provider = "ExampleAuth".to_string();
        view.set_mcp(data, false);

        assert!(view.state.mask_api_key);
        view.toggle_mask_api_key(view_cx);
        assert!(!view.state.mask_api_key);
        view.toggle_mask_api_key(view_cx);
        assert!(view.state.mask_api_key);

        view.start_oauth();
        view.save_current(view_cx);
    });

    let mut visual_cx = cx.add_empty_window().clone();
    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            view.handle_key_down(&key_event("cmd-s"), window, cx);
        });
    });
    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            view.handle_key_down(&key_event("escape"), window, cx);
        });
    });
    assert_eq!(
        crate::ui_gpui::navigation_channel().take_pending(),
        Some(crate::presentation::view_command::ViewId::Settings)
    );

    McpConfigureView::navigate_to_settings();
    assert_eq!(
        crate::ui_gpui::navigation_channel().take_pending(),
        Some(crate::presentation::view_command::ViewId::Settings)
    );

    assert_eq!(
        user_rx.recv().expect("oauth start event"),
        UserEvent::StartMcpOAuth {
            id: saved_id,
            provider: "ExampleAuth".to_string(),
        }
    );

    match user_rx.recv().expect("explicit save event") {
        UserEvent::SaveMcpConfig {
            id,
            config,
            secrets,
        } => {
            assert_eq!(id, saved_id);
            assert_eq!(config.name, "Weather MCP");
            assert_eq!(config.package.identifier, "@example/weather-mcp");
            assert_eq!(config.auth_type, crate::mcp::McpAuthType::ApiKey);
            assert_eq!(
                secrets,
                vec![(
                    "WEATHER_API_KEY".to_string(),
                    crate::events::types::SecretValue::new("secret-token")
                )]
            );
        }
        other => panic!("expected SaveMcpConfig event, got {other:?}"),
    }

    match user_rx.recv().expect("cmd-s save event") {
        UserEvent::SaveMcpConfig { id, config, .. } => {
            assert_eq!(id, saved_id);
            assert_eq!(config.name, "Weather MCP");
            assert_eq!(config.package.identifier, "@example/weather-mcp");
        }
        other => panic!("expected SaveMcpConfig event, got {other:?}"),
    }

    assert!(
        user_rx.try_recv().is_err(),
        "unexpected additional mcp configure events"
    );
}

fn key_event(key: &str) -> gpui::KeyDownEvent {
    gpui::KeyDownEvent {
        keystroke: gpui::Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
        is_held: false,
        prefer_character_input: false,
    }
}

#[gpui::test]
async fn paste_populates_active_field_and_sanitizes(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            view.active_field = Some(ActiveField::ApiKey);
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                "sk-test-123\r\n".to_string(),
            ));
            view.handle_key_down(&key_event("cmd-v"), window, cx);
            assert_eq!(view.state.data.api_key, "sk-test-123");

            view.active_field = Some(ActiveField::KeyfilePath);
            view.handle_key_down(&key_event("cmd-v"), window, cx);
            assert_eq!(view.state.data.keyfile_path, "sk-test-123");
        });
    });
}

#[gpui::test]
async fn paste_without_active_field_is_a_no_op(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                "sk-nothing\r\n".to_string(),
            ));
            view.handle_key_down(&key_event("cmd-v"), window, cx);
            assert!(view.state.data.api_key.is_empty());
            assert!(view.state.data.keyfile_path.is_empty());
        });
    });
}

#[gpui::test]
async fn backspace_pops_last_character(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            view.state.data.api_key = "sk-test".to_string();
            view.active_field = Some(ActiveField::ApiKey);
            view.handle_key_down(&key_event("backspace"), window, cx);
            assert_eq!(view.state.data.api_key, "sk-tes");

            view.state.data.keyfile_path = "/tmp/k.json".to_string();
            view.active_field = Some(ActiveField::KeyfilePath);
            view.handle_key_down(&key_event("backspace"), window, cx);
            assert_eq!(view.state.data.keyfile_path, "/tmp/k.jso");
        });
    });
}

#[gpui::test]
async fn tab_targets_only_fields_rendered_for_auth_method(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            // Default method is ApiKey: the single rendered field.
            view.handle_key_down(&key_event("tab"), window, cx);
            assert_eq!(view.active_field, Some(ActiveField::ApiKey));

            view.handle_key_down(&key_event("tab"), window, cx);
            assert_eq!(view.active_field, Some(ActiveField::ApiKey));

            // Keyfile renders only the path field.
            view.state.data.auth_method = McpAuthMethod::Keyfile;
            view.handle_key_down(&key_event("tab"), window, cx);
            assert_eq!(view.active_field, Some(ActiveField::KeyfilePath));
            assert_eq!(view.ime_marked_byte_count, 0);

            // None/OAuth render no input field.
            view.state.data.auth_method = McpAuthMethod::None;
            view.handle_key_down(&key_event("tab"), window, cx);
            assert_eq!(view.active_field, None);
        });
    });
}

#[gpui::test]
async fn ime_replace_lands_in_active_field(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler;

    let view = cx.new(McpConfigureView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            view.active_field = Some(ActiveField::ApiKey);

            view.replace_text_in_range(None, "sk-ime", window, cx);
            assert_eq!(view.state.data.api_key, "sk-ime");

            view.replace_and_mark_text_in_range(None, "!", None, window, cx);
            assert_eq!(view.state.data.api_key, "sk-ime!");
            assert_eq!(view.marked_text_range(window, cx), Some(6..7));

            view.replace_text_in_range(None, "?", window, cx);
            assert_eq!(view.state.data.api_key, "sk-ime?");
            assert_eq!(view.marked_text_range(window, cx), None);
        });
    });
}

#[gpui::test]
async fn auth_dropdown_state_transitions(cx: &mut TestAppContext) {
    clear_navigation_requests();
    let view = cx.new(McpConfigureView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            view.active_field = Some(ActiveField::ApiKey);

            view.toggle_auth_dropdown(cx);
            assert!(view.show_auth_dropdown);
            assert_eq!(view.active_field, None);

            view.handle_key_down(&key_event("escape"), window, cx);
            assert!(!view.show_auth_dropdown);
            assert_eq!(crate::ui_gpui::navigation_channel().take_pending(), None);

            view.toggle_auth_dropdown(cx);
            view.select_auth_method(McpAuthMethod::OAuth, cx);
            assert_eq!(view.state.data.auth_method, McpAuthMethod::OAuth);
            assert!(!view.show_auth_dropdown);

            // cmd-w still navigates once the dropdown is closed
            view.handle_key_down(&key_event("cmd-w"), window, cx);
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(crate::presentation::view_command::ViewId::Settings)
            );
        });
    });
}

#[gpui::test]
async fn save_payload_carries_auth_env_and_secrets(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);
    let saved_id = Uuid::new_v4();

    view.update(cx, |view: &mut McpConfigureView, _cx| {
        view.set_bridge(Arc::clone(&bridge));

        let mut data = McpConfigureData::new();
        data.id = Some(saved_id.to_string());
        data.name = "Exa".to_string();
        data.url = Some("https://exa.example/mcp".to_string());
        data.auth_method = McpAuthMethod::ApiKey;
        data.env_var_name = "EXA_API_KEY".to_string();
        data.env = vec![("EXA_API_KEY".to_string(), String::new(), true)];
        data.api_key = "secret".to_string();
        view.set_mcp(data, false);
        view.emit_save_mcp_config();
    });

    match user_rx.recv().expect("save mcp config event") {
        UserEvent::SaveMcpConfig {
            id,
            config,
            secrets,
        } => {
            assert_eq!(id, saved_id);
            assert_eq!(config.auth_type, crate::mcp::McpAuthType::ApiKey);
            assert_eq!(
                config.env_vars,
                vec![crate::mcp::EnvVarConfig {
                    name: "EXA_API_KEY".to_string(),
                    required: true,
                    is_secret: true,
                    value: None,
                }]
            );
            assert_eq!(
                secrets,
                vec![(
                    "EXA_API_KEY".to_string(),
                    crate::events::types::SecretValue::new("secret")
                )]
            );
            assert_eq!(config.keyfile_path, None);
        }
        other => panic!("expected SaveMcpConfig event, got {other:?}"),
    }
}

#[gpui::test]
async fn save_payload_derives_env_var_when_missing(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, _cx| {
        view.set_bridge(Arc::clone(&bridge));

        let mut data = McpConfigureData::new();
        data.name = "Manual MCP".to_string();
        data.package = "@example/manual".to_string();
        data.command = "npx".to_string();
        data.auth_method = McpAuthMethod::ApiKey;
        data.env_var_name = "API_KEY".to_string();
        data.env = vec![];
        data.api_key = "typed-key".to_string();
        view.set_mcp(data, true);
        view.emit_save_mcp_config();
    });

    match user_rx.recv().expect("save mcp config event") {
        UserEvent::SaveMcpConfig {
            config, secrets, ..
        } => {
            assert_eq!(config.auth_type, crate::mcp::McpAuthType::ApiKey);
            assert_eq!(
                config.env_vars,
                vec![crate::mcp::EnvVarConfig {
                    name: "API_KEY".to_string(),
                    required: true,
                    is_secret: true,
                    value: None,
                }]
            );
            assert_eq!(
                secrets,
                vec![(
                    "API_KEY".to_string(),
                    crate::events::types::SecretValue::new("typed-key")
                )]
            );
        }
        other => panic!("expected SaveMcpConfig event, got {other:?}"),
    }
}

#[gpui::test]
async fn save_payload_carries_keyfile_path(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, _cx| {
        view.set_bridge(Arc::clone(&bridge));

        let mut data = McpConfigureData::new();
        data.name = "Keyfile MCP".to_string();
        data.package = "@example/keyfile".to_string();
        data.command = "npx".to_string();
        data.auth_method = McpAuthMethod::Keyfile;
        data.keyfile_path = "/tmp/service-key.json".to_string();
        data.env = vec![("FILESYSTEM_TOKEN".to_string(), String::new(), true)];
        view.set_mcp(data, true);
        view.emit_save_mcp_config();
    });

    match user_rx.recv().expect("save mcp config event") {
        UserEvent::SaveMcpConfig {
            config, secrets, ..
        } => {
            assert_eq!(config.auth_type, crate::mcp::McpAuthType::Keyfile);
            assert_eq!(
                config.keyfile_path,
                Some(std::path::PathBuf::from("/tmp/service-key.json"))
            );
            assert_eq!(
                config.env_vars,
                vec![crate::mcp::EnvVarConfig {
                    name: "FILESYSTEM_TOKEN".to_string(),
                    required: true,
                    is_secret: false,
                    value: Some(String::new()),
                }]
            );
            assert!(secrets.is_empty());
        }
        other => panic!("expected SaveMcpConfig event, got {other:?}"),
    }
}

#[gpui::test]
async fn secret_flagged_env_rows_demote_to_plain_under_non_api_key_auth(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, _cx| {
        let mut data = McpConfigureData::new();
        data.name = "Path MCP".to_string();
        data.command = "npx".to_string();
        data.auth_method = McpAuthMethod::None;
        // Registry name-heuristic drift can flag a plain path var as
        // secret; under auth None the typed value must survive as a plain
        // row rather than being dropped with the secret slot.
        data.env = vec![("BUNDLE_PATH".to_string(), "/opt/bundle".to_string(), true)];
        view.set_mcp(data, true);

        let env_vars = view.state.data.persisted_env_vars();
        assert_eq!(
            env_vars,
            vec![crate::mcp::EnvVarConfig {
                name: "BUNDLE_PATH".to_string(),
                required: true,
                is_secret: false,
                value: Some("/opt/bundle".to_string()),
            }]
        );
    });
}

#[gpui::test]
async fn paste_during_ime_composition_drops_marked_bytes(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            view.active_field = Some(ActiveField::ApiKey);
            // Committed text plus a marked composition tail ("XY").
            view.state.data.api_key = "abcXY".to_string();
            view.ime_marked_byte_count = 2;

            view.paste_text("pasted", cx);

            // The marked tail is replaced, exactly like replace_text_in_range.
            assert_eq!(view.state.data.api_key, "abcpasted");
            assert_eq!(view.ime_marked_byte_count, 0);
        });
    });
}

#[gpui::test]
async fn multi_line_paste_strips_interior_newlines(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            view.active_field = Some(ActiveField::ApiKey);
            view.paste_text("ab\r\ncd\nef", cx);
            assert_eq!(view.state.data.api_key, "abcdef");
        });
    });
}
