//! Selectable markdown leaves and source-copy projection for the chat transcript.

use super::markdown_content::{MarkdownCopyLeaf, MarkdownLeaf, MarkdownLeafFactory};
use gpui::{IntoElement, Pixels, Point, SharedString};
use gpui_selection_vendor::{SelectableText, TextSelectionContentKey, TextSelectionCoverage};
use std::sync::Arc;

#[derive(Clone)]
pub struct TranscriptSelectionContext {
    pub scroll_offset: Point<Pixels>,
    pub document_order: u64,
    pub first_copy_separator: &'static str,
    pub content_key: TextSelectionContentKey,
    pub copy_document: Arc<TranscriptCopyDocument>,
}

#[derive(Clone, Debug)]
struct TranscriptCopyLeaf {
    document_order: u64,
    text: String,
    separator_before: String,
}

#[derive(Clone, Debug)]
struct TranscriptCopyMessage {
    content_key: TextSelectionContentKey,
    leaves: Vec<TranscriptCopyLeaf>,
}

#[derive(Clone, Debug, Default)]
pub struct TranscriptCopyDocument {
    messages: Vec<TranscriptCopyMessage>,
}

impl TranscriptCopyDocument {
    pub fn new(messages: Vec<(TextSelectionContentKey, Vec<MarkdownCopyLeaf>)>) -> Self {
        let mut next_document_order = 0_u64;
        let messages = messages
            .into_iter()
            .map(|(content_key, leaves)| {
                let leaves = leaves
                    .into_iter()
                    .map(|leaf| {
                        let document_order = next_document_order;
                        next_document_order = next_document_order
                            .checked_add(1)
                            .expect("transcript copy document order overflowed u64");
                        TranscriptCopyLeaf {
                            document_order,
                            text: leaf.text,
                            separator_before: leaf.separator_before,
                        }
                    })
                    .collect();
                TranscriptCopyMessage {
                    content_key,
                    leaves,
                }
            })
            .collect();
        Self { messages }
    }

    fn copy_for(
        &self,
        coverage: TextSelectionCoverage,
        projected: Option<&str>,
        document_order: u64,
        endpoint_keys: [TextSelectionContentKey; 2],
    ) -> Option<String> {
        let (message_index, leaf_index) = self.leaf_location(document_order)?;
        let [Some(anchor_index), Some(cursor_index)] =
            endpoint_keys.map(|key| self.message_index(key))
        else {
            return None;
        };
        let endpoint_indices = [anchor_index, cursor_index];
        let same_message = endpoint_indices[0] == endpoint_indices[1];
        match coverage {
            TextSelectionCoverage::Bounded => Some(projected?.to_string()),
            TextSelectionCoverage::Full if same_message => {
                Some(self.leaf_with_separator(message_index, leaf_index))
            }
            TextSelectionCoverage::Full => Some(String::new()),
            TextSelectionCoverage::ToEnd if same_message => Some(projected?.to_string()),
            TextSelectionCoverage::ToEnd => self.copy_from_lower_endpoint(
                message_index,
                leaf_index,
                projected?,
                endpoint_indices,
            ),
            TextSelectionCoverage::FromStart if same_message => {
                let mut copied = self.messages[message_index].leaves[leaf_index]
                    .separator_before
                    .clone();
                copied.push_str(projected?);
                Some(copied)
            }
            TextSelectionCoverage::FromStart => {
                self.copy_to_upper_endpoint(message_index, leaf_index, projected?, endpoint_indices)
            }
        }
    }

    fn copy_from_lower_endpoint(
        &self,
        message_index: usize,
        leaf_index: usize,
        projected: &str,
        endpoint_indices: [usize; 2],
    ) -> Option<String> {
        let lower = endpoint_indices[0].min(endpoint_indices[1]);
        let upper = endpoint_indices[0].max(endpoint_indices[1]);
        if message_index != lower {
            return None;
        }
        let mut copied = projected.to_string();
        for leaf in &self.messages[lower].leaves[leaf_index + 1..] {
            append_leaf(&mut copied, leaf);
        }
        for message in &self.messages[lower + 1..upper] {
            append_message(&mut copied, message);
        }
        Some(copied)
    }

