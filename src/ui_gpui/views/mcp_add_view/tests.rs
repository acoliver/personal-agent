#![allow(clippy::future_not_send)]

use super::*;
use flume;
use gpui::{AppContext, EntityInputHandler, TestAppContext};

use crate::events::types::UserEvent;
use crate::presentation::view_command::{ErrorSeverity, ViewCommand, ViewId};

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
async fn emit_search_registry_trims_query_and_reports_registry_source(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpAddView::new);

    view.update(cx, |view: &mut McpAddView, _cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.set_search_query("  fetch registry  ".to_string());
        view.state.registry = McpRegistry::Smithery;
        view.emit_search_registry();
    });

    assert_eq!(
        user_rx.recv().expect("search registry event"),
        UserEvent::SearchMcpRegistry {
            query: "fetch registry".to_string(),
            source: crate::events::types::McpRegistrySource {
                name: "smithery".to_string(),
            },
        }
    );

    view.read_with(cx, |view, _| {
        assert_eq!(view.get_state().search_state, SearchState::Loading);
    });
}

#[gpui::test]
async fn draft_loaded_preserves_transport_metadata_and_requests_configure_navigation(
    cx: &mut TestAppContext,
) {
    clear_navigation_requests();
    let view = cx.new(McpAddView::new);

    view.update(cx, |view: &mut McpAddView, cx| {
        view.handle_command(
            ViewCommand::McpConfigureDraftLoaded {
                id: "smithery::fetch".to_string(),
                name: "Fetch".to_string(),
                package: "@smithery/fetch".to_string(),
                package_type: crate::mcp::McpPackageType::Npm,
                runtime_hint: Some("npx".to_string()),
                env_var_name: "FETCH_API_KEY".to_string(),
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-fetch".to_string(),
                ],
                auth_type: crate::mcp::McpAuthType::ApiKey,
                oauth_connected: false,
                keyfile_path: String::new(),
                env: vec![("FETCH_API_KEY".to_string(), String::new(), true)],
                stored_secret_names: vec![],
                url: None,
            },
            cx,
        );
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(ViewId::McpConfigure)
        );
    });

    view.read_with(cx, |view, _| {
        let state = view.get_state();
        assert_eq!(
            state.manual_entry,
            "npx -y @modelcontextprotocol/server-fetch"
        );
        assert_eq!(state.selected_result_id.as_deref(), Some("fetch"));
        assert_eq!(state.search_state, SearchState::Results);
        assert_eq!(state.results.len(), 1);
        let result = &state.results[0];
        assert_eq!(result.registry, McpRegistry::Smithery);
        assert_eq!(result.source, "smithery");
        assert_eq!(result.command, "@smithery/fetch");
        assert_eq!(
            result.args,
            vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-fetch".to_string()
            ]
        );
        assert_eq!(
            result.env,
            Some(vec![("FETCH_API_KEY".to_string(), String::new())])
        );
        assert_eq!(result.package_type, Some(crate::mcp::McpPackageType::Npm));
        assert_eq!(result.runtime_hint.as_deref(), Some("npx"));
        assert!(state.can_proceed());
    });
}

#[gpui::test]
async fn registry_results_and_errors_update_search_state_without_source_loss(
    cx: &mut TestAppContext,
) {
    let view = cx.new(McpAddView::new);

    view.update(cx, |view: &mut McpAddView, cx| {
        view.set_search_query("fetch".to_string());
        view.handle_command(
            ViewCommand::McpRegistrySearchResults {
                results: vec![
                    crate::presentation::view_command::McpRegistryResult {
                        id: "fetch".to_string(),
                        name: "Fetch".to_string(),
                        description: "HTTP fetch server".to_string(),
                        source: "smithery".to_string(),
                        command: "npx".to_string(),
                        args: vec![
                            "-y".to_string(),
                            "@modelcontextprotocol/server-fetch".to_string(),
                        ],
                        env: Some(vec![("FETCH_API_KEY".to_string(), String::new())]),
                        package_type: Some(crate::mcp::McpPackageType::Npm),
                        runtime_hint: Some("npx".to_string()),
                        url: None,
                    },
                    crate::presentation::view_command::McpRegistryResult {
                        id: "exa".to_string(),
                        name: "Exa".to_string(),
                        description: "Remote MCP".to_string(),
                        source: "official".to_string(),
                        command: String::new(),
                        args: vec![],
                        env: None,
                        package_type: Some(crate::mcp::McpPackageType::Http),
                        runtime_hint: None,
                        url: Some("https://exa.example/mcp".to_string()),
                    },
                ],
            },
            cx,
        );

        assert_eq!(view.get_state().results.len(), 2);
        assert_eq!(view.get_state().search_state, SearchState::Results);
        assert_eq!(view.get_state().results[0].registry, McpRegistry::Smithery);
        assert_eq!(view.get_state().results[0].source, "smithery");
        assert_eq!(
            view.get_state().results[1].url.as_deref(),
            Some("https://exa.example/mcp")
        );

        view.handle_command(
            ViewCommand::ShowError {
                title: "search failed".to_string(),
                message: "registry unavailable".to_string(),
                severity: ErrorSeverity::Warning,
            },
            cx,
        );
        assert_eq!(
            view.get_state().search_state,
            SearchState::Error("registry unavailable".to_string())
        );
    });
}

