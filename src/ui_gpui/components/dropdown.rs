//! Dropdown selection component with popup overlay
//!
//! @plan PLAN-20250130-GPUIREDUX.P02
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, px, IntoElement, MouseButton, Styled};
use std::cell::RefCell;
use std::rc::Rc;

type OnSelectCallback = Rc<RefCell<dyn Fn(usize)>>;

pub struct Dropdown {
    options: Vec<String>,
    selected_index: Rc<RefCell<usize>>,
    is_open: Rc<RefCell<bool>>,
    on_select: Option<OnSelectCallback>,
}

impl Dropdown {
    #[must_use]
    pub fn new(options: Vec<String>) -> Self {
        Self {
            options,
            selected_index: Rc::new(RefCell::new(0)),
            is_open: Rc::new(RefCell::new(false)),
            on_select: None,
        }
    }

    #[must_use]
    pub fn selected(self, index: usize) -> Self {
        *self.selected_index.borrow_mut() = index.min(self.options.len().saturating_sub(1));
        self
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        *self.selected_index.borrow()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        *self.is_open.borrow()
    }

    #[must_use]
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
        let options = self.options;
        let selected_text = options.get(selected_idx).cloned().unwrap_or_default();
        let is_open_state = Rc::clone(&self.is_open);
        let selected_state = Rc::clone(&self.selected_index);
        let on_select = self.on_select.clone();

        let mut dropdown = div().relative().w(px(200.0));

        // Main button showing current selection
        let button = Theme::dropdown(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(Theme::SPACING_MD))
                .py(px(Theme::SPACING_SM))
                .w(px(200.0))
                .min_h(px(36.0))
                .border_1()
                .rounded(px(Theme::RADIUS_MD))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, {
                    let is_open_state = Rc::clone(&is_open_state);
                    move |_, _, _| {
                        let current = *is_open_state.borrow();
                        *is_open_state.borrow_mut() = !current;
                    }
                })
                .child(div().flex_1().text_sm().child(selected_text))
                .child(
                    div()
                        .text_color(Theme::text_secondary())
                        .text_sm()
                        .child(if is_open { "▲" } else { "▼" }),
                ),
        );

        dropdown = dropdown.child(button);

        // Popup overlay with options
        if is_open {
            let options_div = Theme::dropdown(
                div()
                    .absolute()
                    .top(px(40.0))
                    .left(px(0.0))
                    .w(px(200.0))
                    .max_h(px(300.0))
                    .border_1()
                    .rounded(px(Theme::RADIUS_MD))
                    .children(options.iter().enumerate().map(|(idx, opt)| {
                        let is_selected = idx == selected_idx;
                        let option_label = opt.clone();
                        let option_on_select = on_select.clone();
                        let option_selected_state = Rc::clone(&selected_state);
                        let option_open_state = Rc::clone(&is_open_state);

                        let option_row = div()
                            .flex()
                            .items_center()
                            .px(px(Theme::SPACING_MD))
                            .py(px(Theme::SPACING_SM))
                            .w_full()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                *option_selected_state.borrow_mut() = idx;
                                *option_open_state.borrow_mut() = false;
                                if let Some(callback) = &option_on_select {
                                    (callback.borrow())(idx);
                                }
                            })
                            .child(div().flex_1().text_sm().child(option_label));

                        if is_selected {
                            Theme::list_row_selected(option_row)
                        } else {
                            Theme::dropdown_item(option_row)
                        }
                    })),
            );

            dropdown = dropdown.child(options_div);
        }

        dropdown
    }
}
