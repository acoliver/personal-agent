//! Appearance panel rendering for `SettingsView`.

use super::{FontDropdownTarget, SettingsView};
use crate::ui_gpui::theme::{Theme, MAX_FONT_SIZE, MIN_FONT_SIZE};
use gpui::{div, prelude::*, px, FontWeight, MouseButton, SharedString};

fn all_font_names(cx: &gpui::App) -> Vec<String> {
    cx.text_system().all_font_names()
}

fn ui_font_options(cx: &gpui::App) -> Vec<String> {
    let mut options = all_font_names(cx)
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .collect::<Vec<_>>();
    options.insert(0, "System Default".to_string());
    options
}

fn mono_font_options(cx: &gpui::App) -> Vec<String> {
    const MONO_HINTS: &[&str] = &[
        "mono",
        "code",
        "courier",
        "consol",
        "inconsol",
        "jetbrains",
        "menlo",
        "monaco",
        "source code",
        "fira",
        "hack",
        "ubuntu mono",
    ];

    let mut options = all_font_names(cx)
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            MONO_HINTS.iter().any(|hint| lower.contains(hint))
        })
        .collect::<Vec<_>>();

    if !options
        .iter()
        .any(|font| font == crate::ui_gpui::theme::DEFAULT_MONO_FONT_FAMILY)
    {
        options.push(crate::ui_gpui::theme::DEFAULT_MONO_FONT_FAMILY.to_string());
    }

    options.sort();
    options.dedup();
    options
}

impl SettingsView {
    /// Appearance panel: two-column layout with controls (left) and preview (right).
    ///
    /// The controls column scrolls independently while the preview stays visible.
    /// Dropdowns render as overlays to avoid pushing content and causing unwanted scroll.
    pub(super) fn render_appearance_panel(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let any_dropdown_open =
            self.state.theme_dropdown_open || self.state.font_dropdown_open_for.is_some();

        div()
            .id("appearance-panel")
            .relative()
            .flex()
            .flex_row()
            .h_full()
            .w_full()
            .gap(px(16.0))
            .overflow_hidden()
            // Left column: controls with independent scroll
            .child(self.render_controls_column(cx))
            // Right column: preview (fixed, always visible)
            .child(self.render_preview_column())
            // Dropdown overlays — rendered at root level so they don't push content
            .when(self.state.theme_dropdown_open, |d| {
                d.child(self.render_theme_dropdown_overlay(cx))
            })
            .when_some(self.state.font_dropdown_open_for, |d, target| {
                d.child(self.render_font_dropdown_overlay(target, cx))
            })
            // Invisible backdrop to close dropdowns when clicking outside
            .when(any_dropdown_open, |d| {
                d.child(Self::render_dropdown_backdrop(cx))
            })
    }

