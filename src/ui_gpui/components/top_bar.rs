//! Top bar header component for views
//!
//! @plan PLAN-20250130-GPUIREDUX.P02
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, px, IntoElement, Styled, Window};

pub struct TopBarButton {
    label: String,
    on_click: Option<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl TopBarButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
        }
    }

    pub fn on_click(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

pub struct TopBar {
    title: String,
    left_button: Option<TopBarButton>,
    right_buttons: Vec<TopBarButton>,
}

impl TopBar {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            left_button: None,
            right_buttons: Vec::new(),
        }
    }

    pub fn left_button(mut self, button: TopBarButton) -> Self {
        self.left_button = Some(button);
        self
    }

    pub fn right_button(mut self, button: TopBarButton) -> Self {
        self.right_buttons.push(button);
        self
    }

    pub fn right_buttons(mut self, buttons: Vec<TopBarButton>) -> Self {
        self.right_buttons = buttons;
        self
    }
}

impl IntoElement for TopBar {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;
        
        let mut top_bar = div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(44.0))
            .px(px(Theme::SPACING_MD))
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::bg_dark());

        // Left button (back button, etc.)
        if let Some(left_btn) = self.left_button {
            let btn = crate::ui_gpui::components::Button::new(left_btn.label)
                .on_click(left_btn.on_click.unwrap_or(Box::new(|| {})));
            top_bar = top_bar.child(btn);
        } else {
            top_bar = top_bar.child(div().w(px(Theme::SPACING_LG)));
        }

        // Title in center
        top_bar = top_bar.child(
            div()
                .flex_1()
                .text_color(Theme::text_primary())
                .text_lg()
                .font_weight(gpui::FontWeight::BOLD)
                .child(self.title)
        );

        // Right buttons
        for btn in self.right_buttons {
            let button = crate::ui_gpui::components::Button::new(btn.label)
                .on_click(btn.on_click.unwrap_or(Box::new(|| {})));
            top_bar = top_bar.child(button);
        }

        top_bar
    }
}
