//! Applying an `AppStore` snapshot to the chat view.
//!
//! This is the one place the view adopts authoritative state from the store: selection,
//! transcript, streaming projection, drafts and autoscroll.

use uuid::Uuid;

use super::state::{ChatMessage, StreamingState};
use super::{save_draft, take_draft, ChatView};
use crate::presentation::view_command::ConversationMessagePayload;
use crate::ui_gpui::app_store::{ChatStoreSnapshot, ConversationLoadState, StreamingStoreSnapshot};

impl ChatView {
    const fn reset_autoscroll_if_needed(&mut self, should_reset_autoscroll: bool) {
        if should_reset_autoscroll {
            self.state.chat_autoscroll_enabled = true;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn scroll_after_loaded_messages_if_needed(
        &self,
        previous_conversation_id: Option<Uuid>,
        selected_conversation_id: Option<Uuid>,
        previous_selection_generation: u64,
        previous_messages_empty: bool,
        was_streaming: bool,
        streaming: &StreamingStoreSnapshot,
        should_reset_autoscroll: bool,
    ) {
        let loaded_messages_scroll = previous_conversation_id == selected_conversation_id
            && previous_selection_generation == self.selection_generation
            && previous_messages_empty
            && !self.state.messages.is_empty()
            && !was_streaming
            && streaming.active_target.is_none()
            && streaming.stream_buffer.is_empty()
            && streaming.thinking_buffer.is_empty();

        if self.state.chat_autoscroll_enabled && (should_reset_autoscroll || loaded_messages_scroll)
        {
            self.maybe_scroll_chat_to_bottom();
        }
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @plan PLAN-20260407-ISSUE172.P07 (cache priming)
    pub(super) fn messages_from_payload(
        messages: Vec<ConversationMessagePayload>,
    ) -> Vec<ChatMessage> {
        messages
            .into_iter()
            .map(|message| {
                let mut chat_message = match message.role {
                    crate::presentation::view_command::MessageRole::User => {
                        ChatMessage::user(message.content)
                    }
                    crate::presentation::view_command::MessageRole::Assistant => {
                        // Use the per-message model_id if available, otherwise show 'unknown'
                        // to avoid misleading users about which model generated old responses
                        let model_label = message.model_id.unwrap_or_else(|| "unknown".to_string());
                        ChatMessage::assistant(message.content, model_label)
                    }
                    crate::presentation::view_command::MessageRole::System
                    | crate::presentation::view_command::MessageRole::Tool => {
                        unreachable!(
                            "conversation replay payload excludes non-rendered message roles"
                        )
                    }
                };

                if let Some(thinking) = message.thinking_content {
                    chat_message = chat_message.with_thinking(thinking);
                }
                if let Some(timestamp) = message.timestamp {
                    chat_message = chat_message.with_timestamp(timestamp);
                }

                // Prime the markdown cache on the original message so that
                // clones produced during render share the cached Arc.
                let _ = chat_message.get_or_parse_markdown();

                chat_message
            })
            .collect()
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    pub(super) fn streaming_state_from_snapshot(
        streaming: &StreamingStoreSnapshot,
        load_state: &ConversationLoadState,
    ) -> StreamingState {
        if let Some(error) = &streaming.last_error {
            return StreamingState::Error(error.clone());
        }

        if let ConversationLoadState::Error { message, .. } = load_state {
            return StreamingState::Error(message.clone());
        }

        if streaming.active_target.is_some() || !streaming.stream_buffer.is_empty() {
            return StreamingState::Streaming {
                content: streaming.stream_buffer.clone(),
                done: false,
            };
        }

        StreamingState::Idle
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P04
    /// @requirement REQ-ARCH-001.1
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:022-035
    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    pub fn apply_store_snapshot(
        &mut self,
        snapshot: ChatStoreSnapshot,
        cx: &mut gpui::Context<Self>,
    ) {
        let ChatStoreSnapshot {
            selected_conversation_id,
            selected_conversation_title,
            load_state,
            transcript,
            streaming,
            conversations,
            ..
        } = snapshot;

        let previous_conversation_id = self.conversation_id;
        let previous_selection_generation = self.selection_generation;
        let previous_messages_empty = self.state.messages.is_empty();

        self.carry_draft_across_selection(previous_conversation_id, selected_conversation_id);
        self.drop_queued_steering_across_selection(
            previous_conversation_id,
            selected_conversation_id,
        );

        self.state.conversations = conversations;
        self.state.active_conversation_id = selected_conversation_id;
        self.conversation_id = selected_conversation_id;
        self.state.conversation_title = selected_conversation_title;

        let should_reset_autoscroll = self.adopt_selection_generation(
            previous_conversation_id,
            previous_selection_generation,
            &load_state,
        );

        self.reset_autoscroll_if_needed(should_reset_autoscroll);

        let was_streaming = matches!(self.state.streaming, StreamingState::Streaming { .. });

        // The store now guarantees that `snapshot.chat.transcript` is always
        // scoped to the currently selected conversation: it is cleared on
        // selection change in `begin_selection_locked` and repopulated by
        // `reduce_messages_loaded`. Mirror it unconditionally so we never
        // render the previous conversation's messages during a
        // selection -> Loading -> Ready transition.
        self.state.messages = Self::messages_from_payload(transcript);
        self.scroll_after_loaded_messages_if_needed(
            previous_conversation_id,
            selected_conversation_id,
            previous_selection_generation,
            previous_messages_empty,
            was_streaming,
            &streaming,
            should_reset_autoscroll,
        );

        let was_thinking = self
            .state
            .thinking_content
            .as_ref()
            .is_some_and(|content| !content.is_empty());
        self.state.streaming = Self::streaming_state_from_snapshot(&streaming, &load_state);
        // show_thinking is view-local and sticky — do NOT overwrite from store snapshot
        let has_thinking = !streaming.thinking_buffer.is_empty();
        self.state.thinking_content = has_thinking.then_some(streaming.thinking_buffer);
        self.state.sync_conversation_dropdown_index();

        if !should_reset_autoscroll
            && (was_streaming
                || was_thinking
                || has_thinking
                || matches!(self.state.streaming, StreamingState::Streaming { .. }))
        {
            self.maybe_scroll_chat_to_bottom();
        }

        self.sync_conversation_list_state(cx);
        self.refresh_transcript_selection_revisions();

        cx.notify();
    }

    /// Keep composer drafts attached to their conversation across a selection change.
    ///
    /// The outgoing conversation's text is stashed and the incoming one's is restored (or
    /// the composer cleared when it has none), so a draft survives popup close/reopen
    /// cycles without leaking into another conversation. When the selection has not changed the
    /// stash is refreshed anyway, because the popup can be closed by a direct tray toggle
    /// without a further snapshot.
    fn carry_draft_across_selection(
        &mut self,
        previous_conversation_id: Option<Uuid>,
        selected_conversation_id: Option<Uuid>,
    ) {
        if previous_conversation_id == selected_conversation_id {
            if let Some(conv_id) = selected_conversation_id {
                save_draft(conv_id, &self.state.input_text);
            }
            return;
        }

        if let Some(prev_id) = previous_conversation_id {
            save_draft(prev_id, &self.state.input_text);
        }
        if let Some(new_id) = selected_conversation_id {
            // Unconditional: a conversation with no stashed draft must show an empty
            // composer, otherwise the outgoing conversation's text stays visible and the
            // user can send it into the wrong conversation.
            self.state.input_text = take_draft(new_id).unwrap_or_default();
            self.state.cursor_position = self.state.input_text.len();
        }
    }

    /// Drop queued steering entries when the selection moves.
    ///
    /// `queued_steering` describes the transcript on screen, so it cannot
    /// follow the user into another conversation the way a draft does: the
    /// entries belong to a turn that is still running somewhere else, and
    /// they come back with that conversation's own `SteeringQueued` events
    /// only if the service still holds them.
    ///
    /// @plan PLAN-20260903-ISSUE222.P04
    /// @requirement REQ-222-003
    fn drop_queued_steering_across_selection(
        &mut self,
        previous_conversation_id: Option<Uuid>,
        selected_conversation_id: Option<Uuid>,
    ) {
        if previous_conversation_id != selected_conversation_id {
            self.state.queued_steering.clear();
        }
    }

    /// Adopt the generation carried by `load_state`, reporting whether the view is now
    /// looking at a different conversation or a newer selection than before.
    fn adopt_selection_generation(
        &mut self,
        previous_conversation_id: Option<Uuid>,
        previous_selection_generation: u64,
        load_state: &ConversationLoadState,
    ) -> bool {
        match load_state {
            ConversationLoadState::Loading {
                conversation_id,
                generation,
            }
            | ConversationLoadState::Ready {
                conversation_id,
                generation,
            }
            | ConversationLoadState::Error {
                conversation_id,
                generation,
                ..
            } => {
                let changed = previous_conversation_id != Some(*conversation_id)
                    || previous_selection_generation != *generation;
                self.selection_generation = *generation;
                changed
            }
            ConversationLoadState::Idle => {
                let changed =
                    previous_conversation_id.is_some() || previous_selection_generation != 0;
                self.selection_generation = 0;
                changed
            }
        }
    }
}
