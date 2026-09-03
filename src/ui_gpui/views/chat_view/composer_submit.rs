//! What a composer submit means, and what it does.
//!
//! The Send button and the Enter key are the same gesture reached two ways.
//! Both ask [`ChatView::composer_submit`] what the current state makes of the
//! composer text and then hand the answer to [`ChatView::submit_composer`],
//! so a rule added to one is a rule added to both.
//!
//! @plan PLAN-20260903-ISSUE222.P04
//! @requirement REQ-222-002

use super::state::StreamingState;
use super::ChatView;
use crate::events::types::UserEvent;
use uuid::Uuid;

/// The meaning of a composer submit in the view's current state.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-002
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ComposerSubmit {
    /// There is nothing to submit: the composer holds no non-whitespace text,
    /// or a running turn has no conversation to name.
    Nothing,
    /// Nothing is running, so the text starts a turn.
    Send(String),
    /// A turn is running, so the text joins it instead of starting a second.
    Steer { conversation_id: Uuid, text: String },
}

impl ChatView {
    /// Reads the composer and the streaming state, without changing either.
    ///
    /// A steer names the same conversation the Stop button would cancel,
    /// because it joins that turn. With no conversation selected there is no
    /// turn to join and no id to send, so there is nothing to submit.
    ///
    /// @plan PLAN-20260903-ISSUE222.P04
    /// @requirement REQ-222-002
    pub(super) fn composer_submit(&self) -> ComposerSubmit {
        if self.state.input_text.trim().is_empty() {
            return ComposerSubmit::Nothing;
        }
        let text = self.state.input_text.clone();

        if !matches!(self.state.streaming, StreamingState::Streaming { .. }) {
            return ComposerSubmit::Send(text);
        }

        self.state
            .active_conversation_id
            .map_or(ComposerSubmit::Nothing, |conversation_id| {
                ComposerSubmit::Steer {
                    conversation_id,
                    text,
                }
            })
    }

    /// Applies whatever the current state makes of the composer text.
    ///
    /// @plan PLAN-20260903-ISSUE222.P04
    /// @requirement REQ-222-002
    pub(super) fn submit_composer(&mut self, cx: &mut gpui::Context<Self>) {
        match self.composer_submit() {
            ComposerSubmit::Nothing => {}
            ComposerSubmit::Send(text) => {
                tracing::info!("Composer submit - emitting SendMessage: {}", text);
                self.send_message_and_start_streaming(text, cx);
            }
            ComposerSubmit::Steer {
                conversation_id,
                text,
            } => {
                tracing::info!(%conversation_id, "Composer submit - steering the running turn");
                self.steer_streaming(conversation_id, text, cx);
            }
        }
    }

    /// Offers the composer text to the turn already running.
    ///
    /// The streaming state is deliberately untouched: the turn keeps going,
    /// and the service reports back through `SteeringQueued` (or refuses with
    /// `SteeringRejected`). Nothing here cancels anything.
    ///
    /// @plan PLAN-20260903-ISSUE222.P04
    /// @requirement REQ-222-002
    /// @requirement REQ-222-006
    fn steer_streaming(
        &mut self,
        conversation_id: Uuid,
        text: String,
        cx: &mut gpui::Context<Self>,
    ) {
        self.emit(UserEvent::SteerStreaming {
            conversation_id,
            text,
        });
        self.state.input_text.clear();
        self.state.cursor_position = 0;
        self.state.chat_autoscroll_enabled = true;
        self.state.conversation_dropdown_open = false;
        self.state.profile_dropdown_open = false;
        self.state.conversation_title_editing = false;
        self.maybe_scroll_chat_to_bottom();
        cx.notify();
    }

    pub(super) fn send_message_and_start_streaming(
        &mut self,
        text: String,
        cx: &mut gpui::Context<Self>,
    ) {
        self.emit(UserEvent::SendMessage {
            text,
            conversation_id: self.conversation_id,
        });
        self.state.input_text.clear();
        self.state.cursor_position = 0;
        self.state.chat_autoscroll_enabled = true;
        self.state.conversation_dropdown_open = false;
        self.state.profile_dropdown_open = false;
        self.state.conversation_title_editing = false;
        self.state.streaming = StreamingState::Streaming {
            content: String::new(),
            done: false,
        };
        self.refresh_transcript_selection_revisions();
        self.maybe_scroll_chat_to_bottom();
        cx.notify();
    }
}
