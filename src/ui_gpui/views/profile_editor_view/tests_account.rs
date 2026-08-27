//! The `ChatGPT` account row in the profile editor.
//!
//! Split from `tests.rs` to stay under this repo's 1000-line file cap.

use super::super::*;
use super::make_bridge;
use crate::presentation::view_command::ViewCommand;
use gpui::{AppContext, TestAppContext};

/// A `ProfileEditorLoad` for a codex profile bound to `account`.
fn profile_load_command(account: &str) -> ViewCommand {
    ViewCommand::ProfileEditorLoad {
        id: uuid::Uuid::new_v4(),
        name: "Codex".to_string(),
        provider_id: "openai-codex".to_string(),
        model_id: "gpt-5.6-luna".to_string(),
        base_url: "wss://chatgpt.com/backend-api/codex/responses".to_string(),
        api_key_label: String::new(),
        oauth_account: account.to_string(),
        temperature: 1.0,
        max_tokens: Some(256),
        max_tokens_field_name: "max_tokens".to_string(),
        extra_request_fields: "{}".to_string(),
        context_limit: Some(128_000),
        show_thinking: false,
        enable_thinking: false,
        thinking_budget: None,
        system_prompt: String::new(),
    }
}

#[gpui::test]
async fn leaving_chatgpt_drops_its_managed_endpoint(cx: &mut TestAppContext) {
    // The websocket URL belongs to ChatGPT. Carrying it onto a provider that
    // cannot serve it would still let Save light up, because the field is not
    // empty, and persist an endpoint that can never work.
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.state.data.api_type = ApiType::ChatGptCodex;
        view.state.data.apply_api_type_change();
        assert_eq!(
            view.state.data.base_url,
            "wss://chatgpt.com/backend-api/codex/responses"
        );

        view.state.data.api_type = ApiType::OpenResponses;
        view.state.data.apply_api_type_change();

        assert_ne!(
            view.state.data.base_url, "wss://chatgpt.com/backend-api/codex/responses",
            "the managed endpoint must not follow the user to another type"
        );
    });
}

#[gpui::test]
async fn an_endpoint_the_user_typed_survives_a_type_change(cx: &mut TestAppContext) {
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, _cx| {
        view.state.data.api_type = ApiType::OpenResponses;
        view.state.data.base_url = "wss://my-own-host.example/v1/responses".to_string();

        view.state.data.apply_api_type_change();

        assert_eq!(
            view.state.data.base_url,
            "wss://my-own-host.example/v1/responses"
        );
    });
}

#[gpui::test]
async fn loading_a_profile_drops_the_previous_account_caption(cx: &mut TestAppContext) {
    // The load payload carries the slug only, so a stale label would caption
    // this account with the previously loaded one's name.
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.state.data.oauth_account_label = "someone@example.com".to_string();
        view.state.data.oauth_account_plan = "ChatGPT Pro".to_string();

        view.handle_command(profile_load_command("chatgpt-acct-2"), cx);

        assert_eq!(view.state.data.oauth_account, "chatgpt-acct-2");
        assert!(view.state.data.oauth_account_label.is_empty());
        assert!(view.state.data.oauth_account_plan.is_empty());
    });
}