#[gpui::test]
async fn manual_entry_registry_switch_and_empty_search_follow_real_state_rules(
    cx: &mut TestAppContext,
) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpAddView::new);

    view.update(cx, |view: &mut McpAddView, _cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.set_results(vec![
            McpSearchResult::new("fetch", "Fetch", "HTTP fetch")
                .with_registry(McpRegistry::Official)
                .with_command("npx")
                .with_args(vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-fetch".to_string(),
                ]),
            McpSearchResult::new("exa", "Exa", "Remote search")
                .with_registry(McpRegistry::Smithery)
                .with_url(Some("https://exa.example/mcp".to_string())),
        ]);
        view.state.selected_result_id = Some("fetch".to_string());
        assert!(view.get_state().can_proceed());

        view.set_manual_entry(" custom-mcp ".to_string());
        assert_eq!(view.get_state().manual_entry, " custom-mcp ");
        assert_eq!(view.get_state().selected_result_id, None);
        assert!(view.get_state().can_proceed());

        view.set_search_query("exa".to_string());
        view.state.selected_result_id = Some("exa".to_string());
        view.select_registry(McpRegistry::Official);
        assert_eq!(view.get_state().registry, McpRegistry::Official);
        assert_eq!(view.get_state().selected_result_id, None);
        assert_eq!(view.get_state().search_state, SearchState::Loading);

        view.set_loading(false);
        assert_eq!(view.get_state().search_state, SearchState::Results);

        view.set_search_query("   ".to_string());
        view.select_registry(McpRegistry::Both);
        assert_eq!(view.get_state().registry, McpRegistry::Both);
        assert!(view.get_state().results.is_empty());
        assert_eq!(view.get_state().search_state, SearchState::Idle);
    });

    assert_eq!(
        user_rx.recv().expect("registry switch search"),
        UserEvent::SearchMcpRegistry {
            query: "exa".to_string(),
            source: crate::events::types::McpRegistrySource {
                name: "official".to_string(),
            },
        }
    );
    assert!(
        user_rx.try_recv().is_err(),
        "unexpected additional MCP events"
    );
}

#[gpui::test]
async fn set_results_filters_selection_and_command_preview_handles_remote_urls(
    cx: &mut TestAppContext,
) {
    let view = cx.new(McpAddView::new);

    view.update(cx, |view: &mut McpAddView, _cx| {
        view.set_search_query("exa".to_string());
        view.set_results(vec![
            McpSearchResult::new("fetch", "Fetch", "HTTP fetch")
                .with_registry(McpRegistry::Official)
                .with_command("npx")
                .with_args(vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-fetch".to_string(),
                ]),
            McpSearchResult::new("exa", "Exa", "Remote search")
                .with_registry(McpRegistry::Smithery)
                .with_command("npx")
                .with_args(vec!["-y".to_string(), "exa-mcp".to_string()])
                .with_url(Some("https://exa.example/mcp".to_string())),
        ]);
        view.state.selected_result_id = Some("exa".to_string());
        view.state.registry = McpRegistry::Smithery;
        assert_eq!(view.filtered_results().len(), 1);
        assert_eq!(view.filtered_results()[0].id, "exa");
        assert_eq!(
            McpAddView::command_preview(&view.filtered_results()[0]),
            "https://exa.example/mcp"
        );

        view.set_results(vec![McpSearchResult::new("fetch", "Fetch", "HTTP fetch")
            .with_registry(McpRegistry::Official)
            .with_command("npx")
            .with_args(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-fetch".to_string(),
            ])]);
        assert_eq!(view.get_state().selected_result_id, None);
        assert_eq!(view.get_state().search_state, SearchState::Empty);

        view.set_search_query("fetch".to_string());
        view.state.registry = McpRegistry::Official;
        view.set_results(vec![McpSearchResult::new("fetch", "Fetch", "HTTP fetch")
            .with_registry(McpRegistry::Official)
            .with_command("npx")
            .with_args(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-fetch".to_string(),
            ])]);
        assert_eq!(view.get_state().search_state, SearchState::Results);
        assert_eq!(view.filtered_results().len(), 1);
        assert_eq!(
            McpAddView::command_preview(&view.filtered_results()[0]),
            "npx -y @modelcontextprotocol/server-fetch"
        );
    });
}

