//! Scrollable list container component
//!
//! @plan PLAN-20250130-GPUIREDUX.P02
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, px, IntoElement, Styled, Window};
use std::rc::Rc;
use std::cell::RefCell;

pub struct List<T: Clone> {
    items: Vec<T>,
    selected_index: Rc<RefCell<Option<usize>>>,
    render_item: Option<Box<dyn Fn(&T, bool) -> gpui::Div + 'static>>,
    on_select: Option<Rc<RefCell<dyn Fn(usize)>>>,
}

impl<T: Clone + 'static> List<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            selected_index: Rc::new(RefCell::new(None)),
            render_item: None,
            on_select: None,
        }
    }

    pub fn selected(self, index: usize) -> Self {
        *self.selected_index.borrow_mut() = Some(index);
        self
    }

    pub fn selected_index(&self) -> Option<usize> {
        *self.selected_index.borrow()
    }

    pub fn render_item(mut self, f: impl Fn(&T, bool) -> gpui::Div + 'static) -> Self {
        self.render_item = Some(Box::new(f));
        self
    }

    pub fn on_select(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_select = Some(Rc::new(RefCell::new(callback)));
        self
    }

    pub fn select_row(&self, index: usize) {
        *self.selected_index.borrow_mut() = Some(index);
        
        if let Some(on_select) = &self.on_select {
            (on_select.borrow())(index);
        }
    }
}

impl<T: Clone + 'static> IntoElement for List<T> {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;
        
        let selected_idx = *self.selected_index.borrow();
        let items = self.items.clone();
        let render_fn = self.render_item;

        div()
            .flex()
            .flex_col()
            .w_full()
            .children(
                items.iter().enumerate().map(|(idx, item)| {
                    let is_selected = selected_idx == Some(idx);
                    
                    if let Some(ref render_fn) = render_fn {
                        render_fn(item, is_selected)
                    } else {
                        div()
                            .flex()
                            .items_center()
                            .px(px(Theme::SPACING_MD))
                            .py(px(Theme::SPACING_SM))
                            .w_full()
                            .cursor_pointer()
                            .when(is_selected, |d| {
                                d.bg(Theme::bg_dark())
                            })
                            .child(
                                div()
                                    .text_color(Theme::text_primary())
                                    .text_sm()
                                    .child(format!("Item {}", idx))
                            )
                    }
                })
            )
    }
}
