//! Transcript-selection content identity and freshness tracking.

use super::emoji::strip_emojis;
use super::state::{ChatMessage, MessageRole, StreamingState};
use super::transcript::{displayed_thinking, TranscriptRow};
use crate::ui_gpui::components::markdown_content::{
    markdown_copy_leaves, parse_markdown_blocks, MarkdownCopyLeaf,
};
use crate::ui_gpui::components::transcript_selection::{
    TranscriptCopyDocument, TranscriptSelectionContext, THINKING_BODY_SEPARATOR,
};
use gpui::{Pixels, Point};
use gpui_selection_vendor::{TextSelection, TextSelectionContentKey};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
struct MessageContentIdentity {
    conversation_id: Option<Uuid>,
    message_index: usize,
    role: MessageRole,
    displayed_content: Arc<String>,
    displayed_thinking: Option<Arc<String>>,
    filter_emoji: bool,
}

#[derive(Clone, Debug)]
struct MessageRevision {
    identity: MessageContentIdentity,
    content_key: TextSelectionContentKey,
}

impl MessageRevision {
    fn new(identity: MessageContentIdentity) -> Self {
        Self {
            identity,
            content_key: next_content_key(),
        }
    }
}

fn next_content_key() -> TextSelectionContentKey {
    static NEXT_CONTENT_KEY: AtomicU64 = AtomicU64::new(1);
    let value = NEXT_CONTENT_KEY
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .expect("transcript selection content keys exhausted");
    TextSelectionContentKey::new(value)
}

/// Chat-state inputs that selection revisions sync against.
///
/// Bundles the transcript snapshot (conversation, messages, streaming)
/// with the display toggles that shape displayed content, so `sync`
/// takes one purposeful value instead of a growing argument list.
#[derive(Clone, Copy)]
pub(super) struct TranscriptSelectionSyncInputs<'a> {
    pub conversation_id: Option<Uuid>,
    pub messages: &'a [ChatMessage],
    pub streaming: &'a StreamingState,
    pub filter_emoji: bool,
    pub show_thinking: bool,
    pub streaming_thinking: Option<&'a str>,
}

#[derive(Debug, Default)]
pub(super) struct TranscriptSelectionRevisions {
    messages: Vec<MessageRevision>,
    streaming: Option<MessageRevision>,
}

impl TranscriptSelectionRevisions {
    pub(super) fn sync(&mut self, inputs: TranscriptSelectionSyncInputs<'_>) {
        let TranscriptSelectionSyncInputs {
            conversation_id,
            messages,
            streaming,
            filter_emoji,
            show_thinking,
            streaming_thinking,
        } = inputs;
        let identities = messages
            .iter()
            .enumerate()
            .map(|(message_index, message)| MessageContentIdentity {
                conversation_id,
                message_index,
                role: message.role.clone(),
                displayed_content: displayed_message_content(message, filter_emoji),
                displayed_thinking: displayed_thinking_arc(
                    message.thinking.as_ref(),
                    show_thinking,
                    filter_emoji,
                ),
                filter_emoji,
            })
            .collect::<Vec<_>>();
        let is_append = self.messages.len() <= identities.len()
            && self
                .messages
                .iter()
                .zip(&identities)
                .all(|(revision, identity)| revision.identity == *identity);
        self.messages = if is_append {
            identities
                .into_iter()
                .enumerate()
                .map(|(index, identity)| {
                    self.messages
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| MessageRevision::new(identity))
                })
                .collect()
        } else {
            identities.into_iter().map(MessageRevision::new).collect()
        };

        let streaming_identity = streaming_identity(
            conversation_id,
            self.messages.len(),
            streaming,
            filter_emoji,
            show_thinking,
            streaming_thinking,
        );
        self.streaming = match (self.streaming.take(), streaming_identity) {
            (Some(revision), Some(identity)) if revision.identity == identity => Some(revision),
            (_, Some(identity)) => Some(MessageRevision::new(identity)),
            (_, None) => None,
        };
    }

    pub(super) fn message_key(&self, index: usize) -> TextSelectionContentKey {
        self.messages[index].content_key
    }

    pub(super) const fn streaming_key(&self) -> TextSelectionContentKey {
        self.streaming
            .as_ref()
            .expect("streaming row requires a streaming content revision")
            .content_key
    }

    pub(super) fn contains(&self, key: TextSelectionContentKey) -> bool {
        self.messages
            .iter()
            .any(|revision| revision.content_key == key)
            || self
                .streaming
                .as_ref()
                .is_some_and(|revision| revision.content_key == key)
    }
}

