use super::state::{ChatMessage, MessageRole};
use super::{ActiveMessageSelection, ChatView, MessageContextMenu};
use crate::ui_gpui::components::markdown_content::{
    parse_markdown_blocks,
    selectable_markdown::{SelectableMarkdown, SelectableMarkdownEvent},
    visible_document::{MessageRevision, VisibleDocument},
};
use crate::ui_gpui::components::AssistantBubble;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton};
use std::sync::Arc;

impl ChatView {
    fn current_conversation_identity(&self) -> String {
        self.state
            .active_conversation_id
            .map_or_else(|| "new".to_string(), |id| id.to_string())
    }

    pub(super) fn fresh_active_message_selected_text(&self) -> Option<String> {
        let active = self.active_message_selection.as_ref()?;
        let message = self.state.messages.get(active.message_index)?;
        let display_content = if self.state.filter_emoji {
            super::render::strip_emojis(&message.content)
        } else {
            (*message.content).clone()
        };
        let identity = format!(
            "{}:{}:{:?}",
            self.current_conversation_identity(),
            active.message_index,
            message.role
        );
        let revision =
            MessageRevision::new(&identity, &display_content, 0, self.state.filter_emoji);
        if active.revision != revision {
            return None;
        }
        let document = if self.state.filter_emoji {
            VisibleDocument::from_blocks(&parse_markdown_blocks(&display_content))
        } else {
            (*message.get_or_build_visible_document()).clone()
        };
        let selected_text = document.selected_text(&active.selection);
        (!selected_text.is_empty()).then_some(selected_text)
    }

    pub(super) fn apply_selectable_message_event(
        &mut self,
        message_index: usize,
        revision: MessageRevision,
        event: SelectableMarkdownEvent,
    ) {
        match event {
            SelectableMarkdownEvent::SelectionChanged {
                selection: Some(selection),
                selected_text: _,
                dragging,
            } if !selection.is_empty() => {
                self.active_message_selection = Some(ActiveMessageSelection {
                    message_index,
                    revision,
                    selection,
                    dragging,
                });
                self.message_context_menu = None;
            }
            SelectableMarkdownEvent::SelectionChanged { .. } => {
                self.active_message_selection = None;
                self.message_context_menu = None;
            }
            SelectableMarkdownEvent::ContextMenu {
                position,
                selected_text,
            } => {
                self.message_context_menu = Some(MessageContextMenu {
                    position,
                    selected_text,
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn render_message(
        msg: &ChatMessage,
        message_index: usize,
        conversation_identity: &str,
        show_thinking: bool,
        filter_emoji: bool,
        active: Option<&ActiveMessageSelection>,
        view_entity: gpui::Entity<Self>,
    ) -> gpui::AnyElement {
        let display_content = if filter_emoji {
            super::render::strip_emojis(&msg.content)
        } else {
            (*msg.content).clone()
        };
        let (blocks, document) = if filter_emoji {
            let blocks = Arc::new(parse_markdown_blocks(&display_content));
            let document = Arc::new(VisibleDocument::from_blocks(&blocks));
            (blocks, document)
        } else {
            (
                msg.get_or_parse_markdown(),
                msg.get_or_build_visible_document(),
            )
        };
        let identity = format!("{conversation_identity}:{message_index}:{:?}", msg.role);
        let revision = MessageRevision::new(&identity, &display_content, 0, filter_emoji);
        let selection = active
            .filter(|active| active.message_index == message_index && active.revision == revision)
            .map(|active| active.selection.clone());
        let callback_revision = revision.clone();
        let element_identity = gpui::SharedString::from(format!(
            "selectable-message:{identity}:{}",
            callback_revision.hash()
        ));
        let selectable = SelectableMarkdown::from_cached_blocks(
            blocks,
            document,
            revision,
            match msg.role {
                MessageRole::User => Theme::user_bubble_text(),
                MessageRole::Assistant => Theme::text_primary(),
            },
        )
        .id((element_identity, message_index))
        .with_selection(selection)
        .with_dragging(active.is_some_and(|active| {
            active.message_index == message_index
                && active.revision == callback_revision
                && active.dragging
        }))
        .on_event(move |event, _window, cx| {
            let revision = callback_revision.clone();
            view_entity.update(cx, |view, cx| {
                view.apply_selectable_message_event(message_index, revision, event);
                cx.notify();
            });
        });

        match msg.role {
            MessageRole::User => div()
                .w_full()
                .flex()
                .justify_end()
                .child(Theme::user_bubble(
                    div()
                        .max_w(px(300.0))
                        .px(px(10.0))
                        .py(px(10.0))
                        .rounded(px(12.0))
                        .text_size(px(Theme::font_size_mono()))
                        .cursor_text()
                        .child(selectable),
                ))
                .into_any_element(),
            MessageRole::Assistant => {
                let mut bubble = AssistantBubble::new(display_content)
                    .selectable_content(selectable)
                    .model_id(
                        msg.model_label
                            .clone()
                            .unwrap_or_else(|| "Assistant".to_string()),
                    );
                if show_thinking {
                    if let Some(thinking) = &msg.thinking {
                        bubble = bubble.thinking((**thinking).clone()).show_thinking(true);
                    }
                }
                bubble.into_any_element()
            }
        }
    }

    pub(super) fn render_message_context_menu(
        &self,
        window: &gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let menu = self
            .message_context_menu
            .as_ref()
            .expect("menu state checked");
        let viewport = window.viewport_size();
        let left = menu.position.x.min(viewport.width - px(168.0)).max(px(8.0));
        let top = menu.position.y.min(viewport.height - px(52.0)).max(px(8.0));
        let selected_text = menu.selected_text.clone();
        let shortcut = if cfg!(target_os = "macos") {
            "⌘C"
        } else {
            "Ctrl+C"
        };

        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.message_context_menu = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("message-selection-context-menu")
                    .absolute()
                    .left(left)
                    .top(top)
                    .w(px(160.0))
                    .p(px(4.0))
                    .rounded(px(Theme::RADIUS_MD))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| cx.stop_propagation()),
                    )
                    .child(
                        div()
                            .id("message-selection-copy")
                            .flex()
                            .justify_between()
                            .px(px(8.0))
                            .py(px(6.0))
                            .rounded(px(Theme::RADIUS_SM))
                            .cursor_pointer()
                            .hover(|style| style.bg(Theme::bg_darker()))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        selected_text.clone(),
                                    ));
                                    view.message_context_menu = None;
                                    cx.notify();
                                }),
                            )
                            .child("Copy")
                            .child(div().text_color(Theme::text_muted()).child(shortcut)),
                    ),
            )
            .into_any_element()
    }
}
