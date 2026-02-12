//! Icon button component for toolbar actions
//!
//! @plan PLAN-20250130-GPUIREDUX.P02
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, px, IntoElement, Styled, Window};

pub struct IconButton {
    icon: String,
    active: bool,
    on_click: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    tooltip: Option<String>,
}

impl IconButton {
    pub fn new(icon: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            active: false,
            on_click: None,
            tooltip: None,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn on_click(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl IntoElement for IconButton {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;
        
        let mut button = div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.0))
            .rounded(px(Theme::RADIUS_SM))
            .cursor_pointer()
            .child(
                div()
                    .text_color(Theme::text_primary())
                    .text_sm()
                    .child(self.icon)
            );

        if self.active {
            button = button.bg(Theme::accent());
        } else {
            button = button.hover(|style| {
                style.bg(Theme::bg_dark())
            });
        }

        button
    }
}