fn displayed_message_content(message: &ChatMessage, filter_emoji: bool) -> Arc<String> {
    if filter_emoji && message.role == MessageRole::Assistant {
        Arc::new(strip_emojis(&message.content))
    } else {
        Arc::clone(&message.content)
    }
}

/// The displayed thinking a message's selection identity tracks, sharing
/// the Arc when the text is displayed verbatim and reallocating only when
/// the emoji filter changes it, so identity can never desynchronize from
/// what `transcript::displayed_thinking` displays.
fn displayed_thinking_arc(
    thinking: Option<&Arc<String>>,
    show_thinking: bool,
    filter_emoji: bool,
) -> Option<Arc<String>> {
    let text = displayed_thinking(
        thinking.map(|text| text.as_str()),
        show_thinking,
        filter_emoji,
    )?;
    if filter_emoji {
        Some(Arc::new(text.into_owned()))
    } else {
        thinking.map(Arc::clone)
    }
}

/// Prepends displayed thinking text as a message's first copy leaf.
///
/// The first content leaf's separator becomes a blank line so copied
/// thinking and body text never concatenate; copy assembly keeps using
/// the per-message blank-line separation everywhere else.
fn prepend_thinking_leaf(
    mut leaves: Vec<MarkdownCopyLeaf>,
    thinking: Option<&str>,
) -> Vec<MarkdownCopyLeaf> {
    let Some(thinking) = thinking else {
        return leaves;
    };
    if let Some(first) = leaves.first_mut() {
        first.separator_before = THINKING_BODY_SEPARATOR.to_string();
    }
    let mut all = Vec::with_capacity(leaves.len() + 1);
    all.push(MarkdownCopyLeaf {
        text: thinking.to_string(),
        separator_before: String::new(),
    });
    all.append(&mut leaves);
    all
}

fn streaming_identity(
    conversation_id: Option<Uuid>,
    message_index: usize,
    streaming: &StreamingState,
    filter_emoji: bool,
    show_thinking: bool,
    streaming_thinking: Option<&str>,
) -> Option<MessageContentIdentity> {
    let StreamingState::Streaming { content, .. } = streaming else {
        return None;
    };
    Some(MessageContentIdentity {
        conversation_id,
        message_index,
        role: MessageRole::Assistant,
        displayed_content: Arc::new(format!(
            "{}▋",
            if filter_emoji {
                strip_emojis(content)
            } else {
                content.clone()
            }
        )),
        displayed_thinking: displayed_thinking(streaming_thinking, show_thinking, filter_emoji)
            .map(|text| Arc::new(text.into_owned())),
        filter_emoji,
    })
}

impl super::ChatView {
    pub(super) fn refresh_transcript_selection_revisions(&mut self) {
        self.transcript_selection_revisions
            .sync(TranscriptSelectionSyncInputs {
                conversation_id: self.state.active_conversation_id,
                messages: &self.state.messages,
                streaming: &self.state.streaming,
                filter_emoji: self.state.filter_emoji,
                show_thinking: self.state.show_thinking,
                streaming_thinking: self.state.thinking_content.as_deref(),
            });
    }

    pub(super) fn message_selection_content_key(&self, index: usize) -> TextSelectionContentKey {
        self.transcript_selection_revisions.message_key(index)
    }

    pub(super) const fn streaming_selection_content_key(&self) -> TextSelectionContentKey {
        self.transcript_selection_revisions.streaming_key()
    }

    pub(super) fn selection_content_key_is_current(&self, key: TextSelectionContentKey) -> bool {
        self.transcript_selection_revisions.contains(key)
    }

