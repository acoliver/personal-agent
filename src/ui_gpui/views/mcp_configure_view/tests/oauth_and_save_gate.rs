//! OAuth draft and save-gate tests for `McpConfigureView`.

use super::*;

#[gpui::test]
async fn set_mcp_with_oauth_only_saves_when_connected_and_emits_npm_payload(
    cx: &mut TestAppContext,
) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, cx| {
        view.set_bridge(Arc::clone(&bridge));

        let mut data = McpConfigureData::new();
        data.name = "OAuth MCP".to_string();
        data.package = "@example/oauth-mcp".to_string();
        data.package_type = crate::mcp::McpPackageType::Npm;
        data.runtime_hint = Some("npx".to_string());
        data.command = "npx".to_string();
        data.auth_method = McpAuthMethod::OAuth;
        data.oauth_status = OAuthStatus::NotConnected;

        view.set_mcp(data, true);
        assert!(view.state.is_new);
        assert!(!view.state.data.can_save());

        view.handle_command(
            ViewCommand::ShowNotification {
                message: "carol".to_string(),
            },
            cx,
        );
        assert_eq!(
            view.state.data.oauth_status,
            OAuthStatus::Connected {
                username: "carol".to_string()
            }
        );
        assert!(view.state.data.can_save());

        view.handle_command(
            ViewCommand::ShowError {
                title: "oauth failed".to_string(),
                message: "expired".to_string(),
                severity: ErrorSeverity::Error,
            },
            cx,
        );
        assert_eq!(
            view.state.data.oauth_status,
            OAuthStatus::Error("expired".to_string())
        );
        assert!(!view.state.data.can_save());

        view.handle_command(
            ViewCommand::ShowNotification {
                message: "carol".to_string(),
            },
            cx,
        );
        assert!(view.state.data.can_save());
        view.emit_save_mcp_config();
    });

    match user_rx.recv().expect("save npm mcp event") {
        UserEvent::SaveMcpConfig {
            id,
            config,
            secrets,
        } => {
            assert_ne!(id, Uuid::nil());
            assert_eq!(config.name, "OAuth MCP");
            assert_eq!(config.package.package_type, crate::mcp::McpPackageType::Npm);
            assert_eq!(config.package.identifier, "@example/oauth-mcp");
            assert_eq!(config.package.runtime_hint.as_deref(), Some("npx"));
            assert_eq!(config.transport, crate::mcp::McpTransport::Stdio);
            assert_eq!(
                config.source,
                crate::mcp::McpSource::Manual {
                    url: "npx @example/oauth-mcp".to_string()
                }
            );
            // OAuth drafts carry no env vars and derive no secret var.
            assert!(config.env_vars.is_empty());
            assert!(secrets.is_empty());
        }
        other => panic!("expected SaveMcpConfig event, got {other:?}"),
    }
}

#[gpui::test]
async fn selecting_auth_method_updates_can_save(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, cx| {
        let mut data = McpConfigureData::new();
        data.name = "Exa".to_string();
        data.url = Some("https://exa.example/mcp".to_string());
        data.auth_method = McpAuthMethod::None;
        view.set_mcp(data, true);
        assert!(view.state.data.can_save());

        view.select_auth_method(McpAuthMethod::ApiKey, cx);
        assert!(!view.state.data.can_save());
        view.state.data.api_key = "secret".to_string();
        assert!(view.state.data.can_save());

        view.select_auth_method(McpAuthMethod::Keyfile, cx);
        assert!(!view.state.data.can_save());
        view.state.data.keyfile_path = "/tmp/k.json".to_string();
        assert!(view.state.data.can_save());
    });
}

#[gpui::test]
async fn stored_secret_round_trip_keeps_can_save_and_emits_no_secret(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.handle_command(
            ViewCommand::McpConfigureDraftLoaded {
                id: Uuid::new_v4().to_string(),
                name: "Exa Stored".to_string(),
                package: "@example/exa".to_string(),
                package_type: crate::mcp::McpPackageType::Npm,
                runtime_hint: Some("npx".to_string()),
                env_var_name: "EXA_API_KEY".to_string(),
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@example/exa".to_string()],
                auth_type: crate::mcp::McpAuthType::ApiKey,
                oauth_connected: false,
                keyfile_path: String::new(),
                env: vec![("EXA_API_KEY".to_string(), String::new(), true)],
                stored_secret_names: vec!["EXA_API_KEY".to_string()],
                url: None,
            },
            cx,
        );
        // The stored keychain entry satisfies the save gate on its own.
        assert!(view.state.data.can_save());
        assert!(view.state.data.api_key.is_empty());
        view.emit_save_mcp_config();
    });

    match user_rx.recv().expect("save mcp config event") {
        UserEvent::SaveMcpConfig {
            config, secrets, ..
        } => {
            assert!(
                secrets.is_empty(),
                "an untouched stored key must not be re-emitted"
            );
            assert_eq!(config.auth_type, crate::mcp::McpAuthType::ApiKey);
            assert!(
                config
                    .env_vars
                    .iter()
                    .any(|var| var.is_secret && var.name == "EXA_API_KEY"),
                "the secret env var slot must survive the round trip"
            );
        }
        other => panic!("expected SaveMcpConfig event, got {other:?}"),
    }
}