    fn copy_to_upper_endpoint(
        &self,
        message_index: usize,
        leaf_index: usize,
        projected: &str,
        endpoint_indices: [usize; 2],
    ) -> Option<String> {
        let upper = endpoint_indices[0].max(endpoint_indices[1]);
        if message_index != upper {
            return None;
        }
        let mut message_text = String::new();
        for leaf in &self.messages[upper].leaves[..leaf_index] {
            append_leaf(&mut message_text, leaf);
        }
        if !projected.is_empty() {
            if !message_text.is_empty() {
                message_text.push_str(&self.messages[upper].leaves[leaf_index].separator_before);
            }
            message_text.push_str(projected);
        }
        Some(if message_text.is_empty() {
            String::new()
        } else {
            format!("\n\n{message_text}")
        })
    }

    fn leaf_location(&self, document_order: u64) -> Option<(usize, usize)> {
        self.messages
            .iter()
            .enumerate()
            .find_map(|(message, data)| {
                data.leaves
                    .iter()
                    .position(|leaf| leaf.document_order == document_order)
                    .map(|leaf| (message, leaf))
            })
    }

    fn message_index(&self, key: TextSelectionContentKey) -> Option<usize> {
        self.messages
            .iter()
            .position(|message| message.content_key == key)
    }

    fn leaf_with_separator(&self, message_index: usize, leaf_index: usize) -> String {
        let leaf = &self.messages[message_index].leaves[leaf_index];
        if leaf.text.trim().is_empty() {
            String::new()
        } else {
            format!("{}{}", leaf.separator_before, leaf.text)
        }
    }

    /// Returns every message key in reading order and the complete copy text.
    ///
    /// Blank messages contribute their key (so freshness validation still
    /// notices their replacement) without contributing text. Returns `None`
    /// when the document has no messages or only blank text.
    pub fn select_all_payload(&self) -> Option<(Vec<TextSelectionContentKey>, String)> {
        let keys = self
            .messages
            .iter()
            .map(|message| message.content_key)
            .collect::<Vec<_>>();
        let mut text = String::new();
        for message in &self.messages {
            append_message(&mut text, message);
        }
        if keys.is_empty() || text.trim().is_empty() {
            return None;
        }
        Some((keys, text))
    }
}

fn append_leaf(output: &mut String, leaf: &TranscriptCopyLeaf) {
    if leaf.text.trim().is_empty() {
        return;
    }
    if !output.is_empty() {
        output.push_str(&leaf.separator_before);
    }
    output.push_str(&leaf.text);
}

fn append_message(output: &mut String, message: &TranscriptCopyMessage) {
    let mut message_text = String::new();
    for leaf in &message.leaves {
        append_leaf(&mut message_text, leaf);
    }
    if message_text.is_empty() {
        return;
    }
    if !output.is_empty() {
        output.push_str("\n\n");
    }
    output.push_str(&message_text);
}

pub struct TranscriptSelectionLeafFactory {
    scroll_offset: Point<Pixels>,
    content_key: TextSelectionContentKey,
    copy_document: Arc<TranscriptCopyDocument>,
}

impl TranscriptSelectionLeafFactory {
    pub const fn new(
        scroll_offset: Point<Pixels>,
        content_key: TextSelectionContentKey,
        copy_document: Arc<TranscriptCopyDocument>,
    ) -> Self {
        Self {
            scroll_offset,
            content_key,
            copy_document,
        }
    }
}