    pub(super) fn transcript_copy_document(
        &self,
        rows: &[TranscriptRow],
    ) -> Arc<TranscriptCopyDocument> {
        let show_thinking = self.state.show_thinking;
        let streaming_thinking = self.state.thinking_content.as_deref();
        let messages = rows
            .iter()
            .filter_map(|row| match *row {
                TranscriptRow::Message(index) => {
                    let message = &self.state.messages[index];
                    let leaves =
                        if self.state.filter_emoji && message.role == MessageRole::Assistant {
                            markdown_copy_leaves(&parse_markdown_blocks(&strip_emojis(
                                &message.content,
                            )))
                        } else {
                            markdown_copy_leaves(&message.get_or_parse_markdown())
                        };
                    let leaves = prepend_thinking_leaf(
                        leaves,
                        displayed_thinking(
                            message.thinking.as_deref().map(String::as_str),
                            show_thinking,
                            self.state.filter_emoji,
                        )
                        .as_deref(),
                    );
                    Some((self.message_selection_content_key(index), leaves))
                }
                TranscriptRow::Streaming => {
                    let StreamingState::Streaming { content, .. } = &self.state.streaming else {
                        return None;
                    };
                    let content = if self.state.filter_emoji {
                        strip_emojis(content)
                    } else {
                        content.clone()
                    };
                    let leaves = prepend_thinking_leaf(
                        markdown_copy_leaves(&parse_markdown_blocks(&format!("{content}▋"))),
                        displayed_thinking(
                            streaming_thinking,
                            show_thinking,
                            self.state.filter_emoji,
                        )
                        .as_deref(),
                    );
                    Some((self.streaming_selection_content_key(), leaves))
                }
                TranscriptRow::Approval(_) => None,
            })
            .collect();
        Arc::new(TranscriptCopyDocument::new(messages))
    }

    pub(super) fn transcript_selection_context(
        &self,
        row: TranscriptRow,
        scroll_offset: Point<Pixels>,
        document_order: u64,
        copy_document: Arc<TranscriptCopyDocument>,
    ) -> Option<TranscriptSelectionContext> {
        let content_key = match row {
            TranscriptRow::Message(index) => self.message_selection_content_key(index),
            TranscriptRow::Streaming => self.streaming_selection_content_key(),
            TranscriptRow::Approval(_) => return None,
        };
        Some(TranscriptSelectionContext {
            scroll_offset,
            document_order,
            first_copy_separator: if document_order == 0 {
                ""
            } else {
                THINKING_BODY_SEPARATOR
            },
            content_key,
            copy_document,
        })
    }

    pub(super) fn copy_selection_or_input(
        &self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if let Some(keys) = TextSelection::selected_content_keys(window, cx) {
            if keys
                .iter()
                .any(|key| !self.selection_content_key_is_current(*key))
            {
                TextSelection::clear(window, cx);
                return;
            }
            let selected_text = TextSelection::selected_text(window, cx);
            if !selected_text.is_empty() {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(selected_text));
            }
            return;
        }
        if TextSelection::has_selection(window, cx) {
            TextSelection::clear(window, cx);
            return;
        }
        let text = if self.sidebar_search_focused(cx) {
            self.state.sidebar_search_query.clone()
        } else if self.state.conversation_title_editing {
            self.state.conversation_title_input.clone()
        } else {
            self.state.input_text.clone()
        };
        if !text.is_empty() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    /// Installs a logical whole-transcript selection.
    ///
    /// The copy text is frozen from the current copy document, while visible
    /// leaves rejoin by content key as virtualization mounts them. An empty
    /// or blank-only transcript clears any selection instead.
    pub(super) fn select_all_transcript(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.refresh_transcript_selection_revisions();
        let rows = self.transcript_rows();
        let payload = self.transcript_copy_document(&rows).select_all_payload();
        self.stop_selection_auto_scroll();
        match payload {
            Some((keys, text)) => TextSelection::select_all(&keys, &text, window, cx),
            None => TextSelection::clear(window, cx),
        }
        cx.notify();
    }

    /// Clears the engine selection and stops the host auto-scroll loop.
    ///
    /// Returns whether a selection existed before the call.
    pub(super) fn clear_transcript_selection(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let had_selection = TextSelection::has_selection(window, cx);
        if had_selection {
            TextSelection::clear(window, cx);
        }
        self.stop_selection_auto_scroll();
        had_selection
    }

