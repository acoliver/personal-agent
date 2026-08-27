//! Render implementation for [`CodexSignInView`].

use gpui::{div, prelude::*, px, ClipboardItem, FontWeight, MouseButton, SharedString};

use super::{CodexSignInView, PendingSignIn, SignInPhase};
use crate::events::types::CodexSignInMethod;
use crate::presentation::view_command::{CodexSignInFailure, ViewId};
use crate::ui_gpui::theme::Theme;

impl CodexSignInView {
    fn render_top_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("codex-signin-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            .text_color(Theme::text_primary())
            .child(
                div()
                    .text_size(px(Theme::font_size_h3()))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Theme::text_primary())
                    .child("Sign in with ChatGPT"),
            )
            .child(
                div()
                    .id("btn-codex-close")
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_secondary())
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.cancel();
                            crate::ui_gpui::navigation_channel()
                                .request_navigate(ViewId::ProfileEditor);
                            cx.notify();
                        }),
                    )
                    .child("Cancel"),
            )
    }

    /// A line of body copy.
    fn line(text: impl Into<SharedString>, color: gpui::Hsla) -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(color)
            .child(text.into())
    }

    /// A clickable link that also offers a copy button.
    fn render_link(url: String, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let to_open = url.clone();
        let to_copy = url.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .id("codex-signin-url")
                    .cursor_pointer()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::accent())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |_, _, _window, cx| {
                            cx.open_url(&to_open);
                        }),
                    )
                    .child(url),
            )
            .child(Self::render_secondary_button(
                "btn-codex-copy-link",
                "Copy link",
                move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(to_copy.clone()));
                },
                cx,
            ))
    }

    /// A low-emphasis button.
    fn render_secondary_button(
        id: &'static str,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Self, &mut gpui::Context<Self>) + 'static,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .h(px(24.0))
            .px(px(10.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(4.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    action(this, cx);
                    cx.notify();
                }),
            )
            .child(label.into())
    }

    /// The high-emphasis button.
    fn render_primary_button(
        id: &'static str,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Self, &mut gpui::Context<Self>) + 'static,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .h(px(26.0))
            .px(px(12.0))
            .bg(Theme::accent())
            .rounded(px(4.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::accent_fg())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    action(this, cx);
                    cx.notify();
                }),
            )
            .child(label.into())
    }

    /// The body while a browser sign-in is waiting on the user.
    fn render_browser_body(
        pending: &PendingSignIn,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(Self::line(
                "Your browser is opening. If it did not, click here:",
                Theme::text_primary(),
            ))
            .child(Self::render_link(pending.url.clone(), cx))
            .child(Self::render_waiting_line(
                "Waiting for you to finish signing in…",
                pending,
            ))
            .child(Self::line(
                "Listening on localhost:1455.",
                Theme::text_muted(),
            ))
            .child(Self::render_secondary_button(
                "btn-codex-use-device-code",
                "Use a device code",
                |this, _| this.use_device_code(),
                cx,
            ))
            .into_any_element()
    }

    /// The body while a device code is waiting for approval.
    fn render_device_code_body(
        pending: &PendingSignIn,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let code = pending.user_code.clone().unwrap_or_default();
        let to_copy = code.clone();
        let copied = pending.copied;

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .when(pending.fell_back, |body| {
                body.child(Self::line(
                    "Port 1455 is in use, so we switched to a device code.",
                    Theme::text_muted(),
                ))
            })
            .child(Self::line(
                "1.  Open this page on any device",
                Theme::text_primary(),
            ))
            .child(Self::render_link(pending.url.clone(), cx))
            .child(Self::line(
                if copied {
                    "2.  Paste the code. It is already on your clipboard."
                } else {
                    "2.  Enter this code"
                },
                Theme::text_primary(),
            ))
            .child(
                div()
                    .id("codex-user-code")
                    .px(px(12.0))
                    .py(px(6.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .text_size(px(Theme::font_size_h2()))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Theme::text_primary())
                    .child(code),
            )
            .child(Self::render_secondary_button(
                "btn-codex-copy-code",
                "Copy code",
                move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(to_copy.clone()));
                },
                cx,
            ))
            .child(Self::render_waiting_line("Waiting for approval…", pending))
            .child(Self::line(
                "Only continue if you started this sign-in.",
                Theme::text_muted(),
            ))
            .into_any_element()
    }

    /// The spinner line plus countdown.
    fn render_waiting_line(label: &'static str, pending: &PendingSignIn) -> impl IntoElement {
        let (text, color) = if pending.expired() {
            ("Expired.", Theme::error())
        } else {
            (label, Theme::text_secondary())
        };

        div()
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .text_size(px(Theme::font_size_ui()))
            .text_color(color)
            .child(text)
            .child(
                div()
                    .text_color(Theme::text_muted())
                    .child(pending.countdown()),
            )
    }

    /// The body once a grant has been stored.
    fn render_success_body(
        label: &str,
        plan: Option<&str>,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(Self::line(
                format!("Signed in as {label}"),
                Theme::success(),
            ))
            .when_some(plan.map(str::to_owned), |body, plan| {
                body.child(Self::line(plan, Theme::text_muted()))
            })
            .child(Self::render_primary_button(
                "btn-codex-done",
                "Done",
                |_, _| {
                    crate::ui_gpui::navigation_channel().request_navigate(ViewId::ProfileEditor);
                },
                cx,
            ))
            .into_any_element()
    }

    /// The body after a failure, with the way forward that fits it.
    fn render_failure_body(
        reason: CodexSignInFailure,
        message: &str,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let detail = message.to_owned();

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(Self::line(Self::failure_message(reason), Theme::error()))
            .child(Self::line(detail, Theme::text_muted()))
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .when_some(Self::failure_action(reason), |row, label| {
                        row.child(Self::render_primary_button(
                            "btn-codex-retry",
                            label,
                            |this, _| this.retry(),
                            cx,
                        ))
                    })
                    .when(reason.suggests_device_code(), |row| {
                        row.child(Self::render_secondary_button(
                            "btn-codex-failure-device-code",
                            "Use a device code",
                            |this, _| this.use_device_code(),
                            cx,
                        ))
                    }),
            )
            .into_any_element()
    }

    fn render_body(&self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        match &self.state {
            SignInPhase::Idle => div()
                .text_size(px(Theme::font_size_ui()))
                .text_color(Theme::text_muted())
                .child("Starting sign-in…")
                .into_any_element(),
            SignInPhase::Pending(pending) => match pending.method {
                CodexSignInMethod::Browser => Self::render_browser_body(pending, cx),
                CodexSignInMethod::DeviceCode => Self::render_device_code_body(pending, cx),
            },
            SignInPhase::Succeeded { label, plan } => {
                Self::render_success_body(label, plan.as_deref(), cx)
            }
            SignInPhase::Failed { reason, message } => {
                Self::render_failure_body(*reason, message, cx)
            }
        }
    }
}

impl gpui::Render for CodexSignInView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_base())
            .text_color(Theme::text_primary())
            .child(Self::render_top_bar(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(px(16.0))
                    .gap(px(12.0))
                    .text_color(Theme::text_primary())
                    .child(self.render_body(cx)),
            )
    }
}
