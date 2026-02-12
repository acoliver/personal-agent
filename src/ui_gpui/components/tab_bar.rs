//! Tab bar component for view navigation
//!
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-002

use gpui::{div, px, prelude::*, IntoElement};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Chat,
    History,
    Settings,
}

pub struct TabBar {
    active: Tab,
    on_select: Option<Box<dyn Fn(Tab) + Send + Sync + 'static>>,
}

impl TabBar {
    pub fn new(active: Tab) -> Self {
        Self { active, on_select: None }
    }

    pub fn on_select(mut self, f: impl Fn(Tab) + Send + Sync + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }
}

impl IntoElement for TabBar {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;
        
        let tabs = [Tab::Chat, Tab::History, Tab::Settings];
        let active = self.active;

        let mut tab_bar = div()
            .flex()
            .items_center()
            .gap(px(Theme::SPACING_XS))
            .p(px(Theme::SPACING_SM))
            .bg(Theme::bg_darker())
            .rounded(px(Theme::RADIUS_LG));

        for tab in tabs {
            let is_active = active == tab;
            let tab_label = match tab {
                Tab::Chat => "Chat",
                Tab::History => "History",
                Tab::Settings => "Settings",
            };

            let tab_button = crate::ui_gpui::components::Button::new(tab_label)
                .active(is_active);

            tab_bar = tab_bar.child(tab_button);
        }

        tab_bar
    }
}