    /// Handles select-all with editor and overlay precedence.
    ///
    /// Sidebar search ignores select-all, and an open title edit or dropdown
    /// keeps the composer-style cursor move. Otherwise an existing window
    /// selection owns the command and expands to the whole transcript, and
    /// with no selection the composer keeps its cursor-to-end behavior.
    pub(super) fn select_all_for_focused_surface(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.sidebar_search_focused(cx) {
            // select-all is a no-op for sidebar search (single-line)
        } else if self.state.conversation_title_editing
            || self.state.conversation_dropdown_open
            || self.state.profile_dropdown_open
        {
            self.handle_select_all(cx);
        } else if TextSelection::has_selection(window, cx) {
            self.select_all_transcript(window, cx);
        } else {
            self.handle_select_all(cx);
        }
    }

    /// Handles Escape after editor and overlay cancel branches.
    ///
    /// A transcript selection clears (and stops its auto-scroll loop) without
    /// stopping the stream; with no selection the streaming-stop behavior is
    /// preserved.
    pub(super) fn handle_escape_key(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.clear_transcript_selection(window, cx) {
            cx.notify();
            return;
        }
        if matches!(self.state.streaming, StreamingState::Streaming { .. }) {
            // @plan PLAN-20260416-ISSUE173.P14-CR7
            // @requirement REQ-173-002.3
            // Emit the StopStreaming event only when we have a
            // conversation id to target, but always reset the local
            // composer state so the UI exits Stop mode even if we
            // have no active conversation id (the event and the
            // local composer state are independent concerns).
            if let Some(conversation_id) = self.state.active_conversation_id {
                self.emit(crate::events::types::UserEvent::StopStreaming { conversation_id });
            }
            self.state.streaming = StreamingState::Idle;
            self.refresh_transcript_selection_revisions();
            // Refocus so keyboard input works after stopping.
            self.focus_composer(cx);
            cx.notify();
        }
    }

