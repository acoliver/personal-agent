//! Message bubble components for chat
//!
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, IntoElement, px};

pub struct UserBubble {
    content: String,
}

impl UserBubble {
    pub fn new(content: impl Into<String>) -> Self {
        Self { content: content.into() }
    }
}

impl IntoElement for UserBubble {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;
        
        div()
            .flex()
            .justify_end()
            .w_full()
            .child(
                div()
                    .w(px(400.0))
                    .px(px(Theme::SPACING_MD))
                    .py(px(Theme::SPACING_SM))
                    .rounded(px(Theme::RADIUS_LG))
                    .bg(Theme::bg_dark())
                    .text_color(Theme::text_primary())
                    .child(self.content)
            )
    }
}

pub struct AssistantBubble {
    content: String,
    model_id: Option<String>,
    thinking: Option<String>,
    show_thinking: bool,
    is_streaming: bool,
}

impl AssistantBubble {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            model_id: None,
            thinking: None,
            show_thinking: false,
            is_streaming: false,
        }
    }

    pub fn model_id(mut self, id: impl Into<String>) -> Self {
        self.model_id = Some(id.into());
        self
    }

    pub fn thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking = Some(thinking.into());
        self
    }

    pub fn show_thinking(mut self, show: bool) -> Self {
        self.show_thinking = show;
        self
    }

    pub fn streaming(mut self, is_streaming: bool) -> Self {
        self.is_streaming = is_streaming;
        self
    }
}

impl IntoElement for AssistantBubble {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;
        
        let mut bubble = div()
            .flex()
            .flex_col()
            .items_start()
            .w_full()
            .gap(px(Theme::SPACING_SM));

        // Thinking section (if show_thinking and thinking is present)
        if self.show_thinking {
            if let Some(thinking_content) = self.thinking {
                bubble = bubble.child(
                    div()
                        .w(px(400.0))
                        .px(px(Theme::SPACING_MD))
                        .py(px(Theme::SPACING_SM))
                        .rounded(px(Theme::RADIUS_MD))
                        .bg(Theme::bg_darker())
                        .text_color(Theme::text_secondary())
                        .text_sm()
                        .child(format!("Thinking: {}", thinking_content))
                );
            }
        }

        // Main content (with cursor if streaming)
        let content_text = if self.is_streaming {
            format!("{}▋", self.content)
        } else {
            self.content.clone()
        };

        let main_content = div()
            .w(px(400.0))
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .rounded(px(Theme::RADIUS_LG))
            .bg(Theme::bg_darker())
            .text_color(Theme::text_primary())
            .child(content_text);

        bubble = bubble.child(main_content);

        // Model ID (if present)
        if let Some(model_id) = self.model_id {
            bubble = bubble.child(
                div()
                    .text_sm()
                    .text_color(Theme::text_muted())
                    .child(format!("via {}", model_id))
            );
        }

        bubble
    }
}