fn key_event(key: &str) -> gpui::KeyDownEvent {
    gpui::KeyDownEvent {
        keystroke: gpui::Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
        is_held: false,
        prefer_character_input: false,
    }
}

fn setup_view_with_results(view: &mut McpAddView, bridge: &Arc<GpuiBridge>) {
    view.set_bridge(Arc::clone(bridge));
    view.set_results(vec![
        McpSearchResult::new("fetch", "Fetch", "HTTP fetch")
            .with_registry(McpRegistry::Official)
            .with_command("npx")
            .with_args(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-fetch".to_string(),
            ]),
        McpSearchResult::new("exa", "Exa", "Remote search")
            .with_registry(McpRegistry::Smithery)
            .with_url(Some("https://exa.example/mcp".to_string())),
    ]);
}

#[gpui::test]
async fn text_input_and_ime_handling_emits_search_events(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpAddView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpAddView, cx| {
            setup_view_with_results(view, &bridge);

            view.state.active_field = Some(ActiveField::ManualEntry);
            view.handle_key_down(&key_event("tab"), cx);
            assert_eq!(
                view.get_state().active_field,
                Some(ActiveField::SearchQuery)
            );

            view.replace_text_in_range(None, "exa", window, cx);
            assert_eq!(view.get_state().search_query, "exa");
            assert_eq!(
                view.text_for_range(0..2, &mut None, window, cx),
                Some("ex".to_string())
            );

            view.replace_and_mark_text_in_range(None, "!", None, window, cx);
            assert_eq!(view.get_state().search_query, "exa!");
            assert_eq!(view.marked_text_range(window, cx), Some(3..4));
            view.replace_text_in_range(None, "-mcp", window, cx);
            assert_eq!(view.get_state().search_query, "exa-mcp");
            assert_eq!(view.marked_text_range(window, cx), None);

            let selected = view
                .selected_text_range(false, window, cx)
                .expect("selection range");
            let len = "exa-mcp".encode_utf16().count();
            assert_eq!(selected.range, len..len);
        });
    });

    let expected_queries = ["exa", "exa!", "exa-mcp"];
    for query in expected_queries {
        assert_eq!(
            user_rx.recv().expect("search registry event"),
            UserEvent::SearchMcpRegistry {
                query: query.to_string(),
                source: crate::events::types::McpRegistrySource {
                    name: "both".to_string(),
                },
            }
        );
    }
    assert!(
        user_rx.try_recv().is_err(),
        "unexpected additional search registry events"
    );
}

#[gpui::test]
async fn dropdown_select_and_navigation_keys_behave_correctly(cx: &mut TestAppContext) {
    let (bridge, _user_rx) = make_bridge();
    let view = cx.new(McpAddView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut McpAddView, cx| {
            setup_view_with_results(view, &bridge);
            view.state.search_query = "exa-mcp".to_string();

            view.state.show_registry_dropdown = true;
            view.handle_key_down(&key_event("enter"), cx);
            assert!(!view.get_state().show_registry_dropdown);

            view.toggle_registry_dropdown(cx);
            assert!(view.get_state().show_registry_dropdown);
            // flush any navigation requests from concurrent tests before asserting None
            clear_navigation_requests();
            view.handle_key_down(&key_event("escape"), cx);
            assert!(!view.get_state().show_registry_dropdown);
            assert_eq!(crate::ui_gpui::navigation_channel().take_pending(), None);

            view.select_result("exa".to_string(), cx);
            assert_eq!(view.get_state().selected_result_id.as_deref(), Some("exa"));
            assert!(view.get_state().manual_entry.is_empty());

            view.handle_key_down(&key_event("backspace"), cx);
            assert_eq!(view.get_state().search_query, "exa-mcp");
            assert_eq!(view.get_state().selected_result_id, Some("exa".to_string()));

            clear_navigation_requests();
            view.handle_key_down(&key_event("cmd-w"), cx);
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::Settings)
            );
        });
    });
}

