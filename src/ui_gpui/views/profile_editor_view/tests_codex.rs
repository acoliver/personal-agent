//! Codex sign-in and account tests for the profile editor.
//!
//! Split from `tests.rs` to stay under this repo's 1000-line file cap.

use super::super::*;
use super::clear_navigation_requests;
use super::make_bridge;
use gpui::{AppContext, TestAppContext};

#[gpui::test]
async fn a_codex_profile_needs_an_account_not_a_key(cx: &mut TestAppContext) {
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.state.data.name = "Codex".to_string();
        view.state.data.model_id = "gpt-5.6-luna".to_string();
        view.state.data.api_type = ApiType::ChatGptCodex;
        view.state.data.apply_api_type_change();

        assert!(!view.state.data.api_type.requires_api_key());
        assert!(view.state.data.api_type.requires_oauth_account());
        assert!(
            !view.state.data.can_save(),
            "Save stays disabled until an account is signed in"
        );

        view.state.data.oauth_account = "chatgpt-acct-1".to_string();
        assert!(view.state.data.can_save());
    });
}

#[gpui::test]
async fn choosing_codex_fills_in_the_managed_endpoint(cx: &mut TestAppContext) {
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.state.data.base_url = "https://example.test/v1".to_string();
        view.state.data.api_type = ApiType::ChatGptCodex;
        view.state.data.apply_api_type_change();

        assert_eq!(
            view.state.data.base_url,
            "wss://chatgpt.com/backend-api/codex/responses"
        );
    });
}

#[gpui::test]
async fn leaving_codex_drops_the_account(cx: &mut TestAppContext) {
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.state.data.api_type = ApiType::ChatGptCodex;
        view.state.data.oauth_account = "chatgpt-acct-1".to_string();
        view.state.data.oauth_account_label = "a@b.c".to_string();

        view.state.data.api_type = ApiType::OpenAI;
        view.state.data.apply_api_type_change();

        assert!(
            view.state.data.oauth_account.is_empty(),
            "a key-authenticated provider cannot use an account"
        );
        assert!(view.state.data.oauth_account_label.is_empty());
    });
}

#[gpui::test]
async fn leaving_a_key_provider_drops_the_key_label(cx: &mut TestAppContext) {
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.state.data.api_type = ApiType::OpenAI;
        view.state.data.key_label = "openai-key".to_string();

        view.state.data.api_type = ApiType::ChatGptCodex;
        view.state.data.apply_api_type_change();

        assert!(view.state.data.key_label.is_empty());
    });
}

#[gpui::test]
async fn signing_in_asks_for_a_browser_flow(cx: &mut TestAppContext) {
    clear_navigation_requests();
    let (bridge, events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    while events.try_recv() == Ok(UserEvent::RefreshApiKeys) {}

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.start_codex_sign_in();
    });

    assert_eq!(
        events.try_recv().expect("event emitted"),
        UserEvent::StartCodexSignIn {
            method: crate::events::types::CodexSignInMethod::Browser
        }
    );

    // Starting a sign-in also navigates to the sheet. The navigation channel
    // is a single global slot shared by every test in this binary, so a
    // request left behind here surfaces as a stray navigation in another test.
    clear_navigation_requests();
}

#[gpui::test]
async fn a_completed_sign_in_populates_the_account_row(cx: &mut TestAppContext) {
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.state.data.api_type = ApiType::ChatGptCodex;
        // Selecting the type is what fills in its managed endpoint.
        view.state.data.apply_api_type_change();
        view.handle_command(
            ViewCommand::CodexSignInCompleted {
                account: "chatgpt-acct-1".to_string(),
                label: "andrew@example.com".to_string(),
                plan: Some("ChatGPT Pro".to_string()),
            },
            cx,
        );

        assert_eq!(view.state.data.oauth_account, "chatgpt-acct-1");
        assert_eq!(view.state.data.oauth_account_label, "andrew@example.com");
        assert_eq!(view.state.data.oauth_account_plan, "ChatGPT Pro");
        // A signed-in account is the credential this API type needs, so with
        // the other required fields present Save becomes available.
        view.state.data.name = "Codex".to_string();
        view.state.data.model_id = "gpt-5.6-luna".to_string();
        assert!(view.state.data.can_save());
    });
}

#[gpui::test]
async fn signing_out_clears_the_account_and_tells_the_presenter(cx: &mut TestAppContext) {
    let (bridge, events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    while events.try_recv() == Ok(UserEvent::RefreshApiKeys) {}

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.state.data.api_type = ApiType::ChatGptCodex;
        view.state.data.oauth_account = "chatgpt-acct-1".to_string();

        view.sign_out_codex_account("chatgpt-acct-1".to_string());

        assert!(view.state.data.oauth_account.is_empty());
        assert!(!view.state.data.can_save());
    });

    assert_eq!(
        events.try_recv().expect("event emitted"),
        UserEvent::SignOutCodexAccount {
            account: "chatgpt-acct-1".to_string()
        }
    );
}

#[gpui::test]
async fn a_codex_profile_loads_with_its_account(cx: &mut TestAppContext) {
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.handle_command(
            ViewCommand::ProfileEditorLoad {
                id: Uuid::new_v4(),
                name: "Codex".to_string(),
                provider_id: "openai-codex".to_string(),
                model_id: "gpt-5.6-luna".to_string(),
                base_url: "wss://chatgpt.com/backend-api/codex/responses".to_string(),
                api_key_label: String::new(),
                oauth_account: "chatgpt-acct-1".to_string(),
                temperature: 1.0,
                max_tokens: Some(4096),
                max_tokens_field_name: String::new(),
                extra_request_fields: String::new(),
                context_limit: Some(128_000),
                show_thinking: false,
                enable_thinking: false,
                thinking_budget: None,
                reasoning_effort: String::new(),
                system_prompt: String::new(),
            },
            cx,
        );

        assert_eq!(view.state.data.api_type, ApiType::ChatGptCodex);
        assert_eq!(view.state.data.oauth_account, "chatgpt-acct-1");
        assert!(view.state.data.can_save());
    });
}