    /// Left column: all font controls with independent vertical scrolling.
    fn render_controls_column(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("appearance-controls-column")
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(280.0))
            .max_w(px(400.0))
            .min_h_0()
            .gap(px(16.0))
            .overflow_y_scroll()
            .child(self.render_theme_section(cx))
            .child(self.render_font_size_section(cx))
            .child(self.render_ui_font_section(cx))
            .child(self.render_mono_font_section(cx))
    }

    /// Right column: font preview, always visible.
    fn render_preview_column(&self) -> impl IntoElement {
        div()
            .id("appearance-preview-column")
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(240.0))
            .flex_shrink_0()
            .h_full()
            .items_start()
            .child(self.render_font_preview_section())
    }

    /// Invisible backdrop to catch clicks and close any open dropdown.
    fn render_dropdown_backdrop(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("appearance-dropdown-backdrop")
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .block_mouse_except_scroll()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.close_all_dropdowns();
                    cx.notify();
                }),
            )
    }

    /// Close all open dropdowns.
    pub(super) const fn close_all_dropdowns(&mut self) {
        self.state.theme_dropdown_open = false;
        self.state.font_dropdown_open_for = None;
    }

    /// Theme section.
    /// Note: dropdown list is rendered as an overlay, not inline.
    pub(super) fn render_theme_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let themes = &self.state.available_themes;
        let selected_name = themes
            .iter()
            .find(|t| t.slug == self.state.selected_theme_slug)
            .map_or_else(
                || self.state.selected_theme_slug.clone(),
                |t| t.name.clone(),
            );
        let is_open = self.state.theme_dropdown_open;

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("THEME"),
            )
            .child(self.render_theme_dropdown_trigger(&selected_name, is_open, cx))
    }

    /// Theme dropdown overlay (floating, positioned below trigger).
    fn render_theme_dropdown_overlay(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let themes = &self.state.available_themes;

        div()
            .id("theme-dropdown-overlay")
            .absolute()
            .top(px(56.0)) // Below header + trigger height
            .left(px(0.0))
            .min_w(px(200.0))
            .max_w(px(360.0))
            .max_h(px(200.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::accent())
            .rounded(px(4.0))
            .shadow_lg()
            .overflow_y_scroll()
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, _| {}))
            .children(themes.iter().map(|t| self.render_theme_row(t, cx)))
            .when(themes.is_empty(), |d| {
                d.items_center().justify_center().child(
                    div()
                        .text_size(px(Theme::font_size_mono()))
                        .text_color(Theme::text_muted())
                        .child("No themes available"),
                )
            })
    }

    /// Shared theme dropdown trigger button (used by Appearance panel).
    #[allow(clippy::unused_self)] // &self needed for cx.listener pattern
    pub(super) fn render_theme_dropdown_trigger(
        &self,
        selected_name: &str,
        is_open: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let selected_name = selected_name.to_string();
        div()
            .id("theme-dropdown-trigger")
            .w_full()
            .h(px(28.0))
            .px(px(8.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(if is_open {
                Theme::accent()
            } else {
                Theme::border()
            })
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .text_size(px(Theme::font_size_mono()))
            .text_color(Theme::text_primary())
            .child(selected_name)
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child(if is_open { "▲" } else { "▼" }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.toggle_theme_dropdown();
                    cx.notify();
                }),
            )
    }

    /// Single theme row in the dropdown list.
    fn render_theme_row(
        &self,
        option: &super::ThemeOption,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let slug = option.slug.clone();
        let name = option.name.clone();
        let is_selected = self.state.selected_theme_slug == slug;

        div()
            .id(SharedString::from(format!("theme-{slug}")))
            .w_full()
            .h(px(24.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .when(is_selected, |d| {
                d.bg(Theme::selection_bg())
                    .text_color(Theme::selection_fg())
            })
            .when(!is_selected, |d| {
                d.hover(|s| s.bg(Theme::bg_dark()))
                    .text_color(Theme::text_primary())
            })
            .text_size(px(Theme::font_size_mono()))
            .child(name)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    tracing::info!("Theme selected: {}", slug);
                    this.select_theme_from_dropdown(slug.clone(), cx);
                }),
            )
    }

    /// Font size stepper section.
    fn render_font_size_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let size = self.state.font_size;
        let at_min = size <= MIN_FONT_SIZE;
        let at_max = size >= MAX_FONT_SIZE;

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(Self::render_font_size_header())
            .child(Self::render_font_size_controls_row(
                size, at_min, at_max, cx,
            ))
            .child(Self::render_font_size_help_text())
    }

    fn render_font_size_header() -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child("FONT SIZE")
    }

    fn render_font_size_help_text() -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_muted())
            .child("Base size for all text. Headings and UI scale proportionally.")
    }

    fn render_font_size_controls_row(
        size: f32,
        at_min: bool,
        at_max: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(Self::render_font_size_decrement(size, at_min, cx))
            .child(Self::render_font_size_value(size))
            .child(Self::render_font_size_increment(size, at_max, cx))
            .child(Self::render_font_size_keyboard_hint())
    }

    fn render_font_size_decrement(
        size: f32,
        at_min: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("btn-font-size-dec")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::border())
            .text_size(px(Theme::font_size_body()))
            .text_color(if at_min {
                Theme::text_muted()
            } else {
                Theme::text_primary()
            })
            .child("-")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    if !at_min {
                        this.set_font_size(size - 1.0, cx);
                    }
                }),
            )
    }

    fn render_font_size_increment(
        size: f32,
        at_max: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("btn-font-size-inc")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::border())
            .text_size(px(Theme::font_size_body()))
            .text_color(if at_max {
                Theme::text_muted()
            } else {
                Theme::text_primary()
            })
            .child("+")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    if !at_max {
                        this.set_font_size(size + 1.0, cx);
                    }
                }),
            )
    }

    fn render_font_size_value(size: f32) -> impl IntoElement {
        div()
            .w(px(40.0))
            .h(px(28.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(Theme::font_size_mono()))
            .text_color(Theme::text_primary())
            .child(format!("{size}"))
    }

    fn render_font_size_keyboard_hint() -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_muted())
            .child("⌘+ / ⌘- to zoom")
    }

    /// UI font family dropdown section.
    /// Note: dropdown list is rendered as an overlay, not inline.
    fn render_ui_font_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let current_label = self
            .state
            .ui_font_family
            .as_deref()
            .unwrap_or("System Default")
            .to_string();
        let is_open = self.state.font_dropdown_open_for == Some(FontDropdownTarget::UiFont);

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(Self::render_ui_font_header())
            .child(Self::render_ui_font_trigger(current_label, is_open, cx))
            .child(Self::render_ui_font_help())
    }

    fn render_ui_font_header() -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child("UI FONT")
    }

    fn render_ui_font_help() -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_muted())
            .child("Labels, buttons, sidebar, settings chrome")
    }

    fn render_ui_font_trigger(
        current_label: String,
        is_open: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("ui-font-dropdown-trigger")
            .w_full()
            .h(px(28.0))
            .px(px(8.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(if is_open {
                Theme::accent()
            } else {
                Theme::border()
            })
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .text_size(px(Theme::font_size_mono()))
            .text_color(Theme::text_primary())
            .child(current_label)
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child(if is_open { "▲" } else { "▼" }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.toggle_font_dropdown(FontDropdownTarget::UiFont);
                    cx.notify();
                }),
            )
    }

    /// Mono font family dropdown + ligatures toggle section.
    /// Note: dropdown list is rendered as an overlay, not inline.
    fn render_mono_font_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let current_mono = self.state.mono_font_family.clone();
        let is_open = self.state.font_dropdown_open_for == Some(FontDropdownTarget::MonoFont);
        let ligatures = self.state.mono_ligatures;

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(Self::render_mono_font_header())
            .child(Self::render_mono_font_trigger(current_mono, is_open, cx))
            .child(Self::render_ligatures_toggle_row(ligatures, cx))
            .child(Self::render_mono_font_help())
    }

    fn render_mono_font_header() -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child("MONO FONT")
    }

    fn render_mono_font_help() -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_muted())
            .child("Inline code and code blocks in messages")
    }

    fn render_mono_font_trigger(
        current_mono: String,
        is_open: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("mono-font-dropdown-trigger")
            .w_full()
            .h(px(28.0))
            .px(px(8.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(if is_open {
                Theme::accent()
            } else {
                Theme::border()
            })
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .text_size(px(Theme::font_size_mono()))
            .text_color(Theme::text_primary())
            .child(current_mono)
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child(if is_open { "▲" } else { "▼" }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.toggle_font_dropdown(FontDropdownTarget::MonoFont);
                    cx.notify();
                }),
            )
    }

    /// Render font dropdown overlay for either UI or Mono font.
    fn render_font_dropdown_overlay(
        &self,
        target: FontDropdownTarget,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let fonts = match target {
            FontDropdownTarget::UiFont => ui_font_options(cx),
            FontDropdownTarget::MonoFont => mono_font_options(cx),
        };

        // Position differs based on which dropdown is open
        let top = match target {
            FontDropdownTarget::UiFont => px(224.0), // Below theme, font size, and UI font header
            FontDropdownTarget::MonoFont => px(310.0), // Below all controls
        };

        div()
            .id(SharedString::from(format!(
                "font-dropdown-overlay-{target:?}"
            )))
            .absolute()
            .top(top)
            .left(px(0.0))
            .min_w(px(200.0))
            .max_w(px(360.0))
            .max_h(px(300.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::accent())
            .rounded(px(4.0))
            .shadow_lg()
            .overflow_y_scroll()
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, _| {}))
            .children(fonts.iter().map(|font| {
                self.render_font_option_row(font, target, cx)
                    .into_any_element()
            }))
    }

    /// Render a single font option row in the dropdown overlay.
    fn render_font_option_row(
        &self,
        font: &str,
        target: FontDropdownTarget,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let font_str = font.to_string();
        let is_selected = match target {
            FontDropdownTarget::UiFont => {
                if font == "System Default" {
                    self.state.ui_font_family.is_none()
                } else {
                    self.state.ui_font_family.as_deref() == Some(font)
                }
            }
            FontDropdownTarget::MonoFont => self.state.mono_font_family == font,
        };

        div()
            .id(SharedString::from(format!("{target:?}-font-{font_str}")))
            .w_full()
            .h(px(24.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .when(is_selected, |d| {
                d.bg(Theme::selection_bg())
                    .text_color(Theme::selection_fg())
            })
            .when(!is_selected, |d| {
                d.hover(|s| s.bg(Theme::bg_dark()))
                    .text_color(Theme::text_primary())
            })
            .text_size(px(Theme::font_size_mono()))
            .child(font_str.clone())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| match target {
                    FontDropdownTarget::UiFont => {
                        let value = if font_str == "System Default" {
                            None
                        } else {
                            Some(font_str.clone())
                        };
                        this.select_ui_font(value, cx);
                    }
                    FontDropdownTarget::MonoFont => {
                        this.select_mono_font(font_str.clone(), cx);
                    }
                }),
            )
    }

    fn render_ligatures_toggle_row(
        ligatures: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("ligatures-toggle-row")
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.toggle_mono_ligatures(cx);
                }),
            )
            .child(Self::render_ligatures_indicator(ligatures))
            .child(
                div()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .child("Ligatures"),
            )
    }

    fn render_ligatures_indicator(ligatures: bool) -> impl IntoElement {
        div()
            .size(px(14.0))
            .rounded(px(2.0))
            .border_1()
            .border_color(Theme::border())
            .bg(if ligatures {
                Theme::accent()
            } else {
                Theme::bg_dark()
            })
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .when(ligatures, |d| d.child("[OK]"))
    }

    /// Live font preview section.
    pub(super) fn render_font_preview_section(&self) -> impl IntoElement {
        let base_size = self.state.font_size;
        let h3_size = base_size * 1.25;
        let body_size = base_size;
        let mono_size = (base_size * 9.0) / 10.0;
        let ui_family = self.state.ui_font_family.as_ref().map(SharedString::from);
        let mono_family = SharedString::from(self.state.mono_font_family.clone());
        let mono_features = if self.state.mono_ligatures {
            gpui::FontFeatures::default()
        } else {
            gpui::FontFeatures::disable_ligatures()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("PREVIEW"),
            )
            .child(
                div()
                    .id("font-preview-box")
                    .when_some(ui_family, gpui::Styled::font_family)
                    .w_full()
                    .p(px(12.0))
                    .bg(Theme::bg_darker())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    // Heading line
                    .child(
                        div()
                            .text_size(px(h3_size))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child("Heading Text"),
                    )
                    // Body line
                    .child(
                        div()
                            .text_size(px(body_size))
                            .text_color(Theme::text_primary())
                            .child("Body text looks like this in messages."),
                    )
                    // Mixed line: mono code span + body continuation
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(mono_size))
                                    .font_family(mono_family)
                                    .font_features(mono_features)
                                    .text_color(Theme::accent())
                                    .bg(Theme::bg_dark())
                                    .px(px(4.0))
                                    .rounded(px(2.0))
                                    .child("fn main()"),
                            )
                            .child(
                                div()
                                    .text_size(px(body_size))
                                    .text_color(Theme::text_primary())
                                    .child(" inline with body"),
                            ),
                    ),
            )
    }
}
