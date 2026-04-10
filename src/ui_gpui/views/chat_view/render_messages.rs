//! Message-rendering helpers extracted from `render.rs` to keep that file
//! under the project's structural file-length cap.
//!
//! Contains the emoji-stripping helpers, the transcript-aware `StyledText`
//! builder used by selection highlighting, and all of the per-message bubble
//! renderers (user / assistant / streaming / approval).
//!
//! @plan PLAN-20260406-ISSUE151.P01

use super::state::{ApprovalBubbleState, ChatMessage, MessageRole, StreamingState};
use super::ChatView;
use crate::events::types::{ToolApprovalResponseAction, UserEvent};
use crate::ui_gpui::components::{ApprovalBubble, AssistantBubble, UserBubble};
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, IntoElement, StyledText, TextRun};

/// Strip emojis from a string, replacing them with empty string.
/// This only affects display, not the underlying database storage.
pub(super) fn strip_emojis(text: &str) -> String {
    text.chars().filter(|c| !is_emoji(*c)).collect()
}

/// Check if a character is an emoji.
/// Uses Unicode ranges for emoji blocks.
const fn is_emoji(c: char) -> bool {
    // Basic emoji ranges - these cover most emojis
    matches!(c,
        '\u{1F600}'..='\u{1F64F}' |  // Emoticons
        '\u{1F300}'..='\u{1F5FF}' |  // Misc Symbols and Pictographs
        '\u{1F680}'..='\u{1F6FF}' |  // Transport and Map
        '\u{1F1E0}'..='\u{1F1FF}' |  // Flags
        '\u{2600}'..='\u{26FF}'   |  // Misc symbols
        '\u{2700}'..='\u{27BF}'   |  // Dingbats
        '\u{1F900}'..='\u{1F9FF}' |  // Supplemental Symbols and Pictographs
        '\u{1FA00}'..='\u{1FA6F}' |  // Chess Symbols
        '\u{1FA70}'..='\u{1FAFF}' |  // Symbols and Pictographs Extended-A
        '\u{2B50}'                |  // Star
        '\u{2B55}'                |  // Circle
        '\u{25AA}'..='\u{25AB}'   |  // Small squares
        '\u{25B6}' | '\u{25C0}'   |  // Play buttons
        '\u{25FB}'..='\u{25FE}'   |  // Medium squares
        '\u{2934}'..='\u{2935}'   |  // Arrows
        '\u{2B05}'..='\u{2B07}'   |  // Arrows
        '\u{2B1B}'..='\u{2B1C}'   |  // Squares
        '\u{3030}'                |  // Wavy dash
        '\u{303D}'                |  // Part alternation mark
        '\u{3297}'                |  // Circled ideograph congratulation
        '\u{3299}'                |  // Circled ideograph secret
        '\u{FE0F}'                |  // Variation Selector-16
        '\u{20E3}'                |  // Combining enclosing keycap
        '\u{E0020}'..='\u{E007F}' // Tags for emoji sequences
    )
}

/// Build a `StyledText` for a transcript block, optionally highlighting a
/// selection sub-range. The returned text is byte-identical to `text` so the
/// associated `TextLayout` can be used for hit-testing.
#[allow(clippy::option_if_let_else, clippy::similar_names)]
pub(super) fn render_transcript_text(
    text: &str,
    selection: Option<std::ops::Range<usize>>,
) -> StyledText {
    let clamped = selection.and_then(|range| {
        if range.is_empty() {
            return None;
        }
        let start = range.start.min(text.len());
        let end = range.end.min(text.len());
        (start < end).then_some(start..end)
    });

    let normal_color = Theme::text_primary();
    let highlight_fg = Theme::selection_fg();
    let highlight_bg = Theme::selection_bg();

    if let Some(range) = clamped {
        let before_len = range.start;
        let selected_len = range.end - range.start;
        let after_len = text.len() - range.end;

        let mut runs = Vec::new();
        if before_len > 0 {
            runs.push(TextRun {
                len: before_len,
                color: normal_color,
                ..Default::default()
            });
        }
        runs.push(TextRun {
            len: selected_len,
            color: highlight_fg,
            background_color: Some(highlight_bg),
            ..Default::default()
        });
        if after_len > 0 {
            runs.push(TextRun {
                len: after_len,
                color: normal_color,
                ..Default::default()
            });
        }
        StyledText::new(text.to_string()).with_runs(runs)
    } else {
        StyledText::new(text.to_string()).with_runs(vec![TextRun {
            len: text.len(),
            color: normal_color,
            ..Default::default()
        }])
    }
}

