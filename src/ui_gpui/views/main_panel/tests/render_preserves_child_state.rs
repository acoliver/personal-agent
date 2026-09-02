//! Render-level regression coverage for issue #212: a render pass must not
//! destroy child-view state.
//!
//! `impl gpui::Render for MainPanel` runs on every frame and refocuses the
//! active child view's composer through `focus_current_view`. A shipped bug
//! (introduced in 407af1b, fixed in #211 / 23d0c8e) made that per-frame
//! refocus clear the chat view's conversation dropdown, profile dropdown,
//! and inline conversation rename, so each was destroyed within a single
//! frame of being opened: both dropdowns were impossible to open and an
//! inline rename was cancelled instantly. Every test still passed because
//! nothing drove `MainPanel::render` itself — the `ChatView` snapshot tests
//! hand-call `focus_composer`, which pins that method's contract but not
//! the render-level property that matters.
//!
//! These tests mount a real `MainPanel` as the root view of a gpui test
//! window and drive actual frames through the real `Render` implementation
//! (`add_window_view` opens the window and draws its first frame; later
//! frames are drawn with `window.draw`), then assert the transient chrome
//! survives: set before a frame, toggled between frames, and across more
//! than one subsequent frame.

use crate::presentation::view_command::ViewId;
use crate::ui_gpui::views::chat_view::ChatView;
use crate::ui_gpui::views::main_panel::{tests::build_app_state, MainPanel};
use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};

/// The chat view's transient chrome that the shipped bug destroyed.
#[derive(Clone, Copy)]
struct Chrome {
    conversation_dropdown_open: bool,
    profile_dropdown_open: bool,
    conversation_title_editing: bool,
}

const ALL_CHROME_OPEN: Chrome = Chrome {
    conversation_dropdown_open: true,
    profile_dropdown_open: true,
    conversation_title_editing: true,
};

const CONVERSATION_DROPDOWN_ONLY: Chrome = Chrome {
    conversation_dropdown_open: true,
    profile_dropdown_open: false,
    conversation_title_editing: false,
};

const PROFILE_DROPDOWN_ONLY: Chrome = Chrome {
    conversation_dropdown_open: false,
    profile_dropdown_open: true,
    conversation_title_editing: false,
};

const TITLE_RENAME_ONLY: Chrome = Chrome {
    conversation_dropdown_open: false,
    profile_dropdown_open: false,
    conversation_title_editing: true,
};

/// Mount a real `MainPanel` as the root view of a test window with the
/// `MainPanelAppState` global installed. Opening the window draws one full
/// frame through `impl Render for MainPanel` before returning, which lazily
/// initializes the child views. `prepare` runs inside the root-view
/// constructor, before that first frame is drawn.
fn mount_panel(
    cx: &mut TestAppContext,
    prepare: impl FnOnce(&mut MainPanel, &mut gpui::Context<MainPanel>),
) -> (Entity<MainPanel>, &mut VisualTestContext) {
    let (app_state, _user_rx, _first_id, _second_id, _profile_id) = build_app_state();
    cx.set_global(app_state);

    let (panel, window_cx) = cx.add_window_view(move |_window, cx| {
        let mut panel = MainPanel::new(cx);
        prepare(&mut panel, cx);
        panel
    });
    (panel, window_cx)
}

/// Drive one real frame through the window's root `MainPanel` render.
fn draw_frame(window_cx: &mut VisualTestContext) {
    window_cx.update(|window, cx| {
        _ = window.draw(cx);
    });
}

/// The chat view child that `ViewId::Chat` frames route to.
fn chat_view_of(panel: &Entity<MainPanel>, cx: &impl AppContext) -> Entity<ChatView> {
    panel.read_with(cx, |panel, _| {
        panel
            .chat_view
            .clone()
            .expect("a live chat view must exist once a frame has been rendered")
    })
}

/// Open all three pieces of transient chrome at once, mirroring the state
/// the shipped bug destroyed on every frame.
fn open_all_chrome(chat_view: &Entity<ChatView>, cx: &mut impl AppContext) {
    chat_view.update(cx, |view, cx| {
        view.state.conversation_dropdown_open = true;
        view.state.profile_dropdown_open = true;
        view.state.conversation_title_editing = true;
        cx.notify();
    });
}

/// Assert the full transient-chrome triple after a frame. Every message
/// names the render-level invariant that broke in issue #212, so a failure
/// reads as the user-visible bug it was.
fn assert_chrome_preserved(
    chat_view: &Entity<ChatView>,
    cx: &impl AppContext,
    expected: Chrome,
    stage: &str,
) {
    chat_view.read_with(cx, |view, _| {
        assert_eq!(
            view.state.conversation_dropdown_open,
            expected.conversation_dropdown_open,
            "issue #212 regression at {stage}: a render pass must not close or open the conversation dropdown; the user left it open={}",
            expected.conversation_dropdown_open
        );
        assert_eq!(
            view.state.profile_dropdown_open,
            expected.profile_dropdown_open,
            "issue #212 regression at {stage}: a render pass must not close or open the profile dropdown; the user left it open={}",
            expected.profile_dropdown_open
        );
        assert_eq!(
            view.state.conversation_title_editing,
            expected.conversation_title_editing,
            "issue #212 regression at {stage}: a render pass must not cancel or start an inline rename; the user left it active={}",
            expected.conversation_title_editing
        );
    });
}

