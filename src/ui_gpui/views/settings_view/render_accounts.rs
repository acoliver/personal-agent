//! The signed-in `ChatGPT` accounts list, shown under Models.
//!
//! Accounts exist to serve model profiles, so they live next to the profile
//! list rather than in a category of their own.

use gpui::{div, prelude::*, px, MouseButton};

use super::SettingsView;
use crate::events::types::{CodexSignInMethod, UserEvent};
use crate::presentation::view_command::{CodexAccountInfo, ViewId};
use crate::ui_gpui::theme::Theme;

impl SettingsView {
    /// The accounts block: a heading, one row per account, and a way to add
    /// another.
    pub(super) fn render_accounts_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("CHATGPT ACCOUNTS"),
            )
            .children(
                self.state
                    .accounts
                    .iter()
                    .enumerate()
                    .map(|(index, account)| Self::render_account_row(index, account, cx))
                    .collect::<Vec<_>>(),
            )
            .when(self.state.accounts.is_empty(), |block| {
                block.child(
                    div()
                        .text_size(px(Theme::font_size_small()))
                        .text_color(Theme::text_muted())
                        .child("No ChatGPT accounts yet."),
                )
            })
            .child(Self::render_add_account_button(cx))
    }

    /// One account: who it is, what state it is in, and what to do about it.
    fn render_account_row(
        index: usize,
        account: &CodexAccountInfo,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let slug = account.account.clone();
        let (marker, marker_color) = if account.needs_reauth {
            ("!", Theme::error())
        } else {
            ("*", Theme::success())
        };
        let status = Self::account_status_line(account);
        let used_by = Self::account_usage_line(account);

        div()
            .id(("codex-account-row", index))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .p(px(8.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(4.0))
            .text_color(Theme::text_primary())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child(div().text_color(marker_color).child(marker))
                    .child(account.label.clone())
                    .when_some(account.plan.clone(), |row, plan| {
                        row.child(div().text_color(Theme::text_muted()).child(plan))
                    }),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_small()))
                    .text_color(Theme::text_muted())
                    .child(status),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_small()))
                    .text_color(Theme::text_muted())
                    .child(used_by),
            )
            .child(Self::render_account_actions(
                index,
                account.needs_reauth,
                slug,
                cx,
            ))
            .into_any_element()
    }

    /// The buttons on an account row: a way back in when the session died, and
    /// always a way out.
    fn render_account_actions(
        index: usize,
        needs_reauth: bool,
        slug: String,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .gap(px(8.0))
            .when(needs_reauth, |row| {
                row.child(
                    div()
                        .id(("btn-codex-reauth", index))
                        .px(px(10.0))
                        .py(px(4.0))
                        .bg(Theme::accent())
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .text_size(px(Theme::font_size_ui()))
                        .text_color(Theme::accent_fg())
                        .child("Sign in again")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _window, _cx| {
                                this.start_codex_sign_in();
                            }),
                        ),
                )
            })
            .child(
                div()
                    .id(("btn-codex-account-signout", index))
                    .px(px(10.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(Theme::border())
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_darker()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child(if needs_reauth { "Remove" } else { "Sign out" })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, _cx| {
                            this.emit(&UserEvent::SignOutCodexAccount {
                                account: slug.clone(),
                            });
                        }),
                    ),
            )
    }

    /// The button that starts a sign-in for a new account.
    fn render_add_account_button(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("btn-codex-add-account")
            .px(px(12.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(Theme::border())
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_dark()))
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child("+ Add ChatGPT account")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, _cx| {
                    this.start_codex_sign_in();
                }),
            )
    }

    /// Ask for a sign-in and show the sheet that reports its progress.
    ///
    /// The presenter only publishes sign-in state; nothing in that path
    /// changes the current view. Without this the user stays on Settings while
    /// a sign-in runs unseen, and a browser opens with no explanation.
    pub fn start_codex_sign_in(&self) {
        self.emit(&UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::Browser,
        });
        crate::ui_gpui::navigation_channel().request_navigate(ViewId::CodexSignIn);
    }

    /// The second line of an account row.
    #[must_use]
    pub fn account_status_line(account: &CodexAccountInfo) -> String {
        if account.needs_reauth {
            return "Session expired".to_string();
        }
        match account.expires_in_secs {
            Some(secs) if secs <= 0 => "Renewing".to_string(),
            Some(secs) => Self::signed_in_for(secs),
            None => "Signed in".to_string(),
        }
    }

    /// Describe how long a grant has left, in a unit a person reads.
    ///
    /// Access tokens usually last about an hour, so minutes is the useful
    /// unit. A longer-lived grant is not worth counting down, and rendering it
    /// in minutes produces numbers in the tens of millions.
    fn signed_in_for(secs: i64) -> String {
        const MINUTE: i64 = 60;
        const HOUR: i64 = 60 * MINUTE;
        const DAY: i64 = 24 * HOUR;

        match secs {
            s if s < MINUTE => "Signed in, expires shortly".to_string(),
            s if s < 2 * HOUR => Self::left(s / MINUTE, "minute"),
            s if s < DAY => Self::left(s / HOUR, "hour"),
            _ => "Signed in".to_string(),
        }
    }

    /// "Signed in, 1 minute left" / "Signed in, 5 minutes left".
    fn left(count: i64, unit: &str) -> String {
        let plural = if count == 1 { "" } else { "s" };
        format!("Signed in, {count} {unit}{plural} left")
    }

    /// The third line of an account row.
    #[must_use]
    pub fn account_usage_line(account: &CodexAccountInfo) -> String {
        match account.used_by.len() {
            0 => "Not used by any profile".to_string(),
            1 => "Used by 1 profile".to_string(),
            n => format!("Used by {n} profiles"),
        }
    }
}
