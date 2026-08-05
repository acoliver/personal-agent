//! Model-generated conversation titles.
//!
//! A freshly created conversation carries a placeholder title such as
//! `"New Conversation"`. Once the user sends the first prompt we ask the model for a
//! short descriptive title and use it instead, so the conversation list is navigable.
//!
//! This module owns the parts of that feature that are independent of the chat stream:
//! deciding whether a title is still a placeholder, building the request sent to the
//! model, and turning a raw model response into a title that is safe to display.

use async_trait::async_trait;

use crate::llm::{LlmClient, Message as LlmMessage};
use crate::models::ModelProfile;
use crate::services::{ServiceError, ServiceResult};

/// Maximum length of a generated title, in characters.
///
/// Conversation rows are narrow, so anything longer is dead weight.
pub const MAX_TITLE_CHARS: usize = 60;

/// Maximum number of characters of the first prompt sent to the model.
///
/// A title only needs the opening of the prompt; sending a multi-megabyte paste again
/// would make auto-naming more expensive than the conversation itself.
pub const MAX_PROMPT_CHARS: usize = 2000;

/// Placeholder titles assigned by the app rather than chosen by the user.
const PLACEHOLDER_TITLES: [&str; 3] = ["new conversation", "untitled conversation", "untitled"];

/// Width of the timestamp title produced by `Conversation::new` (`%Y%m%d%H%M%S%3f`).
const TIMESTAMP_TITLE_LEN: usize = 17;

/// Characters stripped from both ends of a candidate title.
///
/// Covers markdown emphasis/heading/bullet/quote decoration plus the quote characters
/// models like to wrap titles in.
const TITLE_TRIM_CHARS: &str = " \t#*_`>-\"\u{27}\u{201C}\u{201D}\u{2018}\u{2019}\u{AB}\u{BB}";

/// Sentence punctuation stripped from the end of a candidate title.
const TITLE_TRAILING_PUNCTUATION: &str = ".,;:!?";

/// Label prefixes models emit before the actual title, matched case-insensitively.
const TITLE_LABEL_PREFIXES: [&str; 3] = ["conversation title:", "title:", "chat title:"];

/// Thinking blocks some models leak into the response body.
///
/// Ordered longest-tag-first: `</think>` is a prefix of `</thinking>`, so stripping the
/// short pair first would cut a `<thinking>` block in the wrong place.
const THINKING_TAGS: [(&str, &str); 2] = [("<thinking>", "</thinking>"), ("<think>", "</think>")];

/// Asks a model to name a conversation.
///
/// This is the transport seam between the chat service and the LLM: implementations
/// return the model's answer verbatim, and the caller decides what is usable. Keeping
/// it that way lets tests drive the whole auto-naming path, sanitization included,
/// without a network call.
#[async_trait]
pub trait ConversationTitleGenerator: Send + Sync {
    /// Ask the model to title a conversation opening with `first_prompt`.
    ///
    /// The returned string is the raw model response and still needs
    /// [`sanitize_generated_title`] before it is shown or stored.
    ///
    /// # Errors
    ///
    /// Returns a `ServiceError` when the model cannot be reached.
    async fn propose_title(
        &self,
        profile: &ModelProfile,
        first_prompt: &str,
    ) -> ServiceResult<String>;
}

/// Real generator: asks the conversation's own model for a title.
pub struct LlmConversationTitleGenerator;

#[async_trait]
impl ConversationTitleGenerator for LlmConversationTitleGenerator {
    async fn propose_title(
        &self,
        profile: &ModelProfile,
        first_prompt: &str,
    ) -> ServiceResult<String> {
        let client = LlmClient::from_profile(profile).map_err(|error| {
            ServiceError::Configuration(format!(
                "Failed to create LLM client for title generation: {error}"
            ))
        })?;

        let response = client
            .request(&build_title_request_messages(first_prompt))
            .await
            .map_err(|error| {
                ServiceError::Network(format!("Title generation request failed: {error}"))
            })?;

        Ok(response.content)
    }
}

/// Generator that never proposes a title.
///
/// Used by `ChatServiceImpl::new_for_tests` so that test harnesses exercising the chat
/// stream do not also fire a live title request at a provider. Tests that cover
/// auto-naming inject their own generator instead.
pub struct DisabledConversationTitleGenerator;

