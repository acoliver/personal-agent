//! Dropdown internals of the chat view: conversation and profile rows, the
//! sidebar-aware offset, and the local-engine status dot with its palette.

use super::ChatView;
use crate::llm::local::engine::EngineStatus;
use crate::presentation::view_command::{AppMode, ConversationSummary, ProfileSummary};
use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::views::main_panel::MainPanelAppState;
use gpui::{div, prelude::*, px, MouseButton, SharedString};

impl ChatView {
    /// Single row inside the conversation dropdown.
    pub(super) fn render_conversation_item(
        index: usize,
        conversation: &ConversationSummary,
        active_id: Option<uuid::Uuid>,
        highlighted_index: usize,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let conversation_id = conversation.id;
        let selected = active_id == Some(conversation_id);
        let highlighted = highlighted_index == index;
        let title = if conversation.title.trim().is_empty() {
            "Untitled Conversation".to_string()
        } else {
            conversation.title.clone()
        };
        let count_label = if conversation.message_count == 1 {
            "1 message".to_string()
        } else {
            format!("{} messages", conversation.message_count)
        };

        div()
            .id(SharedString::from(format!(
                "chat-conversation-item-{conversation_id}"
            )))
            .w_full()
            .px(px(Theme::spacing_sm_scaled()))
            .py(px(Theme::spacing_md_scaled() * 0.5))
            .cursor_pointer()
            .when(selected, |row| {
                row.bg(Theme::accent()).text_color(Theme::selection_fg())
            })
            .when(!selected && highlighted, |row| {
                row.bg(Theme::accent_hover())
                    .text_color(Theme::selection_fg())
            })
            .when(!selected && !highlighted, |row| {
                row.hover(|s| s.bg(Theme::bg_darker()))
                    .text_color(Theme::text_primary())
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(Theme::font_size_ui())).child(title))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_secondary())
                            .child(count_label),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.select_conversation_at_index(index, cx);
                    cx.stop_propagation();
                }),
            )
    }

    /// Single row inside the profile dropdown.
    pub(super) fn render_profile_item(
        index: usize,
        profile: &ProfileSummary,
        selected_id: Option<uuid::Uuid>,
        highlighted_index: usize,
        engine_status: Option<&EngineStatus>,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let is_selected = selected_id == Some(profile.id);
        let is_highlighted = highlighted_index == index;
        let label = if profile.is_default {
            format!("{} (default)", profile.name)
        } else {
            profile.name.clone()
        };
        let model_id = profile.model_id.clone();

        div()
            .id(SharedString::from(format!(
                "chat-profile-item-{}",
                profile.id
            )))
            .w_full()
            .px(px(Theme::spacing_sm_scaled()))
            .py(px(Theme::spacing_md_scaled() * 0.5))
            .cursor_pointer()
            .when(is_selected, |row| {
                row.bg(Theme::accent()).text_color(Theme::selection_fg())
            })
            .when(!is_selected && is_highlighted, |row| {
                row.bg(Theme::accent_hover())
                    .text_color(Theme::selection_fg())
            })
            .when(!is_selected && !is_highlighted, |row| {
                row.hover(|s| s.bg(Theme::bg_darker()))
                    .text_color(Theme::text_primary())
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .min_w(px(0.0))
                            .flex_1()
                            .items_center()
                            .gap(px(6.0))
                            // `children` skips the gap when the dot is absent,
                            // so non-local rows keep their old layout.
                            .children(engine_status.map(engine_status_dot))
                            .child(
                                div()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis_start()
                                    .text_size(px(Theme::font_size_ui()))
                                    .child(label),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_secondary())
                            .child(model_id),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.select_profile_at_index(index, cx);
                }),
            )
    }

    /// Extra left offset when the sidebar toggle button is present in popout mode.
    pub(super) fn sidebar_toggle_offset(cx: &gpui::Context<Self>) -> f32 {
        let is_popout = cx
            .try_global::<MainPanelAppState>()
            .is_some_and(|s| s.app_mode == AppMode::Popout);
        if is_popout {
            36.0
        } else {
            0.0
        }
    }
}

/// The small engine-status dot on `provider == "local"` dropdown rows
/// (mockup section 2): gray not-resident, amber loading, green resident,
/// red load failure.
fn engine_status_dot(status: &EngineStatus) -> gpui::AnyElement {
    div()
        .size(px(8.0))
        .rounded(px(4.0))
        .flex_shrink_0()
        .bg(engine_status_dot_color(status))
        .into_any_element()
}

/// Dot palette per engine state; mirrors the settings status card's colors.
fn engine_status_dot_color(status: &EngineStatus) -> gpui::Hsla {
    match status {
        EngineStatus::NotLoaded => Theme::text_muted(),
        EngineStatus::Loading => Theme::warning(),
        EngineStatus::Loaded { .. } => Theme::accent(),
        EngineStatus::Error { .. } => Theme::error(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the dot's state→token pairing (gray not-resident, amber loading,
    /// green resident, red failure). Headless tests resolve several tokens to
    /// the same fallback color, so token *identity* is asserted, not visual
    /// distinctness — the mac-native palette keeps them visibly apart.
    #[test]
    fn engine_states_map_to_the_intended_theme_tokens() {
        let cases = [
            (EngineStatus::NotLoaded, Theme::text_muted()),
            (EngineStatus::Loading, Theme::warning()),
            (
                EngineStatus::Loaded {
                    layers: 41,
                    total_layers: 41,
                    n_ctx: 8192,
                    last_tok_s: 0.0,
                },
                Theme::accent(),
            ),
            (
                EngineStatus::Error {
                    message: "boom".to_string(),
                },
                Theme::error(),
            ),
        ];
        for (status, expected) in cases {
            let got = engine_status_dot_color(&status);
            // Hsla has no PartialEq and exact float equality trips
            // clippy::float_cmp, so compare components with a tiny tolerance.
            let same = |a: f32, b: f32| (a - b).abs() < f32::EPSILON * 4.0;
            assert!(
                same(got.h, expected.h)
                    && same(got.s, expected.s)
                    && same(got.l, expected.l)
                    && same(got.a, expected.a),
                "{status:?} must use its intended dot token"
            );
        }
    }
}
