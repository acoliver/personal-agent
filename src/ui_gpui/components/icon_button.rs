//! Icon button component for toolbar actions
//!
//! @plan PLAN-20250130-GPUIREDUX.P02
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, px, IntoElement, MouseButton, Styled};

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

    #[must_use]
    pub const fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    #[must_use]
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    #[must_use]
    pub fn on_click(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl IntoElement for IconButton {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;

        let on_click = self.on_click;

        let button = div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.0))
            .rounded(px(Theme::RADIUS_SM))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                if let Some(ref callback) = on_click {
                    (callback)();
                }
            })
            .child(div().text_sm().child(self.icon));

        if self.active {
            Theme::button_primary(button)
        } else {
            Theme::toolbar_button(button)
        }
    }
}