#[async_trait]
impl ConversationTitleGenerator for DisabledConversationTitleGenerator {
    async fn propose_title(
        &self,
        _profile: &ModelProfile,
        _first_prompt: &str,
    ) -> ServiceResult<String> {
        Err(ServiceError::Configuration(
            "Conversation title generation is disabled".to_string(),
        ))
    }
}

/// Whether `title` is an app-assigned placeholder rather than a real title.
///
/// Only placeholders are eligible for auto-naming; a title the user (or a previous
/// generation) chose is never overwritten.
#[must_use]
pub fn is_placeholder_title(title: Option<&str>) -> bool {
    let Some(title) = title else {
        return true;
    };

    let trimmed = title.trim();
    if trimmed.is_empty() {
        return true;
    }

    if PLACEHOLDER_TITLES.contains(&trimmed.to_lowercase().as_str()) {
        return true;
    }

    is_default_timestamp_title(trimmed)
}

/// The `Conversation::new` fallback title is a bare `%Y%m%d%H%M%S%3f` stamp.
fn is_default_timestamp_title(title: &str) -> bool {
    title.len() == TIMESTAMP_TITLE_LEN && title.bytes().all(|byte| byte.is_ascii_digit())
}

/// Build the messages sent to the model to obtain a title.
///
/// The first prompt is truncated so the extra request stays cheap.
#[must_use]
pub fn build_title_request_messages(first_prompt: &str) -> Vec<LlmMessage> {
    let excerpt = truncate_chars(first_prompt.trim(), MAX_PROMPT_CHARS);

    vec![
        LlmMessage::system(format!(
            "You name chat conversations. Reply with the title only: a single line of \
             plain text, at most {MAX_TITLE_CHARS} characters, no quotes, no markdown, \
             no trailing punctuation, no explanation."
        )),
        LlmMessage::user(format!(
            "Write a short, specific title for a conversation that opens with this \
             message:\n\n{excerpt}"
        )),
    ]
}

/// Turn a raw model response into a title safe to display, or `None` if unusable.
///
/// Models wrap titles in quotes, prefix them with `Title:`, format them as markdown
/// headings, precede them with a sentence of preamble, or leak `<think>` blocks. All of
/// that is stripped here rather than shown to the user.
#[must_use]
pub fn sanitize_generated_title(raw: &str) -> Option<String> {
    let body = strip_thinking_blocks(raw);
    let candidate = body.lines().map(str::trim).find(|line| !line.is_empty())?;

    // Decoration is trimmed before the label strip so a decorated label such as
    // `**Title: Foo**` is handled as well as a bare `Title: Foo`. Trailing punctuation
    // is only stripped afterwards, otherwise a bare `Title:` would survive as `Title`.
    let candidate = trim_title_decoration(candidate);
    let candidate = strip_label_prefix(candidate);
    let candidate = trim_decoration_and_punctuation(candidate);

    let collapsed = candidate.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = trim_decoration_and_punctuation(&collapsed);

    if trimmed.is_empty() {
        return None;
    }

    Some(truncate_on_word_boundary(trimmed, MAX_TITLE_CHARS))
}

/// Strip decoration from both ends and sentence punctuation from the end, repeatedly.
///
/// One pass is not enough: models produce endings such as `loss".` where the quote is
/// only reachable once the full stop after it has been removed.
fn trim_decoration_and_punctuation(candidate: &str) -> &str {
    let mut current = candidate;
    loop {
        let next = trim_title_decoration(current)
            .trim_end_matches(|character| TITLE_TRAILING_PUNCTUATION.contains(character));
        if next.len() == current.len() {
            return next;
        }
        current = next;
    }
}

/// Strip decoration characters from both ends of a candidate title.
fn trim_title_decoration(candidate: &str) -> &str {
    candidate.trim_matches(|character| TITLE_TRIM_CHARS.contains(character))
}

/// Remove a leading `Title:`-style label, matched case-insensitively.
fn strip_label_prefix(candidate: &str) -> &str {
    let lowered = candidate.to_ascii_lowercase();
    for prefix in TITLE_LABEL_PREFIXES {
        if lowered.starts_with(prefix) {
            return candidate[prefix.len()..].trim_start();
        }
    }
    candidate
}

/// Drop `<think>`/`<thinking>` blocks from a model response.
fn strip_thinking_blocks(raw: &str) -> String {
    let mut body = raw.to_string();
    for (open, close) in THINKING_TAGS {
        body = strip_tag_pairs(&body, open, close);
    }
    body
}

