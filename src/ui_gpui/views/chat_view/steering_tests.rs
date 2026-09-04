//! Mid-turn steering from the chat composer (issue #222).
//!
//! Two controls share the composer row while a turn runs: Stop ends it, Send
//! joins it. These drive the real rendered buttons and the real key handler,
//! so a control that disappears, a click that reaches the wrong branch, or a
//! submit that restarts the turn all fail here.
//!
//! @plan PLAN-20260903-ISSUE222.P04

#![allow(clippy::future_not_send)]
#![allow(deprecated)]

use super::state::{ChatState, StreamingState};
use super::ChatView;
use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::bridge::GpuiBridge;
use gpui::{
    px, size, Entity, KeyDownEvent, Keystroke, Modifiers, Pixels, Point, Render, TestAppContext,
    VisualTestContext,
};
use std::sync::Arc;
use uuid::Uuid;

fn make_chat_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

fn enter_event() -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse("enter").expect("enter keystroke"),
        is_held: false,
        prefer_character_input: false,
    }
}

/// A conversation whose turn is running, with `input` already typed.
fn mid_turn_state(conversation_id: Uuid, input: &str) -> ChatState {
    ChatState {
        active_conversation_id: Some(conversation_id),
        streaming: StreamingState::Streaming {
            content: "partial answer".to_string(),
            done: false,
        },
        input_text: input.to_string(),
        cursor_position: input.len(),
        ..ChatState::default()
    }
}

/// A conversation with nothing running, with `input` already typed.
fn idle_state(conversation_id: Uuid, input: &str) -> ChatState {
    ChatState {
        active_conversation_id: Some(conversation_id),
        input_text: input.to_string(),
        cursor_position: input.len(),
        ..ChatState::default()
    }
}

/// Opens a window whose root is the chat view, so `debug_bounds` reports the
/// composer controls the user would actually click.
fn mount_chat_view(
    cx: &mut TestAppContext,
    state: ChatState,
) -> (
    Entity<ChatView>,
    flume::Receiver<UserEvent>,
    &mut VisualTestContext,
) {
    let (bridge, user_rx) = make_chat_bridge();
    let (view, window_cx) = cx.add_window_view(move |_window, cx| {
        let mut view = ChatView::new(state, cx);
        view.set_bridge(bridge);
        view
    });
    window_cx.simulate_resize(size(px(780.0), px(600.0)));
    window_cx.run_until_parked();
    (view, user_rx, window_cx)
}

/// Redraws the view so `debug_bounds` reflects the current state.
fn redraw(view: &Entity<ChatView>, window_cx: &mut VisualTestContext) {
    window_cx.update(|window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            cx.notify();
            let _ = view.render(window, cx);
        });
    });
    window_cx.run_until_parked();
}

fn with_view<R>(
    view: &Entity<ChatView>,
    window_cx: &mut VisualTestContext,
    read: impl FnOnce(&ChatView) -> R,
) -> R {
    window_cx.update(|_window, app| read(view.read(app)))
}

fn press_enter(view: &Entity<ChatView>, window_cx: &mut VisualTestContext) {
    window_cx.update(|window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_key_down(&enter_event(), window, cx);
        });
    });
    window_cx.run_until_parked();
}

fn click(window_cx: &mut VisualTestContext, selector: &'static str) {
    let center: Point<Pixels> = window_cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} must be rendered"))
        .center();
    window_cx.simulate_click(center, Modifiers::default());
    window_cx.run_until_parked();
}

fn emitted(user_rx: &flume::Receiver<UserEvent>) -> Vec<UserEvent> {
    let mut events = Vec::new();
    while let Ok(event) = user_rx.try_recv() {
        events.push(event);
    }
    events
}

// ── REQ-222-001: Stop is a control of its own ────────────────────────────

/// A running turn offers both controls at once. Before this, Send was
/// replaced by Stop, so the only way to redirect the agent was to kill the
/// turn and lose what it had produced.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-001
#[gpui::test]
fn a_running_turn_shows_stop_beside_send(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, _user_rx, window_cx) = mount_chat_view(cx, mid_turn_state(conversation_id, "wait"));
    redraw(&view, window_cx);

    assert!(
        window_cx.debug_bounds("chat-stop-button").is_some(),
        "a running turn must offer a Stop control"
    );
    assert!(
        window_cx.debug_bounds("chat-send-button").is_some(),
        "a running turn must keep the Send control, or steering has no button"
    );
}