#[gpui::test]
async fn paste_appends_to_active_fields_sanitized_and_ignores_no_field(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpAddView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut McpAddView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("npx foo\r\n".to_string()));

            view.state.active_field = Some(ActiveField::ManualEntry);
            view.handle_key_down(&key_event("cmd-v"), cx);
            assert_eq!(view.get_state().manual_entry, "npx foo");

            view.state.active_field = Some(ActiveField::SearchQuery);
            view.handle_key_down(&key_event("cmd-v"), cx);
            assert_eq!(view.get_state().search_query, "npx foo");
            assert_eq!(view.get_state().selected_result_id, None);

            view.state.active_field = None;
            view.handle_key_down(&key_event("cmd-v"), cx);
            assert_eq!(view.get_state().search_query, "npx foo");
        });
    });

    assert_eq!(
        user_rx.recv().expect("search registry event after paste"),
        UserEvent::SearchMcpRegistry {
            query: "npx foo".to_string(),
            source: crate::events::types::McpRegistrySource {
                name: "both".to_string(),
            },
        }
    );
    assert!(
        user_rx.try_recv().is_err(),
        "paste must not emit events beyond the search refresh"
    );
}

#[gpui::test]
async fn backspace_during_ime_mark_keeps_truncation_on_char_boundaries(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(McpAddView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpAddView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.state.active_field = Some(ActiveField::ManualEntry);
            view.state.manual_entry = "aあ".to_string();

            // Mark the ASCII composition tail "bc" (2 bytes).
            view.replace_and_mark_text_in_range(None, "bc", None, window, cx);
            assert_eq!(view.get_state().manual_entry, "aあbc");
            assert_eq!(view.ime_marked_byte_count, 2);

            // Backspace pops 'c': the marked tail shrinks to 1 byte.
            view.handle_key_down(&key_event("backspace"), cx);
            assert_eq!(view.get_state().manual_entry, "aあb");
            assert_eq!(view.ime_marked_byte_count, 1);

            // Pasting replaces the remaining marked byte. A stale counter
            // of 2 would cut inside the 3-byte 'あ' and panic; the guard
            // also keeps arbitrary byte splits from ever truncating there.
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("XY".to_string()));
            view.handle_key_down(&key_event("cmd-v"), cx);
            assert_eq!(view.get_state().manual_entry, "aあXY");
            assert_eq!(view.ime_marked_byte_count, 0);
        });
    });

    assert!(
        user_rx.try_recv().is_err(),
        "manual-entry editing must not emit search events"
    );
}

#[gpui::test]
async fn multibyte_marked_text_backspaces_clear_without_panicking(cx: &mut TestAppContext) {
    let view = cx.new(McpAddView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpAddView, cx| {
            view.state.active_field = Some(ActiveField::ManualEntry);
            view.replace_text_in_range(None, "a", window, cx);
            view.replace_and_mark_text_in_range(None, "こんにちは", None, window, cx);
            assert_eq!(view.get_state().manual_entry, "aこんにちは");
            assert_eq!(view.ime_marked_byte_count, 15);
            assert_eq!(view.marked_text_range(window, cx), Some(1..6));

            // Each backspace pops one 3-byte char and shrinks the mark.
            for expected in (0..15).step_by(3).rev() {
                view.handle_key_down(&key_event("backspace"), cx);
                assert_eq!(view.ime_marked_byte_count, expected);
            }
            assert_eq!(view.get_state().manual_entry, "a");
            assert_eq!(view.marked_text_range(window, cx), None);
        });
    });
}

#[gpui::test]
async fn paste_during_multibyte_ime_mark_replaces_marked_text(cx: &mut TestAppContext) {
    let view = cx.new(McpAddView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut McpAddView, cx| {
            view.state.active_field = Some(ActiveField::ManualEntry);
            view.replace_text_in_range(None, "hello ", window, cx);
            view.replace_and_mark_text_in_range(None, "わに", None, window, cx);
            assert_eq!(view.get_state().manual_entry, "hello わに");
            assert_eq!(view.ime_marked_byte_count, 6);

            // The paste replaces the multibyte marked range instead of
            // appending after it.
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("world".to_string()));
            view.handle_key_down(&key_event("cmd-v"), cx);
            assert_eq!(view.get_state().manual_entry, "hello world");
            assert_eq!(view.ime_marked_byte_count, 0);
        });
    });
}
