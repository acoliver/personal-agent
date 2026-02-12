# Phase 03: Chat View - Layout

**Phase ID:** PLAN-20250130-GPUIREDUX.P03
**Depends On:** P02a
**Estimated Effort:** 4-6 hours

## Objective

Build the Chat View UI structure exactly matching the mockup. This phase focuses on layout only - no event handling or state management.

## Reference

See `dev-docs/requirements/ui/chat.md` and `project-plans/initial/ui-mockup-v2.html`

## Test-First Requirements

### Layout Structure Tests

```rust
// tests/ui_gpui/views/chat_view_layout_tests.rs

#[gpui::test]
fn test_chat_view_has_top_bar(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    
    // Top bar should be 44px height with #1a1a1a background
    // Contains: icon (24x24), title "PersonalAgent", buttons [T][S][H][+][Settings]
}

#[gpui::test]
fn test_chat_view_has_title_bar(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    
    // Title bar should be 32px height
    // Contains: conversation dropdown (200px min), model label
}

#[gpui::test]
fn test_chat_view_has_chat_area(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    
    // Chat area should be scrollable with #121212 background
    // Flexible height between title bar and input bar
}

#[gpui::test]
fn test_chat_view_has_input_bar(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    
    // Input bar should be 50px height with #1a1a1a background
    // Contains: text field (flex), Send button (60px), Stop button (60px)
}

#[gpui::test]
fn test_chat_view_empty_state_shows_message(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    
    // When no messages, shows "No messages yet" centered
}
```

### Message Bubble Tests

```rust
// tests/ui_gpui/components/message_bubble_tests.rs

#[gpui::test]
fn test_user_bubble_right_aligned(cx: &mut TestAppContext) {
    let bubble = MessageBubble::user("Hello world");
    
    // Should be right-aligned with green background (#2a4a2a)
    // Max width 300px, 12px radius, 10px padding
}

#[gpui::test]
fn test_assistant_bubble_left_aligned(cx: &mut TestAppContext) {
    let bubble = MessageBubble::assistant("Response text", "claude-sonnet-4");
    
    // Should be left-aligned with dark background (#1a1a1a)
    // Max width 300px, 12px radius, 10px padding
    // Model label above bubble in muted text
}

#[gpui::test]
fn test_assistant_bubble_shows_model_label(cx: &mut TestAppContext) {
    let bubble = MessageBubble::assistant("Text", "gpt-4o");
    
    // Model label "gpt-4o" should appear above the bubble
    // 10pt font, #888888 color
}

#[gpui::test]
fn test_message_bubble_wraps_long_text(cx: &mut TestAppContext) {
    let long_text = "A".repeat(500);
    let bubble = MessageBubble::user(&long_text);
    
    // Text should wrap within 300px max width
}
```

### Thinking Block Tests

```rust
// tests/ui_gpui/components/thinking_block_tests.rs

#[gpui::test]
fn test_thinking_block_has_blue_tint(cx: &mut TestAppContext) {
    let block = ThinkingBlock::new("Thinking content...");
    
    // Background should be #1a1a2a (blue tint)
    // 8px radius, 8px padding
}

#[gpui::test]
fn test_thinking_block_collapsible(cx: &mut TestAppContext) {
    let block = cx.new(|cx| ThinkingBlock::new("Content"));
    
    block.read(cx, |b, _| {
        assert!(b.is_expanded()); // Default expanded
    });
    
    block.update(cx, |b, cx| {
        b.toggle_collapsed(cx);
    });
    
    block.read(cx, |b, _| {
        assert!(!b.is_expanded()); // Now collapsed
    });
}

#[gpui::test]
fn test_thinking_block_header_shows_arrow(cx: &mut TestAppContext) {
    let block = ThinkingBlock::new("Content");
    
    // Header should show "down arrow Thinking..." when expanded
    // Header should show "right arrow Thinking..." when collapsed
}
```

### Top Bar Tests

```rust
// tests/ui_gpui/components/top_bar_tests.rs

#[gpui::test]
fn test_top_bar_has_correct_height(cx: &mut TestAppContext) {
    let bar = TopBar::new("PersonalAgent");
    // Should be 44px height
}

#[gpui::test]
fn test_top_bar_icon_is_24x24(cx: &mut TestAppContext) {
    let bar = TopBar::new("PersonalAgent")
        .with_icon("ai_eye.svg");
    // Icon should be 24x24
}

#[gpui::test]
fn test_top_bar_buttons_are_28x28(cx: &mut TestAppContext) {
    let bar = TopBar::new("PersonalAgent")
        .with_button(IconButton::new("T"))
        .with_button(IconButton::new("H"));
    // Each button should be 28x28 with 8px spacing
}
```

