//! Input bar with text field and buttons
//!
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003.4

use gpui::{div, px, prelude::*, IntoElement};

pub struct InputBar {
    text: String,
    is_streaming: bool,
    on_send: Option<Box<dyn Fn(String) + Send + Sync + 'static>>,
    on_stop: Option<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl InputBar {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            is_streaming: false,
            on_send: None,
            on_stop: None,
        }
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn is_streaming(mut self, streaming: bool) -> Self {
        self.is_streaming = streaming;
        self
    }

    pub fn on_send(mut self, f: impl Fn(String) + Send + Sync + 'static) -> Self {
        self.on_send = Some(Box::new(f));
        self
    }

    pub fn on_stop(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_stop = Some(Box::new(f));
        self
    }
}

impl IntoElement for InputBar {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;
        
        let is_streaming = self.is_streaming;

        let mut input_div = div()
            .flex()
            .items_center()
            .gap(px(Theme::SPACING_SM))
            .p(px(Theme::SPACING_MD))
            .w_full()
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::bg_dark())
            .rounded(px(Theme::RADIUS_LG));

        // Text input area (placeholder)
        let text_display = if self.text.is_empty() {
            div()
                .flex_1()
                .text_color(Theme::text_muted())
                .child("Type your message...")
        } else {
            div()
                .flex_1()
                .text_color(Theme::text_primary())
                .child(self.text.clone())
        };

        input_div = input_div.child(text_display);

        // Button: Send or Stop
        let button_label = if is_streaming { "Stop" } else { "Send" };
        let button = crate::ui_gpui::components::Button::new(button_label)
            .active(is_streaming);

        input_div = input_div.child(button);

        input_div
    }
}