impl MarkdownLeafFactory for TranscriptSelectionLeafFactory {
    fn create_leaf(&mut self, leaf: MarkdownLeaf) -> gpui::AnyElement {
        let id = SharedString::from(format!(
            "selection-leaf-{}-{}",
            self.content_key.value(),
            leaf.document_order
        ));
        let document_order = leaf.document_order;
        let copy_document = Arc::clone(&self.copy_document);
        SelectableText::new(
            id,
            leaf.plain_text,
            leaf.text_runs,
            leaf.surface_background,
            leaf.surface_foreground,
        )
        .links(leaf.links)
        .document_order(document_order)
        .scroll_offset(self.scroll_offset)
        .copy_separator_before("")
        .content_key(self.content_key)
        .copy_with(move |snapshot, projected, _cx| {
            let snapshot = snapshot?;
            let [Some(anchor_key), Some(cursor_key)] = [
                snapshot.anchor().content_key(),
                snapshot.cursor().content_key(),
            ] else {
                return None;
            };
            copy_document.copy_for(
                snapshot.coverage(),
                projected.as_deref(),
                document_order,
                [anchor_key, cursor_key],
            )
        })
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(text: &str, separator: &str) -> MarkdownCopyLeaf {
        MarkdownCopyLeaf {
            text: text.to_string(),
            separator_before: separator.to_string(),
        }
    }

    fn document() -> (TranscriptCopyDocument, [TextSelectionContentKey; 3]) {
        let keys = [
            TextSelectionContentKey::new(11),
            TextSelectionContentKey::new(12),
            TextSelectionContentKey::new(13),
        ];
        let document = TranscriptCopyDocument::new(vec![
            (
                keys[0],
                vec![leaf("first start", ""), leaf("first code", "\n\n")],
            ),
            (
                keys[1],
                vec![
                    leaf("A", ""),
                    leaf("B", "\t"),
                    leaf("x", "\n"),
                    leaf("y", "\t"),
                ],
            ),
            (
                keys[2],
                vec![leaf("third start", ""), leaf("third end", "\n\n")],
            ),
        ]);
        (document, keys)
    }

    fn copy(
        document: &TranscriptCopyDocument,
        coverage: TextSelectionCoverage,
        projected: Option<&str>,
        document_order: u64,
        endpoint_keys: [TextSelectionContentKey; 2],
    ) -> String {
        document
            .copy_for(coverage, projected, document_order, endpoint_keys)
            .expect("test copy projection should resolve")
    }

    #[test]
    fn fully_covered_unpainted_message_is_inserted_between_endpoint_ranges() {
        let (document, keys) = document();
        for endpoint_keys in [[keys[0], keys[2]], [keys[2], keys[0]]] {
            let lower = copy(
                &document,
                TextSelectionCoverage::ToEnd,
                Some("start"),
                0,
                endpoint_keys,
            );
            let upper = copy(
                &document,
                TextSelectionCoverage::FromStart,
                Some("third"),
                6,
                endpoint_keys,
            );

            assert_eq!(
                format!("{lower}{upper}"),
                "start\n\nfirst code\n\nA\tB\nx\ty\n\nthird"
            );
        }
    }

    #[test]
    fn endpoint_messages_contribute_only_selected_utf8_ranges() {
        let (document, keys) = document();
        let endpoint_keys = [keys[0], keys[2]];
        let lower = copy(
            &document,
            TextSelectionCoverage::ToEnd,
            Some("é suffix"),
            0,
            endpoint_keys,
        );
        let upper = copy(
            &document,
            TextSelectionCoverage::FromStart,
            Some("prefix 🦀"),
            7,
            endpoint_keys,
        );

        assert_eq!(
            format!("{lower}{upper}"),
            "é suffix\n\nfirst code\n\nA\tB\nx\ty\n\nthird start\n\nprefix 🦀"
        );
    }

    #[test]
    fn painted_and_unpainted_participants_do_not_duplicate_text_or_separators() {
        let (document, keys) = document();
        let endpoint_keys = [keys[0], keys[2]];
        let outputs = [
            copy(
                &document,
                TextSelectionCoverage::ToEnd,
                Some("start"),
                0,
                endpoint_keys,
            ),
            copy(
                &document,
                TextSelectionCoverage::Full,
                None,
                2,
                endpoint_keys,
            ),
            copy(
                &document,
                TextSelectionCoverage::Full,
                None,
                3,
                endpoint_keys,
            ),
            copy(
                &document,
                TextSelectionCoverage::FromStart,
                Some("third"),
                6,
                endpoint_keys,
            ),
        ];

        assert_eq!(
            outputs.concat(),
            "start\n\nfirst code\n\nA\tB\nx\ty\n\nthird"
        );
    }

    #[test]
    fn same_message_selection_preserves_internal_leaf_separators() {
        let (document, keys) = document();
        let outputs = [
            copy(
                &document,
                TextSelectionCoverage::ToEnd,
                Some("start"),
                0,
                [keys[0], keys[0]],
            ),
            copy(
                &document,
                TextSelectionCoverage::FromStart,
                Some("code"),
                1,
                [keys[0], keys[0]],
            ),
        ];

        assert_eq!(outputs.concat(), "start\n\ncode");
    }

    #[test]
    fn missing_projection_or_stale_document_identity_refuses_copy() {
        let (document, keys) = document();
        assert_eq!(
            document.copy_for(TextSelectionCoverage::ToEnd, None, 0, [keys[0], keys[2]],),
            None
        );
        assert_eq!(
            document.copy_for(
                TextSelectionCoverage::Bounded,
                Some("partial"),
                u64::MAX,
                [keys[0], keys[2]],
            ),
            None
        );
        assert_eq!(
            document.copy_for(
                TextSelectionCoverage::Bounded,
                Some("partial"),
                0,
                [keys[0], TextSelectionContentKey::new(99)],
            ),
            None
        );
    }

    #[test]
    fn select_all_payload_returns_all_keys_and_complete_syntax_free_text() {
        let keys = [
            TextSelectionContentKey::new(21),
            TextSelectionContentKey::new(22),
            TextSelectionContentKey::new(23),
        ];
        let document = TranscriptCopyDocument::new(vec![
            (
                keys[0],
                vec![
                    leaf("plain text", ""),
                    leaf("let code = 1;", "\n\n"),
                    leaf("after code", "\n\n"),
                ],
            ),
            (
                keys[1],
                vec![leaf("h1", ""), leaf("a\tb", "\n\n"), leaf("1\n2", "\n\n")],
            ),
            (keys[2], vec![leaf("closing", "")]),
        ]);

        let (payload_keys, text) = document
            .select_all_payload()
            .expect("non-blank document yields a select-all payload");

        assert_eq!(payload_keys, keys);
        assert_eq!(
            text,
            "plain text\n\nlet code = 1;\n\nafter code\n\nh1\n\na\tb\n\n1\n2\n\nclosing"
        );
    }

    #[test]
    fn select_all_payload_includes_blank_message_keys_for_freshness() {
        let keys = [
            TextSelectionContentKey::new(31),
            TextSelectionContentKey::new(32),
            TextSelectionContentKey::new(33),
        ];
        let document = TranscriptCopyDocument::new(vec![
            (keys[0], vec![leaf("before", "")]),
            (keys[1], vec![leaf("   ", "")]),
            (keys[2], vec![leaf("after", "")]),
        ]);

        let (payload_keys, text) = document
            .select_all_payload()
            .expect("blank interior message does not blank the payload");

        assert_eq!(payload_keys, keys);
        assert_eq!(text, "before\n\nafter");
    }

    #[test]
    fn select_all_payload_is_none_for_empty_or_blank_only_documents() {
        assert_eq!(TranscriptCopyDocument::default().select_all_payload(), None);
        let blank = TranscriptCopyDocument::new(vec![(
            TextSelectionContentKey::new(41),
            vec![leaf(" \n ", "")],
        )]);
        assert_eq!(blank.select_all_payload(), None);
    }

    /// A message whose displayed thinking text is the first copy leaf.
    fn thinking_document() -> (TranscriptCopyDocument, [TextSelectionContentKey; 3]) {
        let keys = [
            TextSelectionContentKey::new(51),
            TextSelectionContentKey::new(52),
            TextSelectionContentKey::new(53),
        ];
        let document = TranscriptCopyDocument::new(vec![
            (keys[0], vec![leaf("question", "")]),
            (
                keys[1],
                vec![
                    leaf("pondering deeply", ""),
                    leaf("the answer", "\n\n"),
                    leaf("tail", "\n"),
                ],
            ),
            (keys[2], vec![leaf("thanks", "")]),
        ]);
        (document, keys)
    }

    #[test]
    fn selection_within_one_message_spans_thinking_and_content_leaves() {
        let (document, keys) = thinking_document();
        let endpoints = [keys[1], keys[1]];

        // Anchor inside the thinking leaf, cursor inside the first content leaf.
        let from_thinking = copy(
            &document,
            TextSelectionCoverage::ToEnd,
            Some("deeply"),
            1,
            endpoints,
        );
        let into_content = copy(&document, TextSelectionCoverage::Full, None, 2, endpoints);
        let to_cursor = copy(
            &document,
            TextSelectionCoverage::FromStart,
            Some("the ans"),
            2,
            endpoints,
        );

        assert_eq!(
            format!("{from_thinking}{into_content}{to_cursor}"),
            "deeply\n\nthe answer\n\nthe ans"
        );
    }

    #[test]
    fn selection_ending_inside_thinking_copies_only_leading_thinking_text() {
        let (document, keys) = thinking_document();
        let endpoints = [keys[0], keys[1]];

        let lower = copy(
            &document,
            TextSelectionCoverage::ToEnd,
            Some("stion"),
            0,
            endpoints,
        );
        let upper = copy(
            &document,
            TextSelectionCoverage::FromStart,
            Some("ponder"),
            1,
            endpoints,
        );

        assert_eq!(format!("{lower}{upper}"), "stion\n\nponder");
    }

    #[test]
    fn cross_message_selection_spans_thinking_in_both_directions() {
        let (document, keys) = thinking_document();

        for endpoint_keys in [[keys[0], keys[2]], [keys[2], keys[0]]] {
            let lower = copy(
                &document,
                TextSelectionCoverage::ToEnd,
                Some("stion"),
                0,
                endpoint_keys,
            );
            let upper = copy(
                &document,
                TextSelectionCoverage::FromStart,
                Some("tha"),
                4,
                endpoint_keys,
            );

            assert_eq!(
                format!("{lower}{upper}"),
                "stion\n\npondering deeply\n\nthe answer\ntail\n\ntha"
            );
        }
    }

    #[test]
    fn select_all_payload_includes_thinking_exactly_once() {
        let (document, keys) = thinking_document();

        let (payload_keys, text) = document
            .select_all_payload()
            .expect("thinking document yields a select-all payload");

        assert_eq!(payload_keys, keys);
        assert_eq!(
            text,
            "question\n\npondering deeply\n\nthe answer\ntail\n\nthanks"
        );
        assert_eq!(text.matches("pondering deeply").count(), 1);
    }

    #[test]
    fn unmounted_thinking_message_between_mounted_endpoints_copies_completely() {
        // Endpoints live in messages 0 and 2; the thinking-bearing message 1
        // is fully virtualized between them and must reconstruct without
        // truncation or duplication, in both drag directions.
        let (document, keys) = thinking_document();

        for endpoint_keys in [[keys[0], keys[2]], [keys[2], keys[0]]] {
            let lower = copy(
                &document,
                TextSelectionCoverage::ToEnd,
                Some("question"),
                0,
                endpoint_keys,
            );
            let upper = copy(
                &document,
                TextSelectionCoverage::FromStart,
                Some("thanks"),
                4,
                endpoint_keys,
            );

            assert_eq!(
                format!("{lower}{upper}"),
                "question\n\npondering deeply\n\nthe answer\ntail\n\nthanks"
            );
        }
    }

    #[test]
    fn endpoint_inside_thinking_reconstructs_the_rest_of_its_message() {
        let (document, keys) = thinking_document();
        let endpoint_keys = [keys[1], keys[2]];

        // Anchor inside the thinking leaf; cursor in the last message. The
        // virtualized remainder of the anchor's own message (content leaves)
        // must follow the selected thinking suffix.
        let lower = copy(
            &document,
            TextSelectionCoverage::ToEnd,
            Some("ing deeply"),
            1,
            endpoint_keys,
        );
        let upper = copy(
            &document,
            TextSelectionCoverage::FromStart,
            Some("thank"),
            4,
            endpoint_keys,
        );

        assert_eq!(
            format!("{lower}{upper}"),
            "ing deeply\n\nthe answer\ntail\n\nthank"
        );
    }
}
