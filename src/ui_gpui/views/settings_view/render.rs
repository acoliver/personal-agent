//! Render implementation for `SettingsView`.

use super::{McpItem, McpStatus, ProfileItem, SettingsCategory, SettingsView};
use crate::events::types::UserEvent;
use crate::presentation::view_command::AppMode;
use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::views::main_panel::MainPanelAppState;
use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FontWeight, MouseButton, Pixels,
    SharedString,
};

impl SettingsView {
    /// Render the top bar with title
    fn render_top_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let is_popout = cx
            .try_global::<MainPanelAppState>()
            .is_some_and(|s| s.app_mode == AppMode::Popout);

        div()
            .id("settings-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .pr(px(12.0))
            .pl(px(if is_popout { 72.0 } else { 12.0 }))
            .flex()
            .items_center()
            .child(
                div()
                    .text_size(px(Theme::font_size_body()))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("Settings"),
            )
    }

    /// Render the bottom bar with back button in the lower-left corner
    fn render_bottom_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("settings-bottom-bar")
            .h(px(36.0))
            .w_full()
            .flex_shrink_0()
            .bg(Theme::bg_darker())
            .border_t_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .child(
                div()
                    .id("btn-back")
                    .h(px(28.0))
                    .px(px(8.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(Theme::font_size_body()))
                    .text_color(Theme::text_secondary())
                    .child("\u{2039} Back")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            tracing::info!("Back clicked - navigating to Chat");
                            crate::ui_gpui::navigation_channel()
                                .request_navigate(crate::presentation::view_command::ViewId::Chat);
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
            .text_size(px(Theme::font_size_mono()))
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

    /// Render the profiles list and toolbar (no header — caller provides it).
    /// The list is constrained to available space with overflow scrolling,
    /// ensuring the +/- toolbar buttons remain visible at all times.
    fn render_profiles_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let profiles = &self.state.profiles;
        let total_profiles = profiles.len();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0)) // Critical: allows flex child to shrink below content size
            .gap(px(6.0))
            .child(
                div()
                    .id("profiles-list")
                    .w_full()
                    .flex_1()
                    .min_h(px(0.0)) // Critical: allows scrolling container to shrink
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
                                .text_size(px(Theme::font_size_mono()))
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
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(Theme::text_muted())
                                .child(format!("{total_profiles} profiles")),
                        )
                    }),
            )
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
                    .text_size(px(Theme::font_size_body()))
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
                    .text_size(px(Theme::font_size_body()))
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
                    .text_size(px(Theme::font_size_mono()))
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
                    .text_size(px(Theme::font_size_mono()))
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
                    .text_size(px(Theme::font_size_ui()))
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

    /// Render the MCP tools section with full-height list.
    /// The list is constrained to available space with overflow scrolling,
    /// ensuring the +/- toolbar buttons remain visible at all times.
    fn render_mcp_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let mcps = &self.state.mcps;
        let total_mcps = mcps.len();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0)) // Critical: allows flex child to shrink below content size
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("MCP TOOLS"),
            )
            .child(
                div()
                    .id("mcps-list")
                    .w_full()
                    .flex_1()
                    .min_h(px(0.0)) // Critical: allows scrolling container to shrink
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
                                .text_size(px(Theme::font_size_mono()))
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
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(Theme::text_muted())
                                .child(format!("{total_mcps} MCP tools")),
                        )
                    }),
            )
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
                    .text_size(px(Theme::font_size_body()))
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
                    .text_size(px(Theme::font_size_body()))
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
                    .text_size(px(Theme::font_size_mono()))
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

    /// Render the export directory setting section.
    fn render_export_dir_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let is_active = self.state.active_field == Some(super::ActiveField::ExportDirInput);
        let input_text = if self.state.export_dir_input.is_empty() {
            "System Downloads (default)".to_string()
        } else {
            self.state.export_dir_input.clone()
        };
        let text_color = if self.state.export_dir_input.is_empty() {
            Theme::text_muted()
        } else {
            Theme::text_primary()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("EXPORT DIRECTORY"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .id("export-dir-input")
                            .flex_1()
                            .min_w(px(0.0))
                            .h(px(28.0))
                            .px(px(8.0))
                            .bg(Theme::bg_darker())
                            .border_1()
                            .border_color(if is_active {
                                Theme::accent()
                            } else {
                                Theme::border()
                            })
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .cursor_text()
                            .text_size(px(Theme::font_size_mono()))
                            .text_color(text_color)
                            .child(input_text)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    window.focus(&this.focus_handle, cx);
                                    this.set_active_field(Some(super::ActiveField::ExportDirInput));
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-browse-export-dir")
                            .h(px(28.0))
                            .px(px(10.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::accent()))
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child("Browse…")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.browse_export_directory(cx);
                                }),
                            ),
                    ),
            )
            .child(self.render_export_dir_toolbar(cx))
    }

    /// Toolbar row for the export directory section: [Save] [Reset] + help text.
    #[allow(clippy::unused_self)] // cx.listener borrows the entity, not &self directly
    fn render_export_dir_toolbar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .id("btn-save-export-dir")
                    .px(px(12.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("Save")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.save_export_directory();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .id("btn-reset-export-dir")
                    .px(px(12.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_secondary())
                    .child("Reset")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.export_dir_input.clear();
                            this.save_export_directory();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child("Enter a directory path, or reset for system Downloads"),
            )
    }

    /// Render the category sidebar (120px, left side).
    fn render_category_sidebar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let selected = self.state.selected_category;

        div()
            .id("settings-sidebar")
            .w(px(120.0))
            .h_full()
            .bg(Theme::bg_darkest())
            .border_r_1()
            .border_color(Theme::border())
            .flex()
            .flex_col()
            .children(SettingsCategory::ALL.iter().map(|&cat| {
                let is_active = cat == selected;
                div()
                    .id(SharedString::from(format!(
                        "cat-{}",
                        cat.display_name().to_lowercase().replace(' ', "-")
                    )))
                    .w_full()
                    .py(px(8.0))
                    .px(px(12.0))
                    .cursor_pointer()
                    .border_l_2()
                    .when(is_active, |d| {
                        d.border_color(Theme::accent())
                            .bg(Theme::bg_dark())
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!is_active, |d| {
                        d.border_color(gpui::transparent_black())
                            .hover(|s| s.bg(Theme::bg_dark()))
                    })
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .child(cat.display_name())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            this.select_category(cat);
                            cx.notify();
                        }),
                    )
                    .into_any_element()
            }))
    }

    /// Dispatch to the appropriate category panel renderer.
    fn render_content_panel(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let panel: gpui::AnyElement = match self.state.selected_category {
            SettingsCategory::General => self.render_general_panel(cx).into_any_element(),
            SettingsCategory::Appearance => self.render_appearance_panel(cx).into_any_element(),
            SettingsCategory::Models => self.render_models_panel(cx).into_any_element(),
            SettingsCategory::Skills => self.render_skills_panel(cx).into_any_element(),
            SettingsCategory::Security => self.render_security_panel(cx).into_any_element(),
            SettingsCategory::McpTools => self.render_mcp_tools_panel(cx).into_any_element(),
            SettingsCategory::Backup => self.render_backup_panel(cx).into_any_element(),
        };

        div()
            .id("settings-content-panel")
            .flex_1()
            .h_full()
            .p(px(12.0))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(panel)
    }

    /// General panel: export directory and emoji filter toggle.
    fn render_general_panel(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(self.render_export_dir_section(cx))
            .child(self.render_emoji_filter_section(cx))
    }

    /// Emoji filter toggle section.
    fn render_emoji_filter_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let filter_emoji = self.state.filter_emoji;

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("EMOJI FILTER"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.toggle_emoji_filter(cx);
                        }),
                    )
                    .child(Self::render_emoji_filter_indicator(filter_emoji))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_mono()))
                            .text_color(Theme::text_primary())
                            .child("Filter emojis from assistant messages"),
                    ),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child("When enabled, emojis are stripped from AI responses for a cleaner display."),
            )
    }

    /// Indicator for emoji filter toggle.
    fn render_emoji_filter_indicator(filter_emoji: bool) -> impl IntoElement {
        div()
            .size(px(14.0))
            .rounded(px(2.0))
            .border_1()
            .border_color(Theme::border())
            .bg(if filter_emoji {
                Theme::accent()
            } else {
                Theme::bg_dark()
            })
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .when(filter_emoji, |d| d.child("\u{2713}"))
    }

    /// Models panel: full-height profiles list + Refresh Models button.
    fn render_models_panel(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(6.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child("PROFILES"),
                    )
                    .child(
                        div()
                            .id("btn-refresh-models")
                            .px(px(12.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child("Refresh Models")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, _cx| {
                                    this.emit(&UserEvent::RefreshModelsRegistry);
                                }),
                            ),
                    ),
            )
            .child(self.render_profiles_section(cx))
    }

    /// Security panel: renders tool approval controls, including skills auto-approve.
    fn render_security_panel(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("security-panel-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll()
            .gap(px(16.0))
            .child(self.render_tool_approval_section(cx))
    }

    /// MCP Tools panel: full-height MCP server list.
    fn render_mcp_tools_panel(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        self.render_mcp_section(cx)
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
            .when_some(Theme::ui_font_family(), |div, family| {
                div.font_family(family)
            })
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
            // Body: sidebar + content panel
            .child(
                div()
                    .id("settings-body")
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(self.render_category_sidebar(cx))
                    .child(self.render_content_panel(cx)),
            )
            // Bottom bar with back button
            .child(Self::render_bottom_bar(cx))
    }
}
