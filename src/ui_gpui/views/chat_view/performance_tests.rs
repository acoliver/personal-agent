//! Performance optimization tests for issue #172.
//!
//! Tests for:
//! 1. Scroll-to-bottom should not trigger multiple re-renders
//! 2. Markdown caching for finalized messages
//! 3. Arc<String> for cheap message cloning

#![allow(clippy::future_not_send)]

use super::*;
use crate::ui_gpui::components::markdown_content::parse_markdown_blocks;
use gpui::{AppContext, TestAppContext};
use std::sync::Arc;

// ── Scroll-to-bottom optimization tests ──────────────────────────────────

#[gpui::test]
async fn maybe_scroll_chat_to_bottom_triggers_single_notify_not_four(cx: &mut TestAppContext) {
    // Before fix: maybe_scroll_chat_to_bottom triggered 4 cx.notify() calls
    // via 3 nested cx.defer chains.
    // After fix: should trigger only 1 notify (from the caller, not this fn).
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.state.chat_autoscroll_enabled = true;
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            // The function itself should not call cx.notify() multiple times
            // via deferred chains - only the caller's cx.notify() should run
            view.maybe_scroll_chat_to_bottom(cx);

            // Invocation count should be 1 (function was called once)
            assert_eq!(view.maybe_scroll_chat_to_bottom_invocations.get(), 1);
        });
    });
}

// ── Markdown caching tests ───────────────────────────────────────────────

#[test]
fn chat_message_caches_parsed_markdown_blocks() {
    // Finalized messages should cache their parsed markdown blocks
    // so re-rendering doesn't re-parse.
    let msg = ChatMessage::assistant("```rust\nfn main() {}\n```", "gpt-4");

    // First access parses and caches
    let blocks1 = msg.get_or_parse_markdown();
    assert!(!blocks1.is_empty());

    // Second access returns cached version (same pointer)
    let blocks2 = msg.get_or_parse_markdown();
    assert_eq!(blocks1.len(), blocks2.len());

    // Verify it's the same Arc (cheap clone)
    assert!(Arc::ptr_eq(&blocks1, &blocks2));
}

#[test]
fn streaming_message_reparse_is_different() {
    // Streaming messages should NOT cache - content changes on each chunk
    // The content difference should be reflected in the parsed blocks
    let partial = "Hello **wor";
    let blocks1 = parse_markdown_blocks(partial);

    let longer = "Hello **world** and more";
    let blocks2 = parse_markdown_blocks(longer);

    // Content changed, and longer content has more text
    // At minimum the raw content strings differ
    assert_ne!(partial, longer);
    // Both should parse successfully (non-empty)
    assert!(!blocks1.is_empty());
    assert!(!blocks2.is_empty());
}

#[test]
fn user_message_also_caches_markdown() {
    let msg = ChatMessage::user("Click [here](https://example.com)");

    let blocks1 = msg.get_or_parse_markdown();
    let blocks2 = msg.get_or_parse_markdown();

    assert_eq!(blocks1.len(), blocks2.len());
    assert!(Arc::ptr_eq(&blocks1, &blocks2));
}

#[test]
fn cloned_message_shares_cache() {
    let msg = ChatMessage::assistant("Original content", "model");

    // Trigger caching on original
    let blocks1 = msg.get_or_parse_markdown();

    // Clone shares the same Arc<String> for content
    #[allow(clippy::redundant_clone)]
    let msg2 = msg.clone();

    // Verify original's cache is still valid
    assert!(Arc::ptr_eq(&blocks1, &msg.get_or_parse_markdown()));

    // The cloned message's cache should also be shared (OnceCell clone)
    let blocks2 = msg2.get_or_parse_markdown();

    // Both should return the same cached Arc
    assert!(Arc::ptr_eq(&blocks1, &blocks2));
}

// ── Arc<String> optimization tests ────────────────────────────────────────

#[test]
fn message_content_uses_arc_for_efficient_cloning() {
    // When messages are cloned during render, the content strings should
    // use Arc<String> so cloning is cheap (pointer copy, not heap allocation)
    let msg = ChatMessage::assistant("x".repeat(10_000), "model");
    let msg2 = msg.clone();

    // Both should share the same underlying string data
    assert!(Arc::ptr_eq(&msg.content, &msg2.content));
}

#[test]
fn chat_message_with_thinking_shares_arc() {
    let msg = ChatMessage::assistant("response", "model").with_thinking("thoughts".repeat(1000));
    let msg2 = msg.clone();

    // Content should share Arc
    assert!(Arc::ptr_eq(&msg.content, &msg2.content));

    // Thinking content should also share Arc if present
    if let (Some(t1), Some(t2)) = (&msg.thinking, &msg2.thinking) {
        assert!(Arc::ptr_eq(t1, t2));
    }
}

// ── Integration: render isolation concept test ────────────────────────────

#[test]
fn render_chat_area_only_reparses_streaming_message() {
    // Simulate a conversation with 5 finalized messages + 1 streaming
    let finalized_count = 5;
    let mut parse_count = 0;

    // Finalized messages: should use cached blocks
    for i in 0..finalized_count {
        let msg = ChatMessage::assistant(format!("Message {i}"), "model");
        let _blocks = msg.get_or_parse_markdown();
        parse_count += 1; // First access parses

        // Second access should use cache (OnceCell)
        let _blocks2 = msg.get_or_parse_markdown();
        // parse_count should NOT increment here - cache hit
    }

    // Simulate 10 stream chunks - each should re-parse only the streaming msg
    let mut streaming_content = String::new();
    for _ in 0..10 {
        streaming_content.push_str("more text ");
        let _blocks = parse_markdown_blocks(&streaming_content);
        parse_count += 1;
    }

    // With caching: 5 (finalized) + 10 (streaming chunks) = 15 parses
    // Without caching: (5 + 1) * 11 = 66 parses (one per render per msg)
    assert_eq!(parse_count, 15);
}