## Implementation

### ChatView Layout

```rust
// src/ui_gpui/views/chat_view.rs

use gpui::prelude::*;
use crate::ui_gpui::components::*;
use crate::ui_gpui::theme::Theme;

pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub input_text: String,
    pub is_streaming: bool,
    pub current_model: String,
    pub conversation_title: String,
    pub show_thinking: bool,
}

pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub thinking: Option<String>,
    pub model_id: Option<String>,
}

pub enum MessageRole {
    User,
    Assistant,
}

pub struct ChatView {
    pub state: ChatState,
    focus_handle: FocusHandle,
}

impl ChatView {
    pub fn new(state: ChatState, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus_handle: cx.focus_handle(),
        }
    }

    fn render_top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("chat-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                // Left: icon + title
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .size(px(24.0))
                            .bg(Theme::accent()) // Placeholder for icon
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child("PersonalAgent")
                    )
            )
            .child(
                // Right: buttons [T][S][H][+][Settings]
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(self.render_icon_button("T", self.state.show_thinking, cx))
                    .child(self.render_icon_button("S", false, cx))
                    .child(self.render_icon_button("H", false, cx))
                    .child(self.render_icon_button("+", false, cx))
                    .child(self.render_icon_button("G", false, cx)) // Gear for settings
            )
    }

    fn render_icon_button(&self, label: &str, active: bool, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size(px(28.0))
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when(active, |d| d.bg(Theme::accent()))
            .when(!active, |d| d.bg(gpui::transparent_black()).hover(|s| s.bg(Theme::bg_dark())))
            .text_size(px(14.0))
            .text_color(if active { gpui::white() } else { Theme::text_secondary() })
            .child(label)
    }

    fn render_title_bar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("chat-title-bar")
            .h(px(32.0))
            .w_full()
            .bg(Theme::bg_darker())
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            // Conversation title dropdown (simplified as text for now)
            .child(
                div()
                    .min_w(px(200.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .text_size(px(13.0))
                    .text_color(Theme::text_primary())
                    .cursor_pointer()
                    .child(&self.state.conversation_title)
            )
            // Model label
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_muted())
                    .child(&self.state.current_model)
            )
    }

    fn render_chat_area(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("chat-area")
            .flex_1()
            .w_full()
            .bg(Theme::bg_base())
            .overflow_y_scroll()
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .when(self.state.messages.is_empty(), |d| {
                d.items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(Theme::text_muted())
                            .child("No messages yet")
                    )
            })
            .when(!self.state.messages.is_empty(), |d| {
                d.children(self.state.messages.iter().map(|msg| {
                    self.render_message(msg)
                }))
            })
    }

    fn render_message(&self, msg: &ChatMessage) -> impl IntoElement {
        match msg.role {
            MessageRole::User => self.render_user_message(&msg.content),
            MessageRole::Assistant => self.render_assistant_message(msg),
        }
    }

    fn render_user_message(&self, content: &str) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .justify_end() // Right-align
            .child(
                div()
                    .max_w(px(300.0))
                    .px(px(10.0))
                    .py(px(10.0))
                    .rounded(px(12.0))
                    .bg(Theme::user_bubble()) // #2a4a2a
                    .text_size(px(13.0))
                    .text_color(Theme::text_primary())
                    .child(content.to_string())
            )
    }

    fn render_assistant_message(&self, msg: &ChatMessage) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(2.0))
            // Model label
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(Theme::text_muted())
                    .child(msg.model_id.clone().unwrap_or_default())
            )
            // Thinking block (if present and visible)
            .when(msg.thinking.is_some() && self.state.show_thinking, |d| {
                d.child(self.render_thinking_block(msg.thinking.as_ref().unwrap()))
            })
            // Response bubble
            .child(
                div()
                    .max_w(px(300.0))
                    .px(px(10.0))
                    .py(px(10.0))
                    .rounded(px(12.0))
                    .bg(Theme::assistant_bubble()) // #1a1a1a
                    .border_1()
                    .border_color(Theme::border())
                    .text_size(px(13.0))
                    .text_color(Theme::text_secondary())
                    .child(&msg.content)
            )
    }

    fn render_thinking_block(&self, content: &str) -> impl IntoElement {
        div()
            .max_w(px(300.0))
            .px(px(8.0))
            .py(px(8.0))
            .rounded(px(8.0))
            .bg(Theme::thinking_bg()) // #1a1a2a
            .border_l_2()
            .border_color(Theme::text_muted())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(Theme::text_muted())
                            .child("Thinking")
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(Theme::text_muted())
                            .font_style(FontStyle::Italic)
                            .child(content.to_string())
                    )
            )
    }

    fn render_input_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_streaming = self.state.is_streaming;
        let has_text = !self.state.input_text.is_empty();

        div()
            .id("chat-input-bar")
            .h(px(50.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_t_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            // Input field
            .child(
                div()
                    .flex_1()
                    .h(px(32.0))
                    .px(px(12.0))
                    .rounded(px(6.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .flex()
                    .items_center()
                    .text_size(px(13.0))
                    .child(
                        if self.state.input_text.is_empty() {
                            div()
                                .text_color(Theme::text_muted())
                                .child("Type a message...")
                        } else {
                            div()
                                .text_color(Theme::text_primary())
                                .child(&self.state.input_text)
                        }
                    )
            )
            // Send button (visible when not streaming)
            .when(!is_streaming, |d| {
                d.child(
                    div()
                        .w(px(60.0))
                        .h(px(32.0))
                        .rounded(px(6.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .when(has_text, |d| d.bg(Theme::user_bubble()))
                        .when(!has_text, |d| d.bg(Theme::bg_dark()))
                        .text_size(px(12.0))
                        .text_color(Theme::text_primary())
                        .child("Send")
                )
            })
            // Stop button (visible when streaming)
            .when(is_streaming, |d| {
                d.child(
                    div()
                        .w(px(60.0))
                        .h(px(32.0))
                        .rounded(px(6.0))
                        .bg(Theme::danger()) // #4a2a2a
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_size(px(12.0))
                        .text_color(Theme::text_primary())
                        .child("Stop")
                )
            })
    }
}

impl Focusable for ChatView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("chat-view")
            .size_full()
            .flex()
            .flex_col()
            .bg(Theme::bg_base())
            .child(self.render_top_bar(cx))
            .child(self.render_title_bar(cx))
            .child(self.render_chat_area(cx))
            .child(self.render_input_bar(cx))
    }
}
```

