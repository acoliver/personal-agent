//! The signed-in account row in the profile editor.
//!
//! Account-authenticated providers have no credential for the user to pick, so
//! this row shows who is signed in and offers the two things that can change
//! it.

use gpui::{div, prelude::*, px, MouseButton};

use super::ProfileEditorView;
use crate::ui_gpui::theme::Theme;

impl ProfileEditorView {
    /// Render the signed-in account row for account-authenticated providers.
    ///
    /// Signed out this offers a single button; signed in it names the account
    /// and offers a way out. Either way the user never types a credential.
    pub(super) fn render_account_section(&self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let signed_in = !self.state.data.oauth_account.trim().is_empty();

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::render_label("ACCOUNT"))
            .child(if signed_in {
                self.render_signed_in_account(cx)
            } else {
                Self::render_signed_out_account(cx)
            })
            .into_any_element()
    }

    /// The account row when a grant is stored.
    fn render_signed_in_account(&self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let label = if self.state.data.oauth_account_label.trim().is_empty() {
            self.state.data.oauth_account.clone()
        } else {
            self.state.data.oauth_account_label.clone()
        };
        let plan = self.state.data.oauth_account_plan.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .child(div().text_color(Theme::success()).child("*"))
                    .child(label)
                    .when(!plan.is_empty(), |row| {
                        row.child(div().text_color(Theme::text_muted()).child(plan))
                    }),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_small()))
                    .text_color(Theme::text_muted())
                    .child("Signed in. The session renews itself."),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(Self::render_account_button(
                        "btn-codex-switch",
                        "Switch",
                        cx,
                    ))
                    .child(Self::render_sign_out_button(
                        self.state.data.oauth_account.clone(),
                        cx,
                    )),
            )
            .into_any_element()
    }

    /// The account row before anyone has signed in.
    fn render_signed_out_account(cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_small()))
                    .text_color(Theme::text_muted())
                    .child("Sign in with your ChatGPT subscription."),
            )
            .child(Self::render_account_button(
                "btn-codex-signin",
                "Sign in with ChatGPT",
                cx,
            ))
            .into_any_element()
    }

    /// A button that starts a sign-in.
    fn render_account_button(
        id: &'static str,
        label: &'static str,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id(id)
            .h(px(24.0))
            .px(px(10.0))
            .bg(Theme::accent())
            .rounded(px(4.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(px(Theme::font_size_mono()))
            .text_color(Theme::accent_fg())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.start_codex_sign_in();
                    cx.notify();
                }),
            )
            .child(label)
            .into_any_element()
    }

    /// A button that forgets the signed-in account.
    fn render_sign_out_button(account: String, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        div()
            .id("btn-codex-signout")
            .h(px(24.0))
            .px(px(10.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(4.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(px(Theme::font_size_mono()))
            .text_color(Theme::text_primary())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.sign_out_codex_account(account.clone());
                    cx.notify();
                }),
            )
            .child("Sign out")
            .into_any_element()
    }
}
