//! Behaviour of the `ChatGPT` sign-in sheet.

use std::sync::Arc;

use super::{CodexSignInView, SignInPhase};
use crate::events::types::{CodexSignInMethod, UserEvent};
use crate::presentation::view_command::{CodexSignInFailure, ViewCommand};
use crate::ui_gpui::bridge::GpuiBridge;

fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

fn started(method: CodexSignInMethod, copy: Option<&str>, fell_back: bool) -> ViewCommand {
    ViewCommand::CodexSignInStarted {
        method,
        url: "https://auth.openai.com/codex/device".to_string(),
        user_code: copy.map(str::to_owned),
        copy_to_clipboard: copy.map(str::to_owned),
        expires_in_secs: 900,
        fell_back,
    }
}

#[test]
fn a_browser_sign_in_renders_the_authorize_link_and_no_code() {
    let mut view = CodexSignInView::new();

    let clipboard = view.apply(started(CodexSignInMethod::Browser, None, false));

    assert_eq!(clipboard, None, "the browser flow has nothing to paste");
    let pending = view.pending().expect("pending");
    assert_eq!(pending.method, CodexSignInMethod::Browser);
    assert!(pending.user_code.is_none());
    assert!(!pending.copied);
}

#[test]
fn a_device_code_goes_on_the_clipboard_without_a_button_press() {
    let mut view = CodexSignInView::new();

    let clipboard = view.apply(started(
        CodexSignInMethod::DeviceCode,
        Some("BXFD-KM2Q"),
        false,
    ));

    assert_eq!(
        clipboard.as_deref(),
        Some("BXFD-KM2Q"),
        "the code is handed to the clipboard on arrival"
    );
    let pending = view.pending().expect("pending");
    assert_eq!(pending.user_code.as_deref(), Some("BXFD-KM2Q"));
    assert!(pending.copied, "the sheet says the code is already copied");
}

#[test]
fn an_automatic_fall_through_is_flagged_so_the_sheet_can_explain_it() {
    let mut view = CodexSignInView::new();

    view.apply(started(
        CodexSignInMethod::DeviceCode,
        Some("BXFD-KM2Q"),
        true,
    ));

    assert!(view.pending().expect("pending").fell_back);
}

#[test]
fn a_deliberate_device_code_is_not_flagged_as_a_fall_through() {
    let mut view = CodexSignInView::new();

    view.apply(started(
        CodexSignInMethod::DeviceCode,
        Some("BXFD-KM2Q"),
        false,
    ));

    assert!(!view.pending().expect("pending").fell_back);
}

#[test]
fn progress_updates_the_countdown() {
    let mut view = CodexSignInView::new();
    view.apply(started(CodexSignInMethod::Browser, None, false));

    view.apply(ViewCommand::CodexSignInProgress {
        remaining_secs: 107,
    });

    let pending = view.pending().expect("pending");
    assert_eq!(pending.remaining_secs, 107);
    assert_eq!(pending.countdown(), "1:47");
    assert!(!pending.expired());
}

#[test]
fn the_countdown_pads_seconds() {
    let mut view = CodexSignInView::new();
    view.apply(started(CodexSignInMethod::Browser, None, false));

    view.apply(ViewCommand::CodexSignInProgress { remaining_secs: 65 });

    assert_eq!(view.pending().expect("pending").countdown(), "1:05");
}

#[test]
fn reaching_zero_marks_the_attempt_expired() {
    let mut view = CodexSignInView::new();
    view.apply(started(CodexSignInMethod::DeviceCode, Some("X"), false));

    view.apply(ViewCommand::CodexSignInProgress { remaining_secs: 0 });

    let pending = view.pending().expect("pending");
    assert!(pending.expired());
    assert_eq!(pending.countdown(), "0:00");
}

#[test]
fn progress_before_anything_started_is_ignored() {
    let mut view = CodexSignInView::new();

    view.apply(ViewCommand::CodexSignInProgress { remaining_secs: 5 });

    assert_eq!(view.phase(), &SignInPhase::Idle);
}

