//! The `ChatGPT` account row in the profile editor.
//!
//! Split from `tests.rs` to stay under this repo's 1000-line file cap.

use super::super::*;
use super::make_bridge;
use crate::presentation::view_command::{CodexAccountInfo, ViewCommand};
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
        reasoning_effort: String::new(),
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

#[test]
fn the_responses_types_do_not_offer_sampling_controls() {
    // The endpoint refuses temperature and max_output_tokens, so the client
    // omits them. Offering the fields would invite a value that is silently
    // discarded.
    assert!(!ApiType::ChatGptCodex.capabilities().sampling);
    assert!(!ApiType::ChatGptCodex.capabilities().max_tokens);
    assert!(!ApiType::OpenResponses.capabilities().sampling);
    assert!(!ApiType::OpenResponses.capabilities().max_tokens);
}

#[test]
fn every_other_type_still_offers_them() {
    for api_type in [
        ApiType::Anthropic,
        ApiType::OpenAI,
        ApiType::Local,
        ApiType::Custom("something".to_string()),
    ] {
        assert!(api_type.capabilities().sampling, "{api_type:?}");
        assert!(api_type.capabilities().max_tokens, "{api_type:?}");
    }
}

#[test]
fn the_responses_types_offer_an_effort_ladder_instead() {
    // The trade the endpoint actually makes: no sampling, but a level.
    let caps = ApiType::ChatGptCodex.capabilities();
    assert!(caps.takes_reasoning_effort());
    assert!(caps.accepts(&crate::models::ReasoningEffort::XHigh));
}

#[test]
fn a_budget_provider_is_not_given_an_effort_control() {
    // Two axes, not one. Anthropic takes a token budget and no level.
    let caps = ApiType::Anthropic.capabilities();
    assert!(caps.thinking_budget);
    assert!(!caps.takes_reasoning_effort());
}

fn account_choice(slug: &str, label: &str) -> CodexAccountInfo {
    CodexAccountInfo {
        account: slug.to_string(),
        label: label.to_string(),
        plan: Some("pro".to_string()),
        needs_reauth: false,
        expires_in_secs: Some(3600),
        used_by: vec![],
    }
}

#[gpui::test]
async fn an_account_signed_in_elsewhere_can_be_attached(cx: &mut gpui::TestAppContext) {
    // Adding an account from Settings left existing profiles with no way to
    // use it: the row only offered a fresh sign-in.
    let (bridge, _rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |this, cx| {
        this.set_bridge(bridge);
        this.state.data.api_type = ApiType::ChatGptCodex;
        // can_save also wants a name and a model; this test is about the
        // account, so give it the rest.
        this.state.data.name = "Codex".to_string();
        this.state.data.model_id = "gpt-5.6-luna".to_string();
        this.state.data.base_url = "wss://chatgpt.com/backend-api/codex/responses".to_string();
        this.handle_command(
            ViewCommand::CodexAccountsListed {
                accounts: vec![account_choice("chatgpt-a", "a@example.com")],
                unreadable: 0,
            },
            cx,
        );

        assert!(this.state.data.oauth_account.is_empty());
        assert!(!this.state.data.can_save(), "no account attached yet");

        this.cycle_oauth_account();

        assert_eq!(this.state.data.oauth_account, "chatgpt-a");
        assert_eq!(this.state.data.oauth_account_label, "a@example.com");
        assert_eq!(this.state.data.oauth_account_plan, "pro");
        assert!(
            this.state.data.can_save(),
            "an attached account makes the profile valid"
        );
    });
}