/// Remove every `open`..`close` span from `input`.
///
/// An unterminated `open` tag discards the rest of the input: the model never closed the
/// block, so nothing after it is answer text.
fn strip_tag_pairs(input: &str, open: &str, close: &str) -> String {
    // `to_ascii_lowercase` is byte-length preserving, so offsets found in the lowered
    // copy are valid in `input`.
    let lowered = input.to_ascii_lowercase();
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;

    while let Some(relative_open) = lowered[cursor..].find(open) {
        let open_at = cursor + relative_open;
        output.push_str(&input[cursor..open_at]);

        let after_open = open_at + open.len();
        let Some(relative_close) = lowered[after_open..].find(close) else {
            return output;
        };
        cursor = after_open + relative_close + close.len();
    }

    output.push_str(&input[cursor..]);
    output
}

/// Truncate to at most `max_chars` characters, respecting char boundaries.
fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Truncate to at most `max_chars` characters, preferring to cut at a word boundary.
///
/// A hard cut is used when the truncated text has no space to back off to, or when
/// backing off would discard most of the title.
fn truncate_on_word_boundary(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let hard_cut = truncate_chars(text, max_chars);
    let Some(last_space) = hard_cut.rfind(' ') else {
        return hard_cut;
    };

    let word_cut = hard_cut[..last_space].trim_end();
    if word_cut.chars().count() * 2 < max_chars {
        return hard_cut;
    }

    word_cut.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_blank_and_app_assigned_titles_are_placeholders() {
        assert!(is_placeholder_title(None));
        assert!(is_placeholder_title(Some("")));
        assert!(is_placeholder_title(Some("   \t ")));
        assert!(is_placeholder_title(Some("New Conversation")));
        assert!(is_placeholder_title(Some("  new conversation  ")));
        assert!(is_placeholder_title(Some("NEW CONVERSATION")));
        assert!(is_placeholder_title(Some("Untitled Conversation")));
        assert!(is_placeholder_title(Some("Untitled")));
    }

    #[test]
    fn default_timestamp_title_is_a_placeholder() {
        let generated = chrono::Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
        assert_eq!(generated.len(), TIMESTAMP_TITLE_LEN);
        assert!(is_placeholder_title(Some(&generated)));
    }

    #[test]
    fn user_chosen_titles_are_not_placeholders() {
        assert!(!is_placeholder_title(Some("Rust build failures")));
        assert!(!is_placeholder_title(Some("New Conversation about GPUI")));
        assert!(!is_placeholder_title(Some("2026")));
        // Same length as the timestamp default, but not all digits.
        assert!(!is_placeholder_title(Some("2026080520084312a")));
    }

    #[test]
    fn title_request_truncates_a_huge_first_prompt() {
        let prompt = "x".repeat(MAX_PROMPT_CHARS * 3);
        let messages = build_title_request_messages(&prompt);

        assert_eq!(messages.len(), 2);
        let user_content = &messages[1].content;
        assert!(
            user_content.ends_with(&"x".repeat(MAX_PROMPT_CHARS)),
            "the first {MAX_PROMPT_CHARS} prompt characters should be resent"
        );
        assert!(
            !user_content.contains(&"x".repeat(MAX_PROMPT_CHARS + 1)),
            "no more than {MAX_PROMPT_CHARS} prompt characters should be resent"
        );
    }

    #[test]
    fn title_request_carries_the_prompt_and_a_length_instruction() {
        let messages = build_title_request_messages("  How do I profile a GPUI view?  ");

        assert!(messages[0].content.contains(&MAX_TITLE_CHARS.to_string()));
        assert!(messages[1]
            .content
            .contains("How do I profile a GPUI view?"));
    }

    #[test]
    fn plain_title_survives_sanitization() {
        assert_eq!(
            sanitize_generated_title("Debugging GPUI focus loss"),
            Some("Debugging GPUI focus loss".to_string())
        );
    }

    #[test]
    fn wrapping_quotes_are_removed() {
        assert_eq!(
            sanitize_generated_title("\"Debugging GPUI focus loss\""),
            Some("Debugging GPUI focus loss".to_string())
        );
        assert_eq!(
            sanitize_generated_title("“Debugging GPUI focus loss”"),
            Some("Debugging GPUI focus loss".to_string())
        );
        assert_eq!(
            sanitize_generated_title("'Debugging GPUI focus loss'"),
            Some("Debugging GPUI focus loss".to_string())
        );
    }

    #[test]
    fn markdown_decoration_is_removed() {
        assert_eq!(
            sanitize_generated_title("## Debugging GPUI focus loss"),
            Some("Debugging GPUI focus loss".to_string())
        );
        assert_eq!(
            sanitize_generated_title("**Debugging GPUI focus loss**"),
            Some("Debugging GPUI focus loss".to_string())
        );
        assert_eq!(
            sanitize_generated_title("- Debugging GPUI focus loss"),
            Some("Debugging GPUI focus loss".to_string())
        );
        assert_eq!(
            sanitize_generated_title("`Debugging GPUI focus loss`"),
            Some("Debugging GPUI focus loss".to_string())
        );
    }

    #[test]
    fn label_prefix_is_removed_with_and_without_decoration() {
        assert_eq!(
            sanitize_generated_title("Title: Debugging GPUI focus loss"),
            Some("Debugging GPUI focus loss".to_string())
        );
        assert_eq!(
            sanitize_generated_title("**Conversation Title: Debugging GPUI focus loss**"),
            Some("Debugging GPUI focus loss".to_string())
        );
        assert_eq!(
            sanitize_generated_title("title: \"Debugging GPUI focus loss\""),
            Some("Debugging GPUI focus loss".to_string())
        );
    }

    #[test]
    fn preamble_lines_are_discarded_in_favour_of_the_first_content_line() {
        assert_eq!(
            sanitize_generated_title("\n\n   \nDebugging GPUI focus loss\n\nLet me know!"),
            Some("Debugging GPUI focus loss".to_string())
        );
    }

    #[test]
    fn terminated_thinking_blocks_are_discarded() {
        assert_eq!(
            sanitize_generated_title(
                "<think>The user wants a title.\nSomething about focus.</think>\nGPUI focus loss"
            ),
            Some("GPUI focus loss".to_string())
        );
        assert_eq!(
            sanitize_generated_title("<THINKING>noise</THINKING>GPUI focus loss"),
            Some("GPUI focus loss".to_string())
        );
    }

    #[test]
    fn unterminated_thinking_block_yields_no_title() {
        assert_eq!(
            sanitize_generated_title("<think>I am still reasoning about this"),
            None
        );
    }

    #[test]
    fn trailing_punctuation_and_inner_whitespace_are_normalized() {
        assert_eq!(
            sanitize_generated_title("Debugging   GPUI\tfocus loss!!!"),
            Some("Debugging GPUI focus loss".to_string())
        );
    }

    #[test]
    fn empty_and_decoration_only_responses_are_rejected() {
        assert_eq!(sanitize_generated_title(""), None);
        assert_eq!(sanitize_generated_title("   \n\t  \n"), None);
        assert_eq!(sanitize_generated_title("\"\""), None);
        assert_eq!(sanitize_generated_title("Title:"), None);
        assert_eq!(sanitize_generated_title("**...**"), None);
    }

    #[test]
    fn over_long_titles_are_capped_at_a_word_boundary() {
        let raw = "Investigating why the GPUI conversation sidebar loses keyboard focus \
                   after switching profiles";

        let title = sanitize_generated_title(raw).expect("title should be usable");

        assert!(
            title.chars().count() <= MAX_TITLE_CHARS,
            "title was {} chars: {title}",
            title.chars().count()
        );
        assert!(
            raw.starts_with(&title),
            "capped title should be a prefix of the response: {title}"
        );
        assert!(
            !title.ends_with(' '),
            "capped title should not end with whitespace: {title}"
        );
    }

    #[test]
    fn over_long_single_word_titles_are_hard_cut() {
        let raw = "A".repeat(MAX_TITLE_CHARS * 2);

        let title = sanitize_generated_title(&raw).expect("title should be usable");

        assert_eq!(title.chars().count(), MAX_TITLE_CHARS);
    }

    #[test]
    fn multi_byte_titles_are_capped_without_splitting_characters() {
        let raw = "日本語".repeat(MAX_TITLE_CHARS);

        let title = sanitize_generated_title(&raw).expect("title should be usable");

        assert_eq!(title.chars().count(), MAX_TITLE_CHARS);
        assert!(title.chars().all(|c| "日本語".contains(c)));
    }
}