    /// Whether a key event with these modifiers routes as a platform shortcut.
    ///
    /// The `platform` modifier (Command on macOS, Windows key on Windows,
    /// Super on Linux) is distinct from Control. On non-macOS the Ctrl
    /// branch decides first whenever Control is held: only an exact plain
    /// Ctrl+A and Ctrl+C route, so Control combined with platform, shift,
    /// alt, or function keeps its own meaning instead of selecting or
    /// copying, and unmodified navigation keys keep their plain bindings.
    /// Events without Control fall back to the platform modifier, which on
    /// macOS preserves the existing Command routing unchanged.
    pub(super) const fn routes_platform_shortcut(
        modifiers: gpui::Modifiers,
        key: &str,
        non_macos: bool,
    ) -> bool {
        if non_macos && modifiers.control {
            matches!(
                modifiers,
                gpui::Modifiers {
                    control: true,
                    shift: false,
                    alt: false,
                    platform: false,
                    function: false,
                }
            ) && (key.eq_ignore_ascii_case("a") || key.eq_ignore_ascii_case("c"))
        } else {
            modifiers.platform
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_gpui::views::chat_view::state::{ChatMessage, StreamingState};
    use uuid::Uuid;

    fn transcript() -> Vec<ChatMessage> {
        vec![
            ChatMessage::user("first"),
            ChatMessage::assistant("second", "model"),
            ChatMessage::user("third"),
        ]
    }

    #[test]
    fn selection_does_not_survive_conversation_switch() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let first = Uuid::new_v4();
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(first),
            messages: &transcript(),
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });
        let selected = revisions.message_key(1);

        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(Uuid::new_v4()),
            messages: &transcript(),
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });

        assert!(!revisions.contains(selected));
    }

    #[test]
    fn selection_does_not_survive_emoji_filter_toggle() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let conversation = Uuid::new_v4();
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &[ChatMessage::assistant("hello 😀", "model")],
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });
        let selected = revisions.message_key(0);

        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &[ChatMessage::assistant(
                concat!("hello ", "\u{1F600}"),
                "model",
            )],
            streaming: &StreamingState::Idle,
            filter_emoji: true,
            show_thinking: false,
            streaming_thinking: None,
        });

        assert!(!revisions.contains(selected));
    }

    #[test]
    fn selection_does_not_survive_deleting_an_earlier_message() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let conversation = Uuid::new_v4();
        let mut messages = transcript();
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });
        let selected = revisions.message_key(2);

        messages.remove(0);
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });

        assert!(!revisions.contains(selected));
    }

    #[test]
    fn different_conversations_never_share_a_content_key() {
        let mut revisions = TranscriptSelectionRevisions::default();
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(Uuid::new_v4()),
            messages: &transcript(),
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });
        let first = revisions.message_key(0);

        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(Uuid::new_v4()),
            messages: &transcript(),
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });

        assert_ne!(first, revisions.message_key(0));
    }

    #[test]
    fn streaming_append_preserves_selection_in_an_earlier_message() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let conversation = Uuid::new_v4();
        let messages = transcript();
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Streaming {
                content: "partial".to_string(),
                done: false,
            },
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });
        let selected = revisions.message_key(0);
        let streaming = revisions.streaming_key();

        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Streaming {
                content: "partial response".to_string(),
                done: false,
            },
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });

        assert!(revisions.contains(selected));
        assert!(!revisions.contains(streaming));
    }

    #[test]
    fn appended_message_during_drag_preserves_existing_endpoint_keys() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let conversation = Uuid::new_v4();
        let mut messages = transcript();
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });
        let anchor = revisions.message_key(0);
        let cursor = revisions.message_key(2);

        messages.push(ChatMessage::assistant("new arrival", "model"));
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });

        assert!(revisions.contains(anchor));
        assert!(revisions.contains(cursor));
    }

    #[test]
    fn selection_does_not_survive_thinking_visibility_toggle() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let conversation = Uuid::new_v4();
        let messages = vec![ChatMessage::assistant("answer", "model").with_thinking("reasoning")];
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: true,
            streaming_thinking: None,
        });
        let selected = revisions.message_key(0);

        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });

        assert!(!revisions.contains(selected));
    }

    #[test]
    fn selection_survives_toggle_when_no_thinking_is_displayed() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let conversation = Uuid::new_v4();
        let messages = vec![ChatMessage::user("question")];
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: true,
            streaming_thinking: None,
        });
        let selected = revisions.message_key(0);

        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });

        assert!(revisions.contains(selected));
    }

    #[test]
    fn selection_does_not_survive_displayed_thinking_text_change() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let conversation = Uuid::new_v4();
        let messages = vec![ChatMessage::assistant("answer", "model").with_thinking("before")];
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: true,
            streaming_thinking: None,
        });
        let selected = revisions.message_key(0);

        let edited = vec![ChatMessage::assistant("answer", "model").with_thinking("after")];
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &edited,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: true,
            streaming_thinking: None,
        });

        assert!(!revisions.contains(selected));
    }

    #[test]
    fn empty_thinking_text_is_not_displayed_identity() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let conversation = Uuid::new_v4();
        let messages = vec![ChatMessage::assistant("answer", "model").with_thinking("   ")];
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: None,
        });
        let selected = revisions.message_key(0);

        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Idle,
            filter_emoji: false,
            show_thinking: true,
            streaming_thinking: None,
        });

        assert!(revisions.contains(selected));
    }

    #[test]
    fn streaming_thinking_change_invalidates_the_streaming_key_only() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let conversation = Uuid::new_v4();
        let messages = transcript();
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Streaming {
                content: "partial".to_string(),
                done: false,
            },
            filter_emoji: false,
            show_thinking: true,
            streaming_thinking: Some("initial thought"),
        });
        let earlier_message = revisions.message_key(0);
        let streaming = revisions.streaming_key();

        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &StreamingState::Streaming {
                content: "partial".to_string(),
                done: false,
            },
            filter_emoji: false,
            show_thinking: true,
            streaming_thinking: Some("extended thinking"),
        });

        assert!(revisions.contains(earlier_message));
        assert!(!revisions.contains(streaming));
    }

    #[test]
    fn streaming_thinking_visibility_toggle_invalidates_the_streaming_key() {
        let mut revisions = TranscriptSelectionRevisions::default();
        let conversation = Uuid::new_v4();
        let messages = transcript();
        let streaming = StreamingState::Streaming {
            content: "partial".to_string(),
            done: false,
        };
        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &streaming,
            filter_emoji: false,
            show_thinking: true,
            streaming_thinking: Some("visible thought"),
        });
        let shown = revisions.streaming_key();

        revisions.sync(TranscriptSelectionSyncInputs {
            conversation_id: Some(conversation),
            messages: &messages,
            streaming: &streaming,
            filter_emoji: false,
            show_thinking: false,
            streaming_thinking: Some("visible thought"),
        });

        assert!(!revisions.contains(shown));
    }
}