/// With nothing running there is nothing to stop, so Stop is absent and only
/// Send remains — the composer the user has always seen.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-001
#[gpui::test]
fn an_idle_conversation_shows_only_send(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, _user_rx, window_cx) = mount_chat_view(cx, idle_state(conversation_id, "hello"));
    redraw(&view, window_cx);

    assert!(
        window_cx.debug_bounds("chat-stop-button").is_none(),
        "there is no turn to stop, so Stop must not be rendered"
    );
    assert!(
        window_cx.debug_bounds("chat-send-button").is_some(),
        "an idle composer must still offer Send"
    );
}

/// Stop keeps doing exactly what it did: end the turn, leave the composer
/// text alone, and hand the keyboard back.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-001
#[gpui::test]
fn clicking_stop_still_ends_the_turn(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, user_rx, window_cx) = mount_chat_view(cx, mid_turn_state(conversation_id, "wait"));
    redraw(&view, window_cx);

    click(window_cx, "chat-stop-button");

    assert_eq!(
        emitted(&user_rx),
        vec![UserEvent::StopStreaming { conversation_id }],
        "Stop must still cancel the turn it names"
    );
    with_view(&view, window_cx, |view| {
        assert_eq!(
            view.state.streaming,
            StreamingState::Idle,
            "Stop resets the view's streaming state"
        );
        assert_eq!(
            view.state.input_text, "wait",
            "Stop must not consume the composer text"
        );
    });
}

// ── REQ-222-002: Send steers a running turn ──────────────────────────────

/// Clicking Send mid-turn joins the turn instead of starting a new one, and
/// the composer empties the way it does for any accepted submit.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-002
#[gpui::test]
fn clicking_send_mid_turn_steers_the_running_turn(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, user_rx, window_cx) =
        mount_chat_view(cx, mid_turn_state(conversation_id, "use the cached index"));
    redraw(&view, window_cx);

    click(window_cx, "chat-send-button");

    assert_eq!(
        emitted(&user_rx),
        vec![UserEvent::SteerStreaming {
            conversation_id,
            text: "use the cached index".to_string(),
        }],
        "Send mid-turn must steer, and must not start a second turn"
    );
    with_view(&view, window_cx, |view| {
        assert_eq!(
            view.state.input_text, "",
            "an accepted steer clears the composer"
        );
        assert_eq!(view.state.cursor_position, 0);
    });
}

/// Enter is the same submit as the button; routing them separately is how
/// the two drift apart.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-002
#[gpui::test]
fn pressing_enter_mid_turn_steers_the_running_turn(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, user_rx, window_cx) = mount_chat_view(
        cx,
        mid_turn_state(conversation_id, "check the config first"),
    );

    press_enter(&view, window_cx);

    assert_eq!(
        emitted(&user_rx),
        vec![UserEvent::SteerStreaming {
            conversation_id,
            text: "check the config first".to_string(),
        }],
        "Enter mid-turn must steer, and must not start a second turn"
    );
    with_view(&view, window_cx, |view| {
        assert_eq!(view.state.input_text, "");
        assert_eq!(view.state.cursor_position, 0);
    });
}

/// Steering is additive: the turn it joins keeps running, so the view's
/// streaming state must survive the submit untouched.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-002
/// @requirement REQ-222-006
#[gpui::test]
fn steering_leaves_the_turn_running(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, user_rx, window_cx) = mount_chat_view(cx, mid_turn_state(conversation_id, "and"));

    let before = with_view(&view, window_cx, |view| view.state.streaming.clone());
    press_enter(&view, window_cx);

    with_view(&view, window_cx, |view| {
        assert_eq!(
            view.state.streaming, before,
            "a steer must not restart, finish or clear the turn it joins"
        );
    });
    let events = emitted(&user_rx);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, UserEvent::SteerStreaming { .. })),
        "the submit under test must have steered, got {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, UserEvent::StopStreaming { .. })),
        "a steer must never cancel the turn on the user's behalf"
    );
}