impl ChatView {
    /// Render the streaming assistant message bubble.
    pub(super) fn render_streaming_message(
        &self,
        streaming: &StreamingState,
        show_thinking: bool,
        filter_emoji: bool,
    ) -> impl IntoElement {
        let (content, _done) = match streaming {
            StreamingState::Streaming { content, done } => {
                tracing::debug!(
                    stream_buffer_len = content.len(),
                    "rendering streaming assistant bubble"
                );
                (content.clone(), *done)
            }
            _ => (String::new(), false),
        };
        let display_content = if filter_emoji {
            strip_emojis(&content)
        } else {
            content
        };
        let mut bubble = AssistantBubble::new(display_content)
            .model_id("streaming")
            .show_thinking(show_thinking)
            .streaming(true);
        if let Some(ref thinking) = self.state.thinking_content {
            if !thinking.is_empty() {
                bubble = bubble.thinking(thinking.clone());
            }
        }
        div().id("streaming-msg").child(bubble)
    }

    /// Render a single message
    /// @plan PLAN-20250130-GPUIREDUX.P03
    /// @plan PLAN-20260406-ISSUE151.P01 - added `message_index` for selection
    pub(super) fn render_message(
        msg: &ChatMessage,
        show_thinking: bool,
        filter_emoji: bool,
        _message_index: usize,
        selection: Option<std::ops::Range<usize>>,
    ) -> gpui::AnyElement {
        match msg.role {
            MessageRole::User => Self::render_user_message(&msg.content, selection),
            MessageRole::Assistant => {
                Self::render_assistant_message(msg, show_thinking, filter_emoji, selection)
            }
        }
    }

    /// Render user message - right aligned, green bubble
    /// @plan PLAN-20260406-ISSUE151.P01
    /// @plan:PLAN-20260402-ISSUE153.P02
    /// @requirement:REQ-MSG-LINK-001
    pub(super) fn render_user_message(
        content: &str,
        selection: Option<std::ops::Range<usize>>,
    ) -> gpui::AnyElement {
        UserBubble::new(content)
            .selection(selection)
            .into_any_element()
    }

    /// Render assistant message - left aligned, dark bubble with model label
    /// @plan:PLAN-20260402-MARKDOWN.P11
    /// @requirement:REQ-MD-INTEGRATE-010
    /// @plan PLAN-20260406-ISSUE151.P01 - added selection parameter
    pub(super) fn render_assistant_message(
        msg: &ChatMessage,
        show_thinking: bool,
        filter_emoji: bool,
        selection: Option<std::ops::Range<usize>>,
    ) -> gpui::AnyElement {
        let content = if filter_emoji {
            strip_emojis(&msg.content)
        } else {
            msg.content.clone()
        };

        let mut bubble = AssistantBubble::new(content).selection(selection);

        if let Some(ref model_label) = msg.model_label {
            bubble = bubble.model_id(model_label.clone());
        } else {
            bubble = bubble.model_id("Assistant");
        }

        if show_thinking {
            if let Some(ref thinking) = msg.thinking {
                bubble = bubble.thinking(thinking.clone()).show_thinking(true);
            }
        }

        bubble.into_any_element()
    }

