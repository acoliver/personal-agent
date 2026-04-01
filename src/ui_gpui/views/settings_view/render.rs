//! Render implementation for `SettingsView`.

use super::{McpItem, McpStatus, ProfileItem, SettingsView};
use crate::events::types::UserEvent;
use crate::ui_gpui::theme::Theme;
use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FontWeight, MouseButton, Pixels,
    SharedString,
};

impl SettingsView {
    /// Render the top bar with back button and title
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_top_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("settings-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            // Left: back button + title
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // Back button - uses navigation_channel
                    .child(
                        div()
                            .id("btn-back")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_secondary())
                            .child("<")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_this, _, _window, _cx| {
                                    tracing::info!("Back clicked - navigating to Chat");
                                    crate::ui_gpui::navigation_channel().request_navigate(
                                        crate::presentation::view_command::ViewId::Chat,
                                    );
                                }),
                            ),
                    )
                    // Title
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child("Settings"),
                    ),
            )
            // Right: Refresh Models button
            .child(
                div()
                    .id("btn-refresh-models")
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .child("Refresh Models")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, _cx| {
                            tracing::info!("Refresh Models clicked");
                            this.emit(&UserEvent::RefreshModelsRegistry);
                        }),
                    ),
            )
    }

    /// Render a single profile row
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_profile_row(
        &self,
        profile: &ProfileItem,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let profile_id = profile.id;
        let is_selected = self.state.selected_profile_id == Some(profile_id);
        let display_text = profile.display_text();

        div()
            .id(SharedString::from(format!("profile-{profile_id}")))
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
            .text_size(px(12.0))
            .child(display_text)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    tracing::info!("Profile selected: {}", profile_id);
                    this.select_profile(profile_id, cx);
                }),
            )
            .into_any_element()
    }

    /// Render the profiles section
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_profiles_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let profiles = &self.state.profiles;
        let total_profiles = profiles.len();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            // Section header
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_primary())
                    .child("PROFILES"),
            )
            // List box
            .child(
                div()
                    .id("profiles-list")
                    .w_full()
                    .h(px(100.0))
                    .bg(Theme::bg_darker())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(profiles.iter().map(|p| self.render_profile_row(p, cx)))
                    .when(profiles.is_empty(), |d| {
                        d.items_center().justify_center().child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::text_muted())
                                .child("No profiles configured"),
                        )
                    })
                    .when(total_profiles > 0, |d| {
                        d.child(
                            div()
                                .w_full()
                                .px(px(8.0))
                                .pb(px(2.0))
                                .text_size(px(10.0))
                                .text_color(Theme::text_muted())
                                .child(format!("{total_profiles} profiles")),
                        )
                    }),
            )
            // Toolbar: [-] [+] [spacer] [Edit]
            .child(self.render_profiles_toolbar(cx))
    }

    /// Profiles section toolbar: [-] [+] [spacer] [Edit]
    fn render_profiles_toolbar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let has_selection = self.state.selected_profile_id.is_some();

        div()
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.0))
            // [-] Delete button
            .child(
                div()
                    .id("btn-delete-profile")
                    .size(px(28.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(has_selection, |d| d.hover(|s| s.bg(Theme::danger())))
                    .when(!has_selection, |d| d.text_color(Theme::text_muted()))
                    .text_size(px(14.0))
                    .text_color(if has_selection {
                        Theme::text_primary()
                    } else {
                        Theme::text_muted()
                    })
                    .child("-")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, _cx| {
                            if let Some(id) = this.state.selected_profile_id {
                                tracing::info!("Delete profile clicked: {}", id);
                            }
                            this.delete_selected_profile();
                        }),
                    ),
            )
            // [+] Add button - uses navigation_channel
            .child(
                div()
                    .id("btn-add-profile")
                    .size(px(28.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(14.0))
                    .text_color(Theme::text_primary())
                    .child("+")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            tracing::info!("Add profile clicked - navigating to ModelSelector");
                            Self::navigate_to_profile_editor();
                        }),
                    ),
            )
            // Spacer
            .child(div().flex_1())
            // [Edit] button - emits event (presenter performs prefill + navigation)
            .child(
                div()
                    .id("btn-edit-profile")
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(has_selection, |d| d.hover(|s| s.bg(Theme::bg_dark())))
                    .text_size(px(12.0))
                    .text_color(if has_selection {
                        Theme::text_primary()
                    } else {
                        Theme::text_muted()
                    })
                    .child("Edit")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, _cx| {
                            if let Some(id) = this.state.selected_profile_id {
                                tracing::info!("Edit profile clicked: {}", id);
                            }
                            this.edit_selected_profile();
                        }),
                    ),
            )
    }

    /// Render a single MCP row with status and toggle
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_mcp_row(&self, mcp: &McpItem, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let mcp_id = mcp.id;
        let is_selected = self.state.selected_mcp_id == Some(mcp_id);
        let name = mcp.name.clone();
        let enabled = mcp.enabled;
        let status = mcp.status;

        // Status color
        let status_color = match status {
            McpStatus::Running => Theme::success(),
            McpStatus::Stopped => Theme::text_muted(),
            McpStatus::Error => Theme::error(),
        };

        div()
            .id(SharedString::from(format!("mcp-{mcp_id}")))
            .w_full()
            .h(px(28.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .when(is_selected, |d| d.bg(Theme::selection_bg()))
            .when(!is_selected, |d| d.hover(|s| s.bg(Theme::bg_dark())))
            // Status indicator
            .child(
                div()
                    .size(px(8.0))
                    .rounded_full()
                    .bg(status_color)
                    .mr(px(8.0)),
            )
            // Name (left-aligned, truncate from left for long names)
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(if is_selected {
                        Theme::selection_fg()
                    } else {
                        Theme::text_primary()
                    })
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            )
            // Toggle switch
            .child(
                div()
                    .id(SharedString::from(format!("toggle-{mcp_id}")))
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .bg(if enabled {
                        Theme::selection_bg()
                    } else {
                        Theme::bg_dark()
                    })
                    .text_size(px(10.0))
                    .text_color(if enabled {
                        Theme::selection_fg()
                    } else {
                        Theme::text_muted()
                    })
                    .child(if enabled { "ON" } else { "OFF" })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, _cx| {
                            tracing::info!("MCP toggle clicked: {} -> {}", mcp_id, !enabled);
                            this.toggle_mcp(mcp_id, !enabled);
                        }),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    tracing::info!("MCP row selected: {}", mcp_id);
                    this.select_mcp(mcp_id, cx);
                }),
            )
            .into_any_element()
    }

    /// Render the MCP tools section
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_mcp_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let mcps = &self.state.mcps;
        let total_mcps = mcps.len();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            // Section header
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_primary())
                    .child("MCP TOOLS"),
            )
            // List box
            .child(
                div()
                    .id("mcps-list")
                    .w_full()
                    .h(px(100.0))
                    .bg(Theme::bg_darker())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(mcps.iter().map(|m| self.render_mcp_row(m, cx)))
                    .when(mcps.is_empty(), |d| {
                        d.items_center().justify_center().child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::text_muted())
                                .child("No MCP tools configured"),
                        )
                    })
                    .when(total_mcps > 0, |d| {
                        d.child(
                            div()
                                .w_full()
                                .px(px(8.0))
                                .pb(px(2.0))
                                .text_size(px(10.0))
                                .text_color(Theme::text_muted())
                                .child(format!("{total_mcps} MCP tools")),
                        )
                    }),
            )
            // Toolbar: [-] [+] [spacer] [Edit]
            .child(self.render_mcp_toolbar(cx))
    }

    /// MCP section toolbar: [-] [+] [spacer] [Edit]
    fn render_mcp_toolbar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let has_selection = self.state.selected_mcp_id.is_some();

        div()
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.0))
            // [-] Delete button
            .child(
                div()
                    .id("btn-delete-mcp")
                    .size(px(28.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(has_selection, |d| d.hover(|s| s.bg(Theme::danger())))
                    .text_size(px(14.0))
                    .text_color(if has_selection {
                        Theme::text_primary()
                    } else {
                        Theme::text_muted()
                    })
                    .child("-")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, _cx| {
                            if let Some(id) = this.state.selected_mcp_id {
                                tracing::info!("Delete MCP clicked: {}", id);
                            }
                            this.delete_selected_mcp();
                        }),
                    ),
            )
            // [+] Add button
            .child(
                div()
                    .id("btn-add-mcp")
                    .size(px(28.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(14.0))
                    .text_color(Theme::text_primary())
                    .child("+")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            tracing::info!("Add MCP clicked - navigating to McpAdd");
                            Self::navigate_to_mcp_add();
                        }),
                    ),
            )
            // Spacer
            .child(div().flex_1())
            // [Edit] button
            .child(
                div()
                    .id("btn-edit-mcp")
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(has_selection, |d| d.hover(|s| s.bg(Theme::bg_dark())))
                    .text_size(px(12.0))
                    .text_color(if has_selection {
                        Theme::text_primary()
                    } else {
                        Theme::text_muted()
                    })
                    .child("Edit")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, _cx| {
                            if let Some(id) = this.state.selected_mcp_id {
                                tracing::info!("Edit MCP clicked: {}", id);
                            }
                            this.edit_selected_mcp();
                        }),
                    ),
            )
    }

    /// Render a single row in the theme dropdown list.
    fn render_theme_row(
        &self,
        option: &super::ThemeOption,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let slug = option.slug.clone();
        let name = option.name.clone();
        let is_selected = self.state.selected_theme_slug == slug;

        div()
            .id(gpui::SharedString::from(format!("theme-{slug}")))
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
            .text_size(px(12.0))
            .child(name)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    tracing::info!("Theme selected: {}", slug);
                    this.select_theme(slug.clone(), cx);
                }),
            )
            .into_any_element()
    }

    /// Render the theme selection section.
    fn render_theme_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let themes = &self.state.available_themes;
        let selected_name = themes
            .iter()
            .find(|t| t.slug == self.state.selected_theme_slug)
            .map_or_else(
                || self.state.selected_theme_slug.clone(),
                |t| t.name.clone(),
            );

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            // Section header
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_primary())
                    .child("THEME"),
            )
            // Current selection indicator
            .child(
                div()
                    .w_full()
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .child(selected_name),
            )
            // Theme list
            .child(
                div()
                    .id("themes-list")
                    .w_full()
                    .h(px(80.0))
                    .bg(Theme::bg_darker())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(themes.iter().map(|t| self.render_theme_row(t, cx)))
                    .when(themes.is_empty(), |d| {
                        d.items_center().justify_center().child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::text_muted())
                                .child("No themes available"),
                        )
                    }),
            )
    }
}

