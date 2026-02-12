//! Toggle on/off switch component
//!
//! @plan PLAN-20250130-GPUIREDUX.P02
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, px, IntoElement, Styled, Window};
use std::rc::Rc;
use std::cell::RefCell;

pub struct Toggle {
    is_on: Rc<RefCell<bool>>,
    on_change: Option<Rc<RefCell<dyn Fn(bool)>>>,
}

impl Toggle {
    pub fn new(is_on: bool) -> Self {
        Self {
            is_on: Rc::new(RefCell::new(is_on)),
            on_change: None,
        }
    }

    pub fn is_on(&self) -> bool {
        *self.is_on.borrow()
    }

    pub fn on_change(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(callback)));
        self
    }

    pub fn handle_click(&self) {
        let current = *self.is_on.borrow();
        *self.is_on.borrow_mut() = !current;
        
        if let Some(on_change) = &self.on_change {
            (on_change.borrow())(*self.is_on.borrow());
        }
    }
}

impl IntoElement for Toggle {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;
        
        let is_on = *self.is_on.borrow();

        div()
            .relative()
            .w(px(44.0))
            .h(px(24.0))
            .rounded(px(12.0))
            .cursor_pointer()
            .bg(if is_on {
                Theme::accent()
            } else {
                Theme::bg_dark()
            })
            .child(
                div()
                    .absolute()
                    .top(px(2.0))
                    .left(if is_on { px(22.0) } else { px(2.0) })
                    .w(px(20.0))
                    .h(px(20.0))
                    .rounded(px(10.0))
                    .bg(gpui::white())
            )
    }
}
