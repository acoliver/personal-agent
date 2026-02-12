//! Button components
//!
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003

use gpui::{div, px, prelude::*, IntoElement};

pub struct Button {
    label: String,
    active: bool,
    disabled: bool,
    on_click: Option<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            active: false,
            disabled: false,
            on_click: None,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl IntoElement for Button {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;
        
        let mut button = div()
            .flex()
            .items_center()
            .justify_center()
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .rounded(px(Theme::RADIUS_MD))
            .text_sm();

        // Apply background color based on state
        if self.active {
            button = button.bg(Theme::bg_dark());
        } else {
            button = button.bg(Theme::bg_darker());
        }

        // Apply text color based on disabled state
        if self.disabled {
            button = button.text_color(Theme::text_muted());
        } else {
            button = button.text_color(Theme::text_primary());
        }

        button.child(self.label)
    }
}