impl gpui::Focusable for SettingsView {
    fn focus_handle(&self, _cx: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for SettingsView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("settings-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_darkest())
            .track_focus(&self.focus_handle)
            // Invisible canvas for InputHandler registration (IME/diacritics)
            .child(
                canvas(
                    |bounds, _window: &mut gpui::Window, _cx: &mut gpui::App| bounds,
                    {
                        let entity = cx.entity();
                        let focus = self.focus_handle.clone();
                        move |bounds: Bounds<Pixels>,
                              _,
                              window: &mut gpui::Window,
                              cx: &mut gpui::App| {
                            window.handle_input(
                                &focus,
                                ElementInputHandler::new(bounds, entity),
                                cx,
                            );
                        }
                    },
                )
                .size_0(),
            )
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                    this.handle_key_down(event, cx);
                }),
            )
            // Top bar (44px)
            .child(Self::render_top_bar(cx))
            // Content scroll area
            .child(
                div()
                    .id("settings-scroll-area")
                    .flex_1()
                    .w_full()
                    .p(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .overflow_y_scroll()
                    // Profiles section
                    .child(self.render_profiles_section(cx))
                    // MCP Tools section
                    .child(self.render_mcp_section(cx))
                    // Tool approval section
                    .child(self.render_tool_approval_section(cx))
                    // Theme section
                    .child(self.render_theme_section(cx)),
            )
    }
}
