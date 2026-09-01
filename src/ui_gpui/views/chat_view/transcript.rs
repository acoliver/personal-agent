//! Flattened rows rendered by the virtualized chat transcript.

use super::state::{ApprovalBubbleState, StreamingState, ToolApprovalBubble};

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
}