    /// Render a single inline approval bubble with action button callbacks.
    ///
    /// A shared `AtomicBool` guard prevents duplicate responses from rapid
    /// clicks — once any button fires, all four become no-ops.
    ///
    /// For grouped bubbles, all `request_ids` in the group are resolved with
    /// the same decision.
    pub(super) fn render_approval_bubble(
        &self,
        bubble: &super::state::ToolApprovalBubble,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let request_ids = bubble.request_ids.clone();
        let state = bubble.state.clone();
        let operation_count = bubble.operation_count();
        let grouped_ops = bubble.grouped_operations.clone();
        let expanded = bubble.expanded;

        let mut approval = ApprovalBubble::new(&bubble.request_id, bubble.context.clone(), state)
            .operation_count(operation_count)
            .expanded(expanded)
            .grouped_operations(grouped_ops);

        if matches!(bubble.state, ApprovalBubbleState::Pending) {
            let bridge = self.bridge.clone();
            let decided = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

            let rids = request_ids.clone();
            let b1 = bridge.clone();
            let d1 = decided.clone();
            approval = approval.on_yes(move || {
                if d1.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b1 {
                    for rid in &rids {
                        bridge.emit(UserEvent::ToolApprovalResponse {
                            request_id: rid.clone(),
                            decision: ToolApprovalResponseAction::ProceedOnce,
                        });
                    }
                }
            });

            let rids = request_ids.clone();
            let b2 = bridge.clone();
            let d2 = decided.clone();
            approval = approval.on_session(move || {
                if d2.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b2 {
                    for rid in &rids {
                        bridge.emit(UserEvent::ToolApprovalResponse {
                            request_id: rid.clone(),
                            decision: ToolApprovalResponseAction::ProceedSession,
                        });
                    }
                }
            });

            let rids = request_ids.clone();
            let b3 = bridge.clone();
            let d3 = decided.clone();
            approval = approval.on_always(move || {
                if d3.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b3 {
                    for rid in &rids {
                        bridge.emit(UserEvent::ToolApprovalResponse {
                            request_id: rid.clone(),
                            decision: ToolApprovalResponseAction::ProceedAlways,
                        });
                    }
                }
            });

            let rids = request_ids;
            let b4 = bridge;
            let d4 = decided;
            approval = approval.on_no(move || {
                if d4.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b4 {
                    for rid in &rids {
                        bridge.emit(UserEvent::ToolApprovalResponse {
                            request_id: rid.clone(),
                            decision: ToolApprovalResponseAction::Denied,
                        });
                    }
                }
            });
        }

        // Use cx to mark the closure as capturing the context lifetime
        let _ = cx;
        approval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_emojis_removes_basic_emojis() {
        // Test basic emoji removal
        let input = "Hello \u{1F60A} World";
        assert_eq!(strip_emojis(input), "Hello  World");
        assert_eq!(strip_emojis("No emojis here"), "No emojis here");
        // Multiple emojis
        let emojis_only = "\u{1F389}\u{1F38A}\u{1F380}";
        assert_eq!(strip_emojis(emojis_only), "");
    }

    #[test]
    fn test_strip_emojis_handles_mixed_content() {
        let input = "Great news! \u{1F389} We shipped the feature \u{1F680}";
        assert_eq!(strip_emojis(input), "Great news!  We shipped the feature ");
    }

    #[test]
    fn test_strip_emojis_preserves_regular_characters() {
        // Test that regular punctuation and special chars are preserved
        let input = "Special chars: !@#$%^&*()_+-=[]{}|;':,./<>?";
        assert_eq!(strip_emojis(input), input);
    }

    #[test]
    fn test_strip_emojis_empty_string() {
        assert_eq!(strip_emojis(""), "");
    }

    #[test]
    fn test_is_emoji_detects_emoticons() {
        assert!(is_emoji('\u{1F60A}')); // smiling face
        assert!(is_emoji('\u{1F602}')); // face with tears of joy
        assert!(is_emoji('\u{2764}')); // heart
    }

    #[test]
    fn test_is_emoji_detects_symbols() {
        assert!(is_emoji('\u{2B50}')); // star
        assert!(is_emoji('\u{2705}')); // check mark
    }

    #[test]
    fn test_is_emoji_rejects_regular_chars() {
        assert!(!is_emoji('A'));
        assert!(!is_emoji('a'));
        assert!(!is_emoji('1'));
        assert!(!is_emoji('!'));
    }
}