#[gpui::test]
async fn keyfile_draft_round_trip_preserves_path_and_method(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.handle_command(
            ViewCommand::McpConfigureDraftLoaded {
                id: Uuid::new_v4().to_string(),
                name: "Service MCP".to_string(),
                package: "@example/service".to_string(),
                package_type: crate::mcp::McpPackageType::Npm,
                runtime_hint: Some("npx".to_string()),
                env_var_name: String::new(),
                command: "npx".to_string(),
                args: vec![],
                auth_type: crate::mcp::McpAuthType::Keyfile,
                oauth_connected: false,
                keyfile_path: "/tmp/service-key.json".to_string(),
                env: vec![],
                stored_secret_names: vec![],
                url: None,
            },
            cx,
        );
        // The persisted method and path are restored verbatim, no inference.
        assert_eq!(view.state.data.auth_method, McpAuthMethod::Keyfile);
        assert_eq!(view.state.data.keyfile_path, "/tmp/service-key.json");
        assert!(view.state.data.can_save());
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
            assert!(secrets.is_empty());
        }
        other => panic!("expected SaveMcpConfig event, got {other:?}"),
    }
}

#[gpui::test]
async fn oauth_draft_resets_stale_connection_state(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            // Simulate a previous MCP leaving a Connected status behind.
            view.state.data.oauth_status = OAuthStatus::Connected {
                username: "stale".to_string(),
            };
            view.state.data.oauth_provider = "stale-provider".to_string();
            view.handle_command(
                ViewCommand::McpConfigureDraftLoaded {
                    id: Uuid::new_v4().to_string(),
                    name: "OAuth MCP".to_string(),
                    package: "@example/oauth-mcp".to_string(),
                    package_type: crate::mcp::McpPackageType::Npm,
                    runtime_hint: Some("npx".to_string()),
                    env_var_name: String::new(),
                    command: "npx".to_string(),
                    args: vec![],
                    auth_type: crate::mcp::McpAuthType::OAuth,
                    oauth_connected: false,
                    keyfile_path: String::new(),
                    env: vec![],
                    stored_secret_names: vec![],
                    url: None,
                },
                cx,
            );
            // A fresh draft cannot ride a stale Connected into can_save.
            assert_eq!(view.state.data.oauth_status, OAuthStatus::NotConnected);
            assert!(view.state.data.oauth_provider.is_empty());
            assert!(!view.state.data.can_save());
        });
    });
}

#[gpui::test]
async fn cmd_s_with_incomplete_api_key_emits_nothing(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpConfigureView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.handle_command(
                ViewCommand::McpConfigureDraftLoaded {
                    id: Uuid::new_v4().to_string(),
                    name: "Exa Empty".to_string(),
                    package: "@example/exa".to_string(),
                    package_type: crate::mcp::McpPackageType::Npm,
                    runtime_hint: Some("npx".to_string()),
                    env_var_name: "EXA_API_KEY".to_string(),
                    command: "npx".to_string(),
                    args: vec![],
                    auth_type: crate::mcp::McpAuthType::ApiKey,
                    oauth_connected: false,
                    keyfile_path: String::new(),
                    env: vec![("EXA_API_KEY".to_string(), String::new(), true)],
                    stored_secret_names: vec![],
                    url: None,
                },
                cx,
            );
            assert!(!view.state.data.can_save());

            view.handle_key_down(&key_event("cmd-s"), window, cx);
        });
    });

    assert!(
        user_rx.try_recv().is_err(),
        "cmd-s on an incomplete draft must not emit SaveMcpConfig"
    );
}

