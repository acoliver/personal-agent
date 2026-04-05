use gpui::{div, hsla, px, Div, InteractiveElement, Styled};

use super::Theme;

impl Theme {
    /// A panel container with paired background, foreground, and border colors.
    #[must_use]
    pub fn panel<E>(element: E) -> E
    where
        E: Styled,
    {
        element
            .bg(Self::bg_dark())
            .text_color(Self::text_primary())
            .border_color(Self::border())
    }

    /// A panel header row with paired colors.
    #[must_use]
    pub fn panel_header<E>(element: E) -> E
    where
        E: Styled,
    {
        element
            .bg(Self::bg_darker())
            .text_color(Self::text_primary())
            .border_color(Self::border())
    }

    /// Input surface colors for text fields and dropdowns.
    #[must_use]
    pub fn input<E>(element: E) -> E
    where
        E: Styled,
    {
        element
            .bg(Self::bg_darker())
            .text_color(Self::text_primary())
            .border_color(Self::border())
    }

    /// Primary action button colors.
    #[must_use]
    pub fn button_primary<E>(element: E) -> E
    where
        E: Styled + InteractiveElement,
    {
        element
            .bg(Self::accent())
            .text_color(Self::selection_fg())
            .hover(|s| s.bg(Self::accent_hover()))
    }

    /// Primary action button colors for disabled state.
    #[must_use]
    pub fn button_primary_disabled<E>(element: E) -> E
    where
        E: Styled,
    {
        element.bg(Self::bg_dark()).text_color(Self::text_muted())
    }

    /// Secondary button colors.
    #[must_use]
    pub fn button_secondary<E>(element: E) -> E
    where
        E: Styled + InteractiveElement,
    {
        element
            .bg(Self::bg_dark())
            .text_color(Self::text_primary())
            .hover(|s| s.bg(Self::bg_darker()))
    }

    /// Danger button colors.
    #[must_use]
    pub fn button_danger<E>(element: E) -> E
    where
        E: Styled + InteractiveElement,
    {
        element
            .bg(Self::error())
            .text_color(Self::selection_fg())
            .hover(|s| s.bg(Self::danger()))
    }

    /// Ghost button colors for low-emphasis actions.
    #[must_use]
    pub fn button_ghost<E>(element: E) -> E
    where
        E: Styled + InteractiveElement,
    {
        element
            .bg(hsla(0.0, 0.0, 0.0, 0.0))
            .text_color(Self::text_secondary())
            .hover(|s| s.bg(Self::bg_dark()))
    }

    /// Toolbar button colors.
    #[must_use]
    pub fn toolbar_button<E>(element: E) -> E
    where
        E: Styled + InteractiveElement,
    {
        element
            .bg(Self::bg_dark())
            .text_color(Self::text_primary())
            .hover(|s| s.bg(Self::bg_darker()))
    }

    /// Disabled toolbar button colors.
    #[must_use]
    pub fn toolbar_button_disabled<E>(element: E) -> E
    where
        E: Styled,
    {
        element.bg(Self::bg_darker()).text_color(Self::text_muted())
    }

    /// List row colors for unselected items.
    #[must_use]
    pub fn list_row<E>(element: E) -> E
    where
        E: Styled + InteractiveElement,
    {
        element
            .bg(hsla(0.0, 0.0, 0.0, 0.0))
            .text_color(Self::text_primary())
            .hover(|s| s.bg(Self::bg_dark()))
    }

    /// List row colors for selected items.
    #[must_use]
    pub fn list_row_selected<E>(element: E) -> E
    where
        E: Styled,
    {
        element
            .bg(Self::selection_bg())
            .text_color(Self::selection_fg())
    }

    /// Dropdown surface colors.
    #[must_use]
    pub fn dropdown<E>(element: E) -> E
    where
        E: Styled,
    {
        element
            .bg(Self::bg_darker())
            .text_color(Self::text_primary())
            .border_color(Self::bg_dark())
    }

    /// Dropdown item colors.
    #[must_use]
    pub fn dropdown_item<E>(element: E) -> E
    where
        E: Styled + InteractiveElement,
    {
        Self::list_row(element)
    }

    /// Badge label colors.
    #[must_use]
    pub fn badge<E>(element: E) -> E
    where
        E: Styled,
    {
        element.bg(Self::bg_dark()).text_color(Self::text_muted())
    }

    /// Section header label colors.
    #[must_use]
    pub fn section_header<E>(element: E) -> E
    where
        E: Styled,
    {
        element
            .bg(hsla(0.0, 0.0, 0.0, 0.0))
            .text_color(Self::text_muted())
    }

    /// User chat bubble colors.
    #[must_use]
    pub fn user_bubble<E>(element: E) -> E
    where
        E: Styled,
    {
        element
            .bg(Self::user_bubble_bg())
            .text_color(Self::user_bubble_text())
    }

    /// Assistant chat bubble colors.
    #[must_use]
    pub fn assistant_bubble<E>(element: E) -> E
    where
        E: Styled,
    {
        element
            .bg(Self::assistant_bubble_bg())
            .text_color(Self::text_primary())
            .border_color(Self::border())
    }

    /// Horizontal divider element.
    #[must_use]
    pub fn divider() -> Div {
        div().h(px(1.0)).w_full().bg(Self::border())
    }
}