### Theme Updates

```rust
// src/ui_gpui/theme.rs - Add these colors

impl Theme {
    // Existing colors...
    
    pub fn user_bubble() -> Rgba {
        gpui::rgb(0x2a4a2a) // Green tint for user messages
    }
    
    pub fn assistant_bubble() -> Rgba {
        gpui::rgb(0x1a1a1a) // Dark for assistant messages
    }
    
    pub fn thinking_bg() -> Rgba {
        gpui::rgb(0x1a1a2a) // Blue tint for thinking
    }
    
    pub fn danger() -> Rgba {
        gpui::rgb(0x4a2a2a) // Red tint for stop/danger
    }
}
```

## Verification Checklist

- [ ] Top bar is 44px height with correct layout
- [ ] Title bar is 32px height with dropdown + model label
- [ ] Chat area scrolls vertically
- [ ] User messages are right-aligned with green background
- [ ] Assistant messages are left-aligned with dark background
- [ ] Model label appears above assistant messages
- [ ] Thinking blocks have blue tint and are collapsible
- [ ] Input bar is 50px with text field + Send/Stop buttons
- [ ] Empty state shows "No messages yet"
- [ ] All colors match mockup exactly
- [ ] All tests pass

## Files Created/Modified

| File | Action |
|------|--------|
| `src/ui_gpui/views/chat_view.rs` | Create |
| `src/ui_gpui/components/message_bubble.rs` | Create |
| `src/ui_gpui/components/thinking_block.rs` | Create |
| `src/ui_gpui/components/top_bar.rs` | Create |
| `src/ui_gpui/theme.rs` | Update |
| `tests/ui_gpui/views/chat_view_layout_tests.rs` | Create |
| `tests/ui_gpui/components/message_bubble_tests.rs` | Create |
| `tests/ui_gpui/components/thinking_block_tests.rs` | Create |
