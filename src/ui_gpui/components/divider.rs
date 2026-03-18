//! Horizontal line separator component
//!
//! @plan PLAN-20250130-GPUIREDUX.P02
//! @requirement REQ-GPUI-003

use gpui::{div, px, IntoElement, Styled};

pub struct Divider {
    color: Option<gpui::Hsla>,
}

impl Divider {
    #[must_use]
    pub const fn new() -> Self {
        Self { color: None }
    }

    #[must_use]
    pub const fn color(mut self, color: gpui::Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for Divider {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;

        let color = self.color.unwrap_or_else(Theme::border);

        div().w_full().h(px(1.0)).bg(color)
    }
}