/// Whitespace is not an instruction. It was a no-op mid-turn before and
/// stays one, through both controls.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-002
#[gpui::test]
fn blank_input_submits_nothing_mid_turn(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, user_rx, window_cx) =
        mount_chat_view(cx, mid_turn_state(conversation_id, "   \n  "));
    redraw(&view, window_cx);

    click(window_cx, "chat-send-button");
    press_enter(&view, window_cx);

    assert_eq!(
        emitted(&user_rx),
        Vec::new(),
        "blank input must submit nothing while a turn runs"
    );
    with_view(&view, window_cx, |view| {
        assert_eq!(
            view.state.streaming,
            StreamingState::Streaming {
                content: "partial answer".to_string(),
                done: false,
            },
            "blank input must not disturb the running turn"
        );
        assert_eq!(
            view.state.input_text, "   \n  ",
            "a refused submit leaves the composer as typed"
        );
    });
}

/// The same rule while idle, which is where it already held.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-002
#[gpui::test]
fn blank_input_submits_nothing_while_idle(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, user_rx, window_cx) = mount_chat_view(cx, idle_state(conversation_id, "   \n  "));
    redraw(&view, window_cx);

    click(window_cx, "chat-send-button");
    press_enter(&view, window_cx);

    assert_eq!(
        emitted(&user_rx),
        Vec::new(),
        "blank input must submit nothing while idle"
    );
    with_view(&view, window_cx, |view| {
        assert_eq!(
            view.state.streaming,
            StreamingState::Idle,
            "a refused submit must not start a turn"
        );
    });
}

// ── REQ-222-003: the queued entry is on screen ───────────────────────────

/// A queued steer has to be visible, not merely recorded: the point of the
/// entry is that the user can see the instruction is waiting its turn.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-003
#[gpui::test]
fn a_queued_steer_is_painted_in_the_transcript(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, _user_rx, window_cx) = mount_chat_view(cx, mid_turn_state(conversation_id, ""));
    redraw(&view, window_cx);
    assert!(
        window_cx.debug_bounds("chat-queued-steering-0").is_none(),
        "nothing is queued yet, so no entry should be on screen"
    );

    window_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::SteeringQueued {
                    conversation_id,
                    steer_id: Uuid::new_v4(),
                    text: "hold off on the refactor".to_string(),
                },
                cx,
            );
        });
    });
    redraw(&view, window_cx);

    assert!(
        window_cx.debug_bounds("chat-queued-steering-0").is_some(),
        "a waiting steer must be painted in the transcript"
    );
}

// ── Regression guard: the idle submit path is unchanged ──────────────────

/// The idle path is the one users take all day; steering must not disturb it.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-002
#[gpui::test]
fn clicking_send_while_idle_still_sends_a_message(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, user_rx, window_cx) = mount_chat_view(cx, idle_state(conversation_id, "first ask"));
    window_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, _cx| {
            view.conversation_id = Some(conversation_id);
        });
    });
    redraw(&view, window_cx);

    click(window_cx, "chat-send-button");

    assert_eq!(
        emitted(&user_rx),
        vec![UserEvent::SendMessage {
            text: "first ask".to_string(),
            conversation_id: Some(conversation_id),
        }],
        "an idle Send must still start a turn"
    );
    with_view(&view, window_cx, |view| {
        assert_eq!(
            view.state.streaming,
            StreamingState::Streaming {
                content: String::new(),
                done: false,
            },
            "an idle Send still moves the view into streaming"
        );
    });
}

/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-002
#[gpui::test]
fn pressing_enter_while_idle_still_sends_a_message(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (view, user_rx, window_cx) = mount_chat_view(cx, idle_state(conversation_id, "first ask"));
    window_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, _cx| {
            view.conversation_id = Some(conversation_id);
        });
    });

    press_enter(&view, window_cx);

    assert_eq!(
        emitted(&user_rx),
        vec![UserEvent::SendMessage {
            text: "first ask".to_string(),
            conversation_id: Some(conversation_id),
        }],
        "an idle Enter must still start a turn"
    );
}