/// Transient chrome that exists before a frame is drawn must survive that
/// frame. The chrome is opened inside the root-view constructor, so the very
/// first frame the window draws meets it in place.
#[gpui::test]
async fn chrome_set_before_a_frame_survives_that_frame(cx: &mut TestAppContext) {
    let (panel, window_cx) = mount_panel(cx, |panel, cx| {
        panel.init(cx);
        let chat_view = panel
            .chat_view
            .as_ref()
            .expect("init must create the chat view")
            .clone();
        open_all_chrome(&chat_view, cx);
    });

    // `open_window` draws exactly one frame before returning; that frame is
    // the one under test. Also prove the frame really ran the per-frame
    // focus path, so this test cannot pass vacuously.
    window_cx.update(|_window, app| {
        let chat_view = chat_view_of(&panel, app);
        assert_chrome_preserved(&chat_view, app, ALL_CHROME_OPEN, "the first drawn frame");
        chat_view.read_with(app, |view, _| {
            assert!(
                view.state.composer_focused,
                "the drawn frame must have run the per-frame composer focus; otherwise no frame was really drawn"
            );
        });
    });
}

/// The exact shipped failure mode: frames are already being drawn when the
/// user starts a rename or toggles a dropdown, and the next frame's
/// composer refocus killed it. Each gesture is checked against all three
/// transient fields. The gestures are ordered so each one's expected state
/// follows from its own documented semantics (opening either dropdown also
/// closes the other transient chrome).
#[gpui::test]
async fn chrome_toggled_between_frames_survives_the_next_frame(cx: &mut TestAppContext) {
    // The window-open frame has already been drawn; every gesture below
    // happens between drawn frames.
    let (panel, window_cx) = mount_panel(cx, |_panel, _cx| {});

    let chat_view = window_cx.update(|_window, app| {
        panel.read_with(app, |panel, _| {
            assert_eq!(
                panel.current_view(),
                ViewId::Chat,
                "the test premise requires navigation to be on the chat view"
            );
            assert!(
                panel.chat_view.is_some(),
                "the window-open frame must have drawn and initialized the chat view"
            );
        });
        chat_view_of(&panel, app)
    });

    // The user starts an inline rename between frames.
    window_cx.update(|_window, app| {
        chat_view.update(app, ChatView::start_rename_conversation);
    });
    draw_frame(window_cx);
    window_cx.update(|_window, app| {
        assert_chrome_preserved(
            &chat_view,
            app,
            TITLE_RENAME_ONLY,
            "the frame after the user started an inline rename",
        );
    });

    // Opening the conversation dropdown closes the rename; the frame must
    // not undo that either.
    window_cx.update(|_window, app| {
        chat_view.update(app, ChatView::toggle_conversation_dropdown);
    });
    draw_frame(window_cx);
    window_cx.update(|_window, app| {
        assert_chrome_preserved(
            &chat_view,
            app,
            CONVERSATION_DROPDOWN_ONLY,
            "the frame after the user opened the conversation dropdown",
        );
    });

    // Opening the profile dropdown closes the conversation dropdown.
    window_cx.update(|_window, app| {
        chat_view.update(app, ChatView::toggle_profile_dropdown);
    });
    draw_frame(window_cx);
    window_cx.update(|_window, app| {
        assert_chrome_preserved(
            &chat_view,
            app,
            PROFILE_DROPDOWN_ONLY,
            "the frame after the user opened the profile dropdown",
        );
    });
}

/// The old bug cleared the chrome on EVERY frame, so surviving one frame
/// was not enough: the toggled dropdown must survive multiple consecutive
/// frames.
#[gpui::test]
async fn toggled_chrome_survives_more_than_one_subsequent_frame(cx: &mut TestAppContext) {
    let (panel, window_cx) = mount_panel(cx, |_panel, _cx| {});

    // Non-vacuous: the window-open frame must have initialized the chat
    // view and run the per-frame composer focus before the toggle below.
    let chat_view = window_cx.update(|_window, app| {
        let chat_view = chat_view_of(&panel, app);
        chat_view.read_with(app, |view, _| {
            assert!(
                view.state.composer_focused,
                "the window-open frame must have run the per-frame composer focus; otherwise no frame was really drawn"
            );
        });
        chat_view
    });

    // The user opens the conversation dropdown after frames have drawn.
    window_cx.update(|_window, app| {
        chat_view.update(app, ChatView::toggle_conversation_dropdown);
    });

    for frame in 1..=3 {
        draw_frame(window_cx);
        window_cx.update(|_window, app| {
            assert_chrome_preserved(
                &chat_view,
                app,
                CONVERSATION_DROPDOWN_ONLY,
                &format!("frame {frame} after the user opened the conversation dropdown"),
            );
        });
    }
}