#[test]
fn completion_shows_the_account_and_plan() {
    let mut view = CodexSignInView::new();
    view.apply(started(CodexSignInMethod::Browser, None, false));

    view.apply(ViewCommand::CodexSignInCompleted {
        account: "chatgpt-acct-1".to_string(),
        label: "andrew@example.com".to_string(),
        plan: Some("ChatGPT Pro".to_string()),
    });

    assert_eq!(
        view.phase(),
        &SignInPhase::Succeeded {
            label: "andrew@example.com".to_string(),
            plan: Some("ChatGPT Pro".to_string()),
        }
    );
}

#[test]
fn use_a_device_code_asks_for_one() {
    let (bridge, events) = make_bridge();
    let view = CodexSignInView::with_bridge(bridge);

    view.use_device_code();

    assert_eq!(
        events.try_recv().expect("event emitted"),
        UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::DeviceCode
        }
    );
}

#[test]
fn cancelling_tells_the_presenter_and_clears_the_sheet() {
    let (bridge, events) = make_bridge();
    let mut view = CodexSignInView::with_bridge(bridge);
    view.apply(started(CodexSignInMethod::Browser, None, false));

    view.cancel();

    assert_eq!(
        events.try_recv().expect("event emitted"),
        UserEvent::CancelCodexSignIn
    );
    assert_eq!(view.phase(), &SignInPhase::Idle);
}

#[test]
fn a_timeout_offers_a_retry_and_a_device_code() {
    assert_eq!(
        CodexSignInView::failure_message(CodexSignInFailure::TimedOut),
        "Sign-in timed out."
    );
    assert_eq!(
        CodexSignInView::failure_action(CodexSignInFailure::TimedOut),
        Some("Try again")
    );
    assert!(CodexSignInFailure::TimedOut.suggests_device_code());
}

#[test]
fn an_expired_code_offers_a_new_one() {
    assert_eq!(
        CodexSignInView::failure_message(CodexSignInFailure::DeviceCodeExpired),
        "That code expired."
    );
    assert_eq!(
        CodexSignInView::failure_action(CodexSignInFailure::DeviceCodeExpired),
        Some("Get a new code")
    );
}

#[test]
fn an_issuer_without_device_login_sends_the_user_back_to_the_browser() {
    assert_eq!(
        CodexSignInView::failure_message(CodexSignInFailure::DeviceCodeUnsupported),
        "This server does not offer device-code sign-in."
    );
    assert_eq!(
        CodexSignInView::failure_action(CodexSignInFailure::DeviceCodeUnsupported),
        Some("Use my browser")
    );
    assert!(!CodexSignInFailure::DeviceCodeUnsupported.suggests_device_code());
}

#[test]
fn a_cancellation_offers_nothing_to_press() {
    assert_eq!(
        CodexSignInView::failure_action(CodexSignInFailure::Cancelled),
        None
    );
    assert!(!CodexSignInFailure::Cancelled.is_retryable());
}

#[test]
fn a_storage_failure_offers_nothing_to_press() {
    assert_eq!(
        CodexSignInView::failure_action(CodexSignInFailure::Storage),
        None
    );
}

#[test]
fn retrying_an_expired_code_asks_for_another_code_not_a_browser() {
    let (bridge, events) = make_bridge();
    let mut view = CodexSignInView::with_bridge(bridge);
    view.apply(ViewCommand::CodexSignInFailed {
        reason: CodexSignInFailure::DeviceCodeExpired,
        message: "device code expired".to_string(),
    });

    view.retry();

    assert_eq!(
        events.try_recv().expect("event emitted"),
        UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::DeviceCode
        }
    );
}

#[test]
fn retrying_a_timeout_asks_for_the_browser() {
    let (bridge, events) = make_bridge();
    let mut view = CodexSignInView::with_bridge(bridge);
    view.apply(ViewCommand::CodexSignInFailed {
        reason: CodexSignInFailure::TimedOut,
        message: "sign-in timed out".to_string(),
    });

    view.retry();

    assert_eq!(
        events.try_recv().expect("event emitted"),
        UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::Browser
        }
    );
}

#[test]
fn an_unrelated_command_leaves_the_sheet_alone() {
    let mut view = CodexSignInView::new();
    view.apply(started(CodexSignInMethod::Browser, None, false));

    let clipboard = view.apply(ViewCommand::ShowNotification {
        message: "unrelated".to_string(),
    });

    assert_eq!(clipboard, None);
    assert!(view.pending().is_some());
}
