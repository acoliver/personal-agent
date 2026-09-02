//! Flattened rows rendered by the virtualized chat transcript.

use super::emoji::strip_emojis;
use super::state::{
    ApprovalBubbleState, ChatMessage, MessageRole, StreamingState, ToolApprovalBubble,
};
use crate::ui_gpui::components::markdown_content::{markdown_leaf_count, parse_markdown_blocks};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TranscriptRow {
    Message(usize),
    Approval(usize),
    Streaming,
}

pub(super) fn build_transcript_rows(
    message_count: usize,
    approval_bubbles: &[ToolApprovalBubble],
    streaming: &StreamingState,
) -> Vec<TranscriptRow> {
    let mut rows = Vec::with_capacity(message_count + 2);
    rows.extend((0..message_count).map(TranscriptRow::Message));

    if let Some((index, _)) = approval_bubbles
        .iter()
        .enumerate()
        .find(|(_, bubble)| bubble.state == ApprovalBubbleState::Pending)
    {
        rows.push(TranscriptRow::Approval(index));
    }

    if matches!(streaming, StreamingState::Streaming { .. }) {
        rows.push(TranscriptRow::Streaming);
    }

    rows
}

/// The thinking text a transcript row displays, if any.
///
/// Thinking is displayed only when visibility is on and the text has
/// content. When the emoji filter is on, emoji are stripped first and the
/// non-blank rule applies to the stripped text, so thinking that blanks out
/// never becomes a leaf. Row leaf counting, selection identity, copy
/// documents, and rendering all agree through this one function.
pub(super) fn displayed_thinking(
    thinking: Option<&str>,
    show_thinking: bool,
    filter_emoji: bool,
) -> Option<Cow<'_, str>> {
    if !show_thinking {
        return None;
    }
    let text = thinking?;
    if filter_emoji {
        let stripped = strip_emojis(text);
        (!stripped.trim().is_empty()).then_some(Cow::Owned(stripped))
    } else {
        (!text.trim().is_empty()).then_some(Cow::Borrowed(text))
    }
}

/// The thinking text a message row displays, if any.
///
/// Rendering attaches thinking only to assistant bubbles, so the role gate
/// lives here beside the visibility gate, the emoji filter, and the
/// non-blank rule. Row leaf counting, selection identity, copy documents,
/// and rendering all agree through this one function.
pub(super) fn displayed_message_thinking(
    message: &ChatMessage,
    show_thinking: bool,
    filter_emoji: bool,
) -> Option<Cow<'_, str>> {
    if message.role != MessageRole::Assistant {
        return None;
    }
    displayed_thinking(
        message.thinking.as_deref().map(String::as_str),
        show_thinking,
        filter_emoji,
    )
}

pub(super) fn transcript_row_leaf_count(
    row: TranscriptRow,
    messages: &[ChatMessage],
    streaming: &StreamingState,
    filter_emoji: bool,
    show_thinking: bool,
    streaming_thinking: Option<&str>,
) -> usize {
    let (content_leaf_count, displayed_thinking) = match row {
        TranscriptRow::Message(index) => {
            let message = &messages[index];
            let content_leaf_count = if filter_emoji && message.role == MessageRole::Assistant {
                markdown_leaf_count(&parse_markdown_blocks(&strip_emojis(&message.content)))
            } else {
                markdown_leaf_count(&message.get_or_parse_markdown())
            };
            (
                content_leaf_count,
                displayed_message_thinking(message, show_thinking, filter_emoji),
            )
        }
        TranscriptRow::Approval(_) => return 0,
        TranscriptRow::Streaming => {
            let content = match streaming {
                StreamingState::Streaming { content, .. } => content.as_str(),
                StreamingState::Idle | StreamingState::Error(_) => "",
            };
            let content = if filter_emoji {
                strip_emojis(content)
            } else {
                content.to_string()
            };
            (
                markdown_leaf_count(&parse_markdown_blocks(&format!("{content}▋"))),
                displayed_thinking(streaming_thinking, show_thinking, filter_emoji),
            )
        }
    };
    content_leaf_count + usize::from(displayed_thinking.is_some())
}

