//! Form field sections of the profile editor: name, model, API type,
//! endpoint, and key-label rows.

use super::{ActiveField, ProfileEditorView};
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton};

impl ProfileEditorView {
    pub(super) fn render_name_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::Name);

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("NAME"))
            .child(
                Self::render_text_field(
                    "field-name",
                    &self.state.data.name,
                    "Profile name",
                    active,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.state.active_field = Some(ActiveField::Name);
                        cx.notify();
                    }),
                ),
            )
    }

    /// Render the model field (editable) with browse button
    /// @plan PLAN-20250130-GPUIREDUX.P08
    pub(super) fn render_model_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::Model);
        // REQ-LM-007: the local model id is a free-text label; the registry
        // browse flow has nothing to offer an in-process engine.
        let is_local = self.state.data.api_type == super::ApiType::Local;

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("MODEL"))
            .child(
                div()
                    .w(px(360.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        Self::render_text_field(
                            "field-model-id",
                            &self.state.data.model_id,
                            if is_local {
                                "e.g. granite-4.2-3b"
                            } else {
                                "e.g. claude-sonnet-4-20250514"
                            },
                            active,
                        )
                        .flex_1()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _window, cx| {
                                this.state.active_field = Some(ActiveField::Model);
                                cx.notify();
                            }),
                        ),
                    )
                    .when(!is_local, |row| {
                        row.child(
                            div()
                                .id("btn-browse-model")
                                .debug_selector(|| "btn-browse-model".to_string())
                                .w(px(60.0))
                                .h(px(24.0))
                                .bg(Theme::bg_dark())
                                .border_1()
                                .border_color(Theme::border())
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(Theme::bg_darker()))
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(Theme::text_secondary())
                                .child("Browse")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _window, _cx| {
                                        tracing::info!(
                                            "Browse model clicked - navigating to ModelSelector \
                                             (preserving edit state)"
                                        );
                                        // Do NOT reset state here — preserve `id`, `key_label`,
                                        // `name`, `is_new`, `system_prompt`, etc. so that an
                                        // edit-flow user can swap models without losing their
                                        // work. `ModelSelected` will only update the model
                                        // fields on return. See issue #182.
                                        this.request_api_key_refresh();
                                        crate::ui_gpui::navigation_channel().request_navigate(
                                        crate::presentation::view_command::ViewId::ModelSelector,
                                    );
                                    }),
                                ),
                        )
                    }),
            )
    }

    /// Render API type dropdown
    /// @plan PLAN-20250130-GPUIREDUX.P08
    pub(super) fn render_api_type_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let api_type = self.state.data.api_type.display();

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("API TYPE"))
            .child(
                div()
                    .id("dropdown-api-type")
                    .w(px(360.0))
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.data.api_type = this.state.data.api_type.next();
                            this.state.data.apply_api_type_change();
                            this.request_account_refresh();
                            this.refresh_local_engine_state();
                            cx.notify();
                        }),
                    )
                    .child(api_type)
                    .child(div().text_color(Theme::text_muted()).child("v")),
            )
    }

    /// Render base URL field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    pub(super) fn render_base_url_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        // REQ-LM-007: a local profile has no endpoint, so offering an empty
        // field would invite a value the engine ignores and Save would store.
        if super::ApiType::Local == self.state.data.api_type {
            return div().flex().flex_col();
        }
        let active = self.state.active_field == Some(ActiveField::BaseUrl);

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("BASE URL"))
            .child(
                Self::render_text_field(
                    "field-base-url",
                    &self.state.data.base_url,
                    "https://api.example.com/v1",
                    active,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.state.active_field = Some(ActiveField::BaseUrl);
                        cx.notify();
                    }),
                ),
            )
    }

    /// @plan PLAN-20250130-GPUIREDUX.P08
    /// Render the credential control: an API key label, or the account row for
    /// providers that authenticate with a sign-in. and "Manage Keys" button.
    pub(super) fn render_key_label_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        // Account-authenticated providers show who is signed in instead of a
        // key to pick.
        if self.state.data.api_type.requires_oauth_account() {
            return self.render_account_section(cx);
        }

        // For Local provider, show "No API key required" message instead of key dropdown
        if !self.state.data.api_type.requires_api_key() {
            return div()
                .flex()
                .flex_col()
                .child(Self::render_label("API KEY"))
                .child(
                    div()
                        .w(px(360.0))
                        .h(px(24.0))
                        .px(px(8.0))
                        .bg(Theme::bg_dark())
                        .border_1()
                        .border_color(Theme::border())
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .text_size(px(Theme::font_size_mono()))
                        .text_color(Theme::text_muted())
                        .child("No API key required"),
                )
                .into_any_element();
        }

        let current_label = if self.state.data.key_label.is_empty() {
            "Select API Key…".to_string()
        } else {
            self.state.data.key_label.clone()
        };

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("API KEY"))
            .child(Self::render_key_dropdown_and_manage_button(
                current_label,
                cx,
            ))
            .into_any_element()
    }

    /// Render the key dropdown and manage button for providers that require API keys.
    fn render_key_dropdown_and_manage_button(
        current_label: String,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            // Dropdown cycling through available keys
            .child(
                div()
                    .id("dropdown-key-label")
                    .debug_selector(|| "dropdown-key-label".to_string())
                    .flex_1()
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(if current_label == "Select API Key…" {
                        Theme::text_muted()
                    } else {
                        Theme::text_primary()
                    })
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            if this.state.data.available_keys.is_empty() {
                                this.request_api_key_refresh();
                                cx.notify();
                                return;
                            }
                            let current_idx = this
                                .state
                                .data
                                .available_keys
                                .iter()
                                .position(|k| k == &this.state.data.key_label)
                                .map_or(0, |i| i + 1);
                            let next_idx = current_idx % this.state.data.available_keys.len();
                            this.state.data.key_label =
                                this.state.data.available_keys[next_idx].clone();
                            cx.notify();
                        }),
                    )
                    .child(current_label)
                    .child(div().text_color(Theme::text_muted()).child("▾")),
            )
            // "Manage Keys" button
            .child(
                div()
                    .id("btn-manage-keys")
                    .debug_selector(|| "btn-manage-keys".to_string())
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_darker()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_secondary())
                    .child("Manage Keys")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            crate::ui_gpui::navigation_channel().request_navigate(
                                crate::presentation::view_command::ViewId::ApiKeyManager,
                            );
                        }),
                    ),
            )
    }
}
