//! Dropdown selection component with popup overlay
//!
//! @plan PLAN-20250130-GPUIREDUX.P02
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, px, IntoElement, Styled, Window};
use std::rc::Rc;
use std::cell::RefCell;

pub struct Dropdown {
    options: Vec<String>,
    selected_index: Rc<RefCell<usize>>,
    is_open: Rc<RefCell<bool>>,
    on_select: Option<Rc<RefCell<dyn Fn(usize)>>>,
}

impl Dropdown {
    pub fn new(options: Vec<String>) -> Self {
        Self {
            options,
            selected_index: Rc::new(RefCell::new(0)),
            is_open: Rc::new(RefCell::new(false)),
            on_select: None,
        }
    }

    pub fn selected(self, index: usize) -> Self {
        *self.selected_index.borrow_mut() = index.min(self.options.len().saturating_sub(1));
        self
    }

    pub fn selected_index(&self) -> usize {
        *self.selected_index.borrow()
    }

    pub fn is_open(&self) -> bool {
        *self.is_open.borrow()
    }

    pub fn on_select(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_select = Some(Rc::new(RefCell::new(callback)));
        self
    }

    pub fn handle_click(&self) {
        let current = *self.is_open.borrow();
        *self.is_open.borrow_mut() = !current;
    }

    pub fn select_item(&self, index: usize) {
        *self.selected_index.borrow_mut() = index;
        *self.is_open.borrow_mut() = false;
        
        if let Some(on_select) = &self.on_select {
            (on_select.borrow())(index);
        }
    }
}

impl IntoElement for Dropdown {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;
        
        let selected_idx = *self.selected_index.borrow();
        let is_open = *self.is_open.borrow();
        let options = self.options.clone();
        let selected_text = options.get(selected_idx).cloned().unwrap_or_default();

        let mut dropdown = div()
            .relative()
            .w(px(200.0));

        // Main button showing current selection
        let button = div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .w(px(200.0))
            .min_h(px(36.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::bg_dark())
            .rounded(px(Theme::RADIUS_MD))
            .cursor_pointer()
            .child(
                div()
                    .flex_1()
                    .text_color(Theme::text_primary())
                    .text_sm()
                    .child(selected_text)
            )
            .child(
                div()
                    .text_color(Theme::text_secondary())
                    .text_sm()
                    .child(if is_open { "▲" } else { "▼" })
            );

        dropdown = dropdown.child(button);

        // Popup overlay with options
        if is_open {
            let options_div = div()
                .absolute()
                .top(px(40.0))
                .left(px(0.0))
                .w(px(200.0))
                .max_h(px(300.0))
                .bg(Theme::bg_darker())
                .border_1()
                .border_color(Theme::bg_dark())
                .rounded(px(Theme::RADIUS_MD))
                .children(
                    options.iter().enumerate().map(|(idx, opt)| {
                        let is_selected = idx == selected_idx;
                        div()
                            .flex()
                            .items_center()
                            .px(px(Theme::SPACING_MD))
                            .py(px(Theme::SPACING_SM))
                            .w_full()
                            .cursor_pointer()
                            .hover(|style| {
                                style.bg(Theme::bg_dark())
                            })
                            .when(is_selected, |d| {
                                d.bg(Theme::bg_dark())
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(Theme::text_primary())
                                    .text_sm()
                                    .child(opt.clone())
                            )
                    })
                );

            dropdown = dropdown.child(options_div);
        }

        dropdown
    }
}
