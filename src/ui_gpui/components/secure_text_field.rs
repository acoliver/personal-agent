//! Password input field with masked display
//!
//! @plan PLAN-20250130-GPUIREDUX.P02
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, px, Context, FocusHandle, Focusable, Styled, Window};
use std::cell::RefCell;
use std::rc::Rc;

type TextCallback<T> = Rc<RefCell<dyn Fn(&str, &mut Context<T>)>>;

pub struct SecureTextField {
    focus_handle: FocusHandle,
    text: Rc<RefCell<String>>,
    masked: Rc<RefCell<bool>>,
    placeholder: String,
    on_change: Option<TextCallback<Self>>,
    on_submit: Option<TextCallback<Self>>,
}

impl SecureTextField {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            text: Rc::new(RefCell::new(String::new())),
            masked: Rc::new(RefCell::new(true)),
            placeholder: String::new(),
            on_change: None,
            on_submit: None,
        }
    }

    #[must_use]
    pub fn with_text(self, text: impl Into<String>) -> Self {
        *self.text.borrow_mut() = text.into();
        self
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.text.borrow().clone()
    }

    #[must_use]
    pub fn display_text(&self) -> String {
        if *self.masked.borrow() {
            "*".repeat(self.text.borrow().len())
        } else {
            self.text.borrow().clone()
        }
    }

    #[must_use]
    pub fn actual_text(&self) -> String {
        self.text.borrow().clone()
    }

    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let mut current = self.text.borrow_mut();
        current.clear();
        current.push_str(text);
        drop(current);
        if let Some(on_change) = &self.on_change {
            (on_change.borrow())(text, cx);
        }
        cx.notify();
    }

    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    #[must_use]
    pub fn masked(self, masked: bool) -> Self {
        *self.masked.borrow_mut() = masked;
        self
    }

    pub fn toggle_mask(&mut self, cx: &mut Context<Self>) {
        let current = *self.masked.borrow();
        *self.masked.borrow_mut() = !current;
        cx.notify();
    }

    #[must_use]
    pub fn on_change(mut self, callback: impl Fn(&str, &mut Context<Self>) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(callback)));
        self
    }

    #[must_use]
    pub fn on_submit(mut self, callback: impl Fn(&str, &mut Context<Self>) + 'static) -> Self {
        self.on_submit = Some(Rc::new(RefCell::new(callback)));
        self
    }

    #[must_use]
    pub const fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }
}

impl Focusable for SecureTextField {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SecureTextField {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        use crate::ui_gpui::theme::Theme;

        let is_focused = self.focus_handle.is_focused(window);
        let display_text = self.display_text();
        let placeholder = self.placeholder.clone();
        let masked = *self.masked.borrow();
        let has_text = !self.text.borrow().is_empty();

        let mut content_div = div().flex_1();

        if has_text {
            content_div = content_div.child(
                div()
                    .text_color(Theme::text_primary())
                    .text_sm()
                    .child(display_text),
            );
        } else if !placeholder.is_empty() {
            content_div = content_div.child(
                div()
                    .text_color(Theme::text_muted())
                    .text_sm()
                    .child(placeholder),
            );
        }

        let mut main_div = div()
            .flex()
            .items_center()
            .gap(px(Theme::SPACING_SM))
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .w_full()
            .min_h(px(36.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(if is_focused {
                Theme::accent()
            } else {
                Theme::bg_dark()
            })
            .rounded(px(Theme::RADIUS_MD))
            .child(content_div);

        if has_text {
            let icon = if masked { "•" } else { "◦" };
            main_div = main_div.child(
                div()
                    .text_xs()
                    .text_color(Theme::text_secondary())
                    .cursor_pointer()
                    .child(icon),
            );
        }

        main_div
    }
}
