//! The signed-in `ChatGPT` accounts list under Models.

use gpui::{AppContext, TestAppContext};

use super::super::SettingsView;
use crate::events::types::UserEvent;
use crate::presentation::view_command::{CodexAccountInfo, ViewCommand};
use crate::ui_gpui::bridge::GpuiBridge;
use std::sync::Arc;

fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

fn account(slug: &str, needs_reauth: bool, used_by: Vec<&str>) -> CodexAccountInfo {
    CodexAccountInfo {
        account: slug.to_string(),
        label: "andrew@example.com".to_string(),
        plan: Some("ChatGPT Pro".to_string()),
        needs_reauth,
        expires_in_secs: Some(2_400),
        used_by: used_by.into_iter().map(str::to_owned).collect(),
    }
}

#[gpui::test]
async fn listed_accounts_land_in_state(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view: &mut SettingsView, cx| {
        view.handle_command(
            ViewCommand::CodexAccountsListed {
                accounts: vec![account("chatgpt-a", false, vec!["Codex"])],
            },
            cx,
        );

        assert_eq!(view.state.accounts.len(), 1);
        assert_eq!(view.state.accounts[0].account, "chatgpt-a");
    });
}

#[gpui::test]
async fn a_later_list_replaces_the_earlier_one(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view: &mut SettingsView, cx| {
        view.handle_command(
            ViewCommand::CodexAccountsListed {
                accounts: vec![account("chatgpt-a", false, vec![])],
            },
            cx,
        );
        view.handle_command(
            ViewCommand::CodexAccountsListed {
                accounts: Vec::new(),
            },
            cx,
        );

        assert!(view.state.accounts.is_empty());
    });
}

#[gpui::test]
async fn signing_out_names_the_account(cx: &mut TestAppContext) {
    let (bridge, events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = SettingsView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view: &mut SettingsView, _cx| {
        view.emit(&UserEvent::SignOutCodexAccount {
            account: "chatgpt-a".to_string(),
        });
    });

    let mut seen = None;
    while let Ok(event) = events.try_recv() {
        if matches!(event, UserEvent::SignOutCodexAccount { .. }) {
            seen = Some(event);
        }
    }
    assert_eq!(
        seen,
        Some(UserEvent::SignOutCodexAccount {
            account: "chatgpt-a".to_string()
        })
    );
}

#[test]
fn a_healthy_account_reports_how_long_it_has_left() {
    let status = SettingsView::account_status_line(&account("chatgpt-a", false, vec![]));

    assert_eq!(status, "Signed in, 40 minutes left");
}

#[test]
fn an_expired_account_says_so() {
    let status = SettingsView::account_status_line(&account("chatgpt-a", true, vec![]));

    assert_eq!(status, "Session expired");
}

#[test]
fn an_account_without_an_expiry_just_reports_signed_in() {
    let mut info = account("chatgpt-a", false, vec![]);
    info.expires_in_secs = None;

    assert_eq!(SettingsView::account_status_line(&info), "Signed in");
}

#[test]
fn an_account_at_zero_reports_renewing_rather_than_expired() {
    let mut info = account("chatgpt-a", false, vec![]);
    info.expires_in_secs = Some(0);

    assert_eq!(SettingsView::account_status_line(&info), "Renewing");
}

#[test]
fn usage_counts_read_naturally() {
    assert_eq!(
        SettingsView::account_usage_line(&account("a", false, vec![])),
        "Not used by any profile"
    );
    assert_eq!(
        SettingsView::account_usage_line(&account("a", false, vec!["Codex"])),
        "Used by 1 profile"
    );
    assert_eq!(
        SettingsView::account_usage_line(&account("a", false, vec!["Codex", "Mini"])),
        "Used by 2 profiles"
    );
}

#[gpui::test]
async fn opening_the_models_panel_asks_for_the_account_list(cx: &mut TestAppContext) {
    // Presenters start before any view exists, so an account list published at
    // startup reaches nobody, and reading the keychain at startup stalls the
    // launch. The panel that shows accounts asks for them when it opens.
    let (bridge, events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = SettingsView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view, _cx| {
        view.select_category(super::SettingsCategory::Models);
    });

    let emitted: Vec<UserEvent> = events.try_iter().collect();
    assert!(
        emitted.contains(&UserEvent::ListCodexAccounts),
        "opening Models should request the account list, got {emitted:?}"
    );
}

#[gpui::test]
async fn other_panels_do_not_touch_the_keychain(cx: &mut TestAppContext) {
    let (bridge, events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = SettingsView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view, _cx| {
        view.select_category(super::SettingsCategory::Appearance);
    });

    let emitted: Vec<UserEvent> = events.try_iter().collect();
    assert!(
        !emitted.contains(&UserEvent::ListCodexAccounts),
        "only the Models panel shows accounts, got {emitted:?}"
    );
}

#[gpui::test]
async fn signing_in_from_settings_opens_the_sheet(cx: &mut TestAppContext) {
    // The presenter only publishes sign-in state; nothing in that path changes
    // the current view. Without an explicit navigation the user stays on
    // Settings while a browser opens with no explanation.
    while crate::ui_gpui::navigation_channel()
        .take_pending()
        .is_some()
    {}
    let (bridge, events) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = SettingsView::new(cx);
        view.set_bridge(bridge);
        view
    });

    view.update(cx, |view, _cx| view.start_codex_sign_in());

    let emitted: Vec<UserEvent> = events.try_iter().collect();
    assert!(
        emitted.iter().any(|event| matches!(
            event,
            UserEvent::StartCodexSignIn {
                method: crate::events::types::CodexSignInMethod::Browser
            }
        )),
        "should ask for a browser sign-in, got {emitted:?}"
    );
    assert_eq!(
        crate::ui_gpui::navigation_channel().take_pending(),
        Some(crate::presentation::view_command::ViewId::CodexSignIn),
        "should show the sheet that reports progress"
    );
}
