//! Flattened rows rendered by the virtualized chat transcript.

use super::emoji::strip_emojis;
use super::state::{
    ApprovalBubbleState, ChatMessage, MessageRole, StreamingState, ToolApprovalBubble,
};
use crate::ui_gpui::components::markdown_content::{markdown_leaf_count, parse_markdown_blocks};

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

pub(super) fn transcript_row_leaf_count(
    row: TranscriptRow,
    messages: &[ChatMessage],
    streaming: &StreamingState,
    filter_emoji: bool,
) -> usize {
    match row {
        TranscriptRow::Message(index) => {
            let message = &messages[index];
            if filter_emoji && message.role == MessageRole::Assistant {
                markdown_leaf_count(&parse_markdown_blocks(&strip_emojis(&message.content)))
            } else {
                markdown_leaf_count(&message.get_or_parse_markdown())
            }
        }
        TranscriptRow::Approval(_) => 0,
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
            markdown_leaf_count(&parse_markdown_blocks(&format!("{content}▋")))
        }
    }
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