#[gpui::test]
async fn switching_walks_the_accounts(cx: &mut gpui::TestAppContext) {
    let (bridge, _rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |this, cx| {
        this.set_bridge(bridge);
        this.state.data.api_type = ApiType::ChatGptCodex;
        this.handle_command(
            ViewCommand::CodexAccountsListed {
                accounts: vec![
                    account_choice("chatgpt-a", "a@example.com"),
                    account_choice("chatgpt-b", "b@example.com"),
                ],
                unreadable: 0,
            },
            cx,
        );

        this.cycle_oauth_account();
        assert_eq!(this.state.data.oauth_account, "chatgpt-a");
        this.cycle_oauth_account();
        assert_eq!(this.state.data.oauth_account, "chatgpt-b");
        this.cycle_oauth_account();
        assert_eq!(this.state.data.oauth_account, "chatgpt-a", "wraps around");
    });
}

#[gpui::test]
async fn a_loaded_profile_gets_its_account_details_from_the_list(cx: &mut gpui::TestAppContext) {
    // A profile stores only the slug, so the row has nothing to show until
    // the accounts arrive.
    let (bridge, _rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |this, cx| {
        this.set_bridge(bridge);
        this.state.data.api_type = ApiType::ChatGptCodex;
        this.state.data.oauth_account = "chatgpt-b".to_string();

        this.handle_command(
            ViewCommand::CodexAccountsListed {
                accounts: vec![
                    account_choice("chatgpt-a", "a@example.com"),
                    account_choice("chatgpt-b", "b@example.com"),
                ],
                unreadable: 0,
            },
            cx,
        );

        assert_eq!(this.state.data.oauth_account_label, "b@example.com");
        assert_eq!(this.state.data.oauth_account_plan, "pro");
    });
}

#[gpui::test]
async fn cycling_with_no_accounts_changes_nothing(cx: &mut gpui::TestAppContext) {
    let (bridge, _rx) = make_bridge();
    let view = cx.new(ProfileEditorView::new);

    view.update(cx, |this, _cx| {
        this.set_bridge(bridge);
        this.state.data.api_type = ApiType::ChatGptCodex;

        this.cycle_oauth_account();

        assert!(this.state.data.oauth_account.is_empty());
    });
}

/// A codex `ProfileEditorLoad` carrying a stored effort and a budget.
fn load_with_effort(effort: &str, budget: Option<u32>) -> ViewCommand {
    match profile_load_command("someone") {
        ViewCommand::ProfileEditorLoad {
            id,
            name,
            provider_id,
            model_id,
            base_url,
            api_key_label,
            oauth_account,
            temperature,
            max_tokens,
            max_tokens_field_name,
            extra_request_fields,
            context_limit,
            show_thinking,
            system_prompt,
            ..
        } => ViewCommand::ProfileEditorLoad {
            id,
            name,
            provider_id,
            model_id,
            base_url,
            api_key_label,
            oauth_account,
            temperature,
            max_tokens,
            max_tokens_field_name,
            extra_request_fields,
            context_limit,
            show_thinking,
            system_prompt,
            enable_thinking: true,
            thinking_budget: budget,
            reasoning_effort: effort.to_string(),
        },
        other => other,
    }
}

#[gpui::test]
async fn a_stored_effort_survives_a_trip_through_the_editor(cx: &mut TestAppContext) {
    // The level the user picked comes back out as picked, rather than being
    // rebuilt from a token count.
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.handle_command(load_with_effort("xhigh", Some(1_024)), cx);

        assert_eq!(
            view.state.data.reasoning_effort,
            Some(crate::models::ReasoningEffort::XHigh),
            "a small budget must not drag the level back down"
        );
    });
}

#[gpui::test]
async fn a_profile_without_a_stored_effort_takes_the_default(cx: &mut TestAppContext) {
    // A profile written before the setting existed has no level, and keeps
    // none: the backend applies its own default rather than this client
    // recording a choice nobody made.
    let (bridge, _events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ProfileEditorView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut ProfileEditorView, cx| {
        view.handle_command(load_with_effort("", Some(64_000)), cx);

        assert_eq!(
            view.state.data.reasoning_effort, None,
            "a large budget must not be reinterpreted as a level"
        );
    });
}