#[gpui::test]
async fn oauth_draft_with_persisted_token_loads_connected_and_savable(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, cx| {
        view.handle_command(
            ViewCommand::McpConfigureDraftLoaded {
                id: Uuid::new_v4().to_string(),
                name: "OAuth MCP".to_string(),
                package: "@example/oauth-mcp".to_string(),
                package_type: crate::mcp::McpPackageType::Npm,
                runtime_hint: Some("npx".to_string()),
                env_var_name: String::new(),
                command: "npx".to_string(),
                args: vec![],
                auth_type: crate::mcp::McpAuthType::OAuth,
                oauth_connected: true,
                keyfile_path: String::new(),
                env: vec![],
                stored_secret_names: vec![],
                url: None,
            },
            cx,
        );
        // The persisted token means the draft is already connected under
        // the loaded MCP's own name and stays savable without re-running
        // the OAuth flow (even for a rename-only edit).
        assert_eq!(
            view.state.data.oauth_status,
            OAuthStatus::Connected {
                username: "OAuth MCP".to_string()
            }
        );
        assert!(view.state.data.can_save());
    });
}

#[gpui::test]
async fn persisted_env_vars_convert_plain_var_to_secret_instead_of_duplicating(
    cx: &mut TestAppContext,
) {
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, _cx| {
        let mut data = McpConfigureData::new();
        data.auth_method = McpAuthMethod::ApiKey;
        data.env_var_name = "API_KEY".to_string();
        // A plain var already carries the derived secret name.
        data.env = vec![("API_KEY".to_string(), String::new(), false)];
        view.set_mcp(data, true);

        let env_vars = view.state.data.persisted_env_vars();
        let api_key_vars: Vec<_> = env_vars
            .iter()
            .filter(|var| var.name == "API_KEY")
            .collect();
        assert_eq!(
            api_key_vars.len(),
            1,
            "the derived secret must reuse the existing plain var, got {env_vars:?}"
        );
        assert!(api_key_vars[0].is_secret);
        assert_eq!(api_key_vars[0].value, None);
    });
}

#[gpui::test]
async fn whitespace_only_api_key_is_not_treated_as_a_typed_secret(cx: &mut TestAppContext) {
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, _cx| {
        let mut data = McpConfigureData::new();
        data.auth_method = McpAuthMethod::ApiKey;
        data.env_var_name = "EXA_API_KEY".to_string();
        data.env = vec![("EXA_API_KEY".to_string(), String::new(), true)];
        data.api_key = "   ".to_string();
        view.set_mcp(data, false);

        // A whitespace-only key must not count as typed (can_save) and must
        // not be emitted, or it would overwrite a good stored key.
        assert!(
            view.state.data.typed_secrets().is_empty(),
            "whitespace-only api_key must yield no secrets payload"
        );
        assert!(
            !view.state.data.can_save(),
            "a whitespace-only key with no stored entry must not unlock save"
        );
    });
}

#[gpui::test]
async fn blocked_save_sets_reason_and_successful_save_clears_it(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpConfigureView::new);

    view.update(cx, |view: &mut McpConfigureView, cx| {
        view.set_bridge(Arc::clone(&bridge));

        // Missing name: the first can_save rule.
        view.handle_command(
            ViewCommand::McpConfigureDraftLoaded {
                id: Uuid::new_v4().to_string(),
                name: String::new(),
                package: "@example/exa".to_string(),
                package_type: crate::mcp::McpPackageType::Npm,
                runtime_hint: Some("npx".to_string()),
                env_var_name: "EXA_API_KEY".to_string(),
                command: "npx".to_string(),
                args: vec![],
                auth_type: crate::mcp::McpAuthType::ApiKey,
                oauth_connected: false,
                keyfile_path: String::new(),
                env: vec![("EXA_API_KEY".to_string(), String::new(), true)],
                stored_secret_names: vec![],
                url: None,
            },
            cx,
        );
        view.save_current(cx);
        assert_eq!(
            view.state.data.save_blocked_reason.as_deref(),
            Some("Name is required"),
            "a blocked save must surface why"
        );
        assert!(
            user_rx.try_recv().is_err(),
            "a blocked save must not emit SaveMcpConfig"
        );

        // Completing the draft (stored key satisfies the ApiKey gate) lets
        // the save through and clears the stale reason.
        view.state.data.name = "Exa".to_string();
        view.state
            .data
            .stored_secret_names
            .push("EXA_API_KEY".to_string());
        assert!(view.state.data.can_save());
        view.save_current(cx);
        assert_eq!(
            view.state.data.save_blocked_reason, None,
            "a completed save must clear the blocked reason"
        );
    });

    assert!(
        user_rx.try_recv().is_ok(),
        "the completed save must emit SaveMcpConfig"
    );
}