pub(super) fn derive_document_orders(row_leaf_counts: &[usize]) -> Vec<u64> {
    let mut next_document_order = 0_u64;
    row_leaf_counts
        .iter()
        .map(|&leaf_count| {
            let document_order = next_document_order;
            let leaf_count = u64::try_from(leaf_count).expect("leaf count does not fit in u64");
            next_document_order = next_document_order
                .checked_add(leaf_count)
                .expect("transcript document order overflowed u64");
            document_order
        })
        .collect()
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::presentation::view_command::{ToolApprovalContext, ToolCategory};

    #[test]
    fn displayed_thinking_requires_visibility_and_non_empty_text() {
        assert_eq!(
            displayed_thinking(Some("reasoning"), true, false),
            Some(Cow::Borrowed("reasoning"))
        );
        assert_eq!(displayed_thinking(Some("reasoning"), false, false), None);
        assert_eq!(displayed_thinking(Some(""), true, false), None);
        assert_eq!(displayed_thinking(Some("   "), true, false), None);
        assert_eq!(displayed_thinking(None, true, false), None);
    }

    #[test]
    fn displayed_thinking_strips_emoji_and_drops_blank_results() {
        assert_eq!(
            displayed_thinking(Some("thought \u{1F600} here"), true, true),
            Some(Cow::Owned("thought  here".to_string()))
        );
        // Blank-after-stripping thinking is not displayed.
        assert_eq!(displayed_thinking(Some("\u{1F600}"), true, true), None);
        assert_eq!(displayed_thinking(Some(" \u{1F600} "), true, true), None);
        // With the filter off, emoji-bearing thinking displays verbatim.
        assert_eq!(
            displayed_thinking(Some("\u{1F600}"), true, false),
            Some(Cow::Borrowed("\u{1F600}"))
        );
    }

    #[test]
    fn message_row_leaf_count_includes_one_displayed_thinking_leaf() {
        let thinking = ChatMessage::assistant("answer", "model").with_thinking("pondering");
        let plain = ChatMessage::assistant("answer", "model");
        let messages = vec![thinking, plain];

        let shown = transcript_row_leaf_count(
            TranscriptRow::Message(0),
            &messages,
            &StreamingState::Idle,
            false,
            true,
            None,
        );
        let hidden = transcript_row_leaf_count(
            TranscriptRow::Message(0),
            &messages,
            &StreamingState::Idle,
            false,
            false,
            None,
        );

        assert_eq!(shown, hidden + 1);
    }

    #[test]
    fn message_row_leaf_count_excludes_empty_thinking_text() {
        let messages = vec![ChatMessage::assistant("answer", "model").with_thinking("   ")];

        let count = transcript_row_leaf_count(
            TranscriptRow::Message(0),
            &messages,
            &StreamingState::Idle,
            false,
            true,
            None,
        );

        let without_thinking = transcript_row_leaf_count(
            TranscriptRow::Message(0),
            &messages,
            &StreamingState::Idle,
            false,
            false,
            None,
        );
        assert_eq!(count, without_thinking);
    }

    #[test]
    fn user_message_rows_never_gain_a_thinking_leaf() {
        let messages = vec![ChatMessage::user("hello")];

        let count = transcript_row_leaf_count(
            TranscriptRow::Message(0),
            &messages,
            &StreamingState::Idle,
            false,
            true,
            Some("streaming thoughts"),
        );

        let baseline = transcript_row_leaf_count(
            TranscriptRow::Message(0),
            &messages,
            &StreamingState::Idle,
            false,
            false,
            None,
        );
        assert_eq!(count, baseline);
    }

    #[test]
    fn user_message_with_thinking_text_gains_no_thinking_leaf() {
        let thinking = vec![ChatMessage::user("hello").with_thinking("secret thoughts")];
        let baseline = vec![ChatMessage::user("hello")];

        for filter_emoji in [false, true] {
            let count = transcript_row_leaf_count(
                TranscriptRow::Message(0),
                &thinking,
                &StreamingState::Idle,
                filter_emoji,
                true,
                None,
            );
            let without = transcript_row_leaf_count(
                TranscriptRow::Message(0),
                &baseline,
                &StreamingState::Idle,
                filter_emoji,
                true,
                None,
            );
            assert_eq!(
                count, without,
                "user thinking must not become a leaf with filter_emoji={filter_emoji}"
            );
        }
    }

    #[test]
    fn streaming_row_leaf_count_includes_displayed_thinking_leaf() {
        let streaming = StreamingState::Streaming {
            content: "partial".to_string(),
            done: false,
        };
        let messages = [ChatMessage::user("go")];

        let shown = transcript_row_leaf_count(
            TranscriptRow::Streaming,
            &messages,
            &streaming,
            false,
            true,
            Some("hmm"),
        );
        let hidden = transcript_row_leaf_count(
            TranscriptRow::Streaming,
            &messages,
            &streaming,
            false,
            false,
            Some("hmm"),
        );

        assert_eq!(shown, hidden + 1);
    }

    #[test]
    fn thinking_leaf_count_keeps_document_orders_stable_per_visibility() {
        // One content leaf per message; the thinking leaf shifts the orders
        // of every later row exactly like any other leaf would.
        let messages = vec![
            ChatMessage::assistant("one", "model").with_thinking("first thoughts"),
            ChatMessage::user("two"),
        ];
        let streaming = StreamingState::Idle;

        let shown_counts: Vec<usize> = (0..messages.len())
            .map(|index| {
                transcript_row_leaf_count(
                    TranscriptRow::Message(index),
                    &messages,
                    &streaming,
                    false,
                    true,
                    None,
                )
            })
            .collect();
        let hidden_counts: Vec<usize> = (0..messages.len())
            .map(|index| {
                transcript_row_leaf_count(
                    TranscriptRow::Message(index),
                    &messages,
                    &streaming,
                    false,
                    false,
                    None,
                )
            })
            .collect();

        assert_eq!(derive_document_orders(&shown_counts), vec![0, 2]);
        assert_eq!(derive_document_orders(&hidden_counts), vec![0, 1]);
    }

    fn approval(state: ApprovalBubbleState) -> ToolApprovalBubble {
        let mut bubble = ToolApprovalBubble::new(
            "request",
            ToolApprovalContext::new("ShellExec", ToolCategory::Shell, "echo test"),
        );
        bubble.state = state;
        bubble
    }

    #[test]
    fn empty_transcript_has_no_rows() {
        assert!(build_transcript_rows(0, &[], &StreamingState::Idle).is_empty());
    }

    #[test]
    fn messages_are_followed_by_streaming_row() {
        assert_eq!(
            build_transcript_rows(
                2,
                &[],
                &StreamingState::Streaming {
                    content: "partial".to_string(),
                    done: false,
                },
            ),
            vec![
                TranscriptRow::Message(0),
                TranscriptRow::Message(1),
                TranscriptRow::Streaming,
            ]
        );
    }

    #[test]
    fn full_transcript_keeps_row_order_and_first_pending_source_index() {
        let bubbles = vec![
            approval(ApprovalBubbleState::Approved),
            approval(ApprovalBubbleState::Pending),
            approval(ApprovalBubbleState::Pending),
        ];
        let streaming = StreamingState::Streaming {
            content: "partial".to_string(),
            done: false,
        };

        assert_eq!(
            build_transcript_rows(1, &bubbles, &streaming),
            vec![
                TranscriptRow::Message(0),
                TranscriptRow::Approval(1),
                TranscriptRow::Streaming,
            ]
        );
    }

    #[test]
    fn document_orders_are_prefix_sums_in_transcript_row_order() {
        assert_eq!(derive_document_orders(&[2, 3, 1]), vec![0, 2, 5]);
    }

    #[test]
    fn document_orders_remain_stable_when_visible_rows_paint_out_of_order() {
        let orders = derive_document_orders(&[2, 0, 3, 1, 4]);

        let painted_rows = [4, 2, 0];
        let painted_orders: Vec<u64> = painted_rows
            .into_iter()
            .map(|row_index| orders[row_index])
            .collect();

        assert_eq!(painted_orders, vec![6, 2, 0]);
        assert_eq!(orders, vec![0, 2, 2, 5, 6]);
    }

    #[test]
    fn zero_leaf_rows_do_not_create_document_order_gaps() {
        assert_eq!(derive_document_orders(&[1, 0, 0, 2]), vec![0, 1, 1, 1]);
    }
}
