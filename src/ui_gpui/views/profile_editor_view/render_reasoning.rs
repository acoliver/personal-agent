//! The reasoning effort picker.
//!
//! Kept apart from the thinking budget deliberately. A budget is a token
//! count and an effort is a named level; a model may take either, both, or
//! neither, and which controls appear is answered by the model's declared
//! capabilities rather than by its API type.

use gpui::prelude::*;
use gpui::{div, px, MouseButton};

use super::ProfileEditorView;
use crate::models::ReasoningEffort;
use crate::ui_gpui::theme::Theme;

impl ProfileEditorView {
    /// Render the effort ladder, or nothing when the model takes no effort.
    ///
    /// Only levels the model declares are offered. There is no free-text
    /// entry: an unrecognised level can arrive from a stored profile and is
    /// carried through untouched, but it is not something to invent here.
    pub(super) fn render_reasoning_effort_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let capabilities = self.capabilities();
        let selected = self.state.data.reasoning_effort.clone();
        let hint = if selected.is_none() {
            "How hard the model thinks. Not a token amount. Unset leaves it to the model."
        } else {
            "How hard the model thinks. Not a token amount."
        };
        let offered = capabilities.reasoning_efforts().to_vec();

        div().flex().flex_col().gap(px(8.0)).when(
            capabilities.takes_reasoning_effort(),
            move |section| {
                section
                    .child(Self::render_label("REASONING EFFORT"))
                    .child(div().flex().flex_row().flex_wrap().gap(px(6.0)).children(
                        offered.into_iter().map(|effort| {
                            Self::render_effort_choice(&effort, selected.as_ref(), cx)
                        }),
                    ))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_small()))
                            .text_color(Theme::text_secondary())
                            .child(hint),
                    )
            },
        )
    }

    /// One selectable level.
    fn render_effort_choice(
        effort: &ReasoningEffort,
        selected: Option<&ReasoningEffort>,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let is_selected = selected == Some(effort);
        let choice = effort.clone();
        let element_id = format!("effort-{}", effort.as_str());

        div()
            .id(gpui::SharedString::from(element_id))
            .px(px(10.0))
            .py(px(4.0))
            .border_1()
            .rounded(px(3.0))
            .cursor_pointer()
            .border_color(if is_selected {
                Theme::accent()
            } else {
                Theme::border()
            })
            .when(is_selected, |d| d.bg(Theme::accent()))
            .text_size(px(Theme::font_size_mono()))
            .text_color(if is_selected {
                Theme::selection_fg()
            } else {
                Theme::text_primary()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.state.data.reasoning_effort = Some(choice.clone());
                    cx.notify();
                }),
            )
            .child(effort.display_name())
    }
}
