# History View Requirements

The History View displays saved conversations and allows users to load or delete them. **The view is purely presentational** - it renders data from ConversationService and forwards user actions.

---

## Visual Reference

```
┌──────────────────────────────────────────────────────────────┐
│ TOP BAR (44px, dark background #1a1a1a)                      │
│                                                              │
│  [<]  History                                                │
│  28px  14pt bold                                             │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ HISTORY SCROLL AREA (flex height, #121212 background)        │
│                                                              │
│  12px padding                                                │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Chat about Rust async patterns                         │  │
│  │ Today, 2:30 PM • 47 messages                           │  │
│  │                                          [Load][Delete]│  │
│  └────────────────────────────────────────────────────────┘  │
│   8px gap                                                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Research on MCP architecture                           │  │
│  │ Yesterday, 10:15 AM • 23 messages                      │  │
│  │                                          [Load][Delete]│  │
│  └────────────────────────────────────────────────────────┘  │
│   8px gap                                                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Untitled Conversation                                  │  │
│  │ Jan 19, 2025 • 5 messages                              │  │
│  │                                          [Load][Delete]│  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
└──────────────────────────────────────────────────────────────┘

── EMPTY STATE (when no conversations) ──────────────────────────

┌──────────────────────────────────────────────────────────────┐
│ TOP BAR (44px)                                               │
│  [<]  History                                                │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│                                                              │
│                                                              │
│                  No saved conversations                      │
│                        14pt, #888888                         │
│                                                              │
│              Start chatting to create history                │
│                        12pt, #666666                         │
│                                                              │
│                                                              │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## Layout Specifications

### Overall Dimensions

| Property | Value | Notes |
|----------|-------|-------|
| Popover width | 400px | Same as Chat View |
| Popover height | 500px | Same as Chat View |
| Background | #121212 | Theme.BG_BASE |

### Spacing Standards

| Context | Value | Notes |
|---------|-------|-------|
| Content padding | 12px | All edges of scroll area |
| Card spacing | 8px | Between cards |
| Card internal padding | 12px | All sides inside card |
| Button spacing | 8px | Between Load and Delete |

### Typography

| Element | Font | Size | Color |
|---------|------|------|-------|
| "History" title | System Bold | 14pt | #e5e5e5 |
| Card title | System Bold | 13pt | #e5e5e5 |
| Card metadata | System Regular | 11pt | #888888 |
| Button labels | System Medium | 12pt | #e5e5e5 |
| Empty primary | System Regular | 14pt | #888888 |
| Empty secondary | System Regular | 12pt | #666666 |

---

## Component Requirements

### Top Bar

**Layout:** Horizontal stack, 44px height, #1a1a1a background

```
[12px] [<] [8px] [History] [spacer] [12px]
```

| ID | Element | Spec | Behavior |
|----|---------|------|----------|
| TB-1 | Back button | 28x28, "<" label | Navigate to Chat View |
| TB-2 | Title label | "History", 14pt bold | Static |
| TB-3 | Layout | Left-aligned | Back + title flush left |
| TB-4 | Button style | Borderless, hover highlight | Consistent with Chat View |

### Conversation Cards

**Layout:** Vertical stack inside card, full width

```
┌─────────────────────────────────────────────────────┐
│ Title (bold, single line, truncate with ...)        │  ← Line 1
│ Date • N messages                                   │  ← Line 2
│                                       [Load][Delete]│  ← Line 3
└─────────────────────────────────────────────────────┘
     12px padding all sides
```

| ID | Element | Spec |
|----|---------|------|
| CC-1 | Container | Full width minus 24px (padding), #1a1a1a |
| CC-2 | Corner radius | 8px all corners |
| CC-3 | Padding | 12px all sides |
| CC-4 | Title label | 13pt bold, #e5e5e5 |
| CC-5 | Title truncation | Single line, truncate tail with "..." |
| CC-6 | Title fallback | "Untitled Conversation" when title is None/empty |
| CC-7 | Metadata label | 11pt, #888888 |
| CC-8 | Metadata format | "{date} . {count} messages" |
| CC-9 | Button row | HStack, right-aligned |
| CC-10 | [Load] button | 60px wide, 28px tall, standard style |
| CC-11 | [Delete] button | 60px wide, 28px tall, standard style |
| CC-12 | Delete hover | Red tint background (#4a2a2a) on hover |
| CC-13 | Button gap | 8px between Load and Delete |
| CC-14 | Row spacing | 4px between title, metadata, buttons |

### Empty State

**Layout:** Centered vertically and horizontally in scroll area

| ID | Element | Spec |
|----|---------|------|
| ES-1 | Visibility | When conversation count = 0 |
| ES-2 | Container | VStack, centered alignment |
| ES-3 | Primary text | "No saved conversations" |
| ES-4 | Primary style | 14pt, #888888, centered |
| ES-5 | Secondary text | "Start chatting to create history" |
| ES-6 | Secondary style | 12pt, #666666, centered |
| ES-7 | Text spacing | 8px between primary and secondary |

---

## Behavioral Requirements

### View Loading Flow

| Step | Action | Visual Feedback |
|------|--------|-----------------|
| 1 | View appears | |
| 2 | Call ConversationService.list() | |
| 3 | Sort by created_at descending | Newest first |
| 4a | If empty | Show empty state |
| 4b | If has data | Render cards |
| 5 | Cards appear | Instant, no animation |

### Load Conversation Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [Load] on card | |
| 2 | | Get conversation ID from card data |
| 3 | | Navigate to Chat View |
| 4 | | Post LoadConversation notification with ID |
| 5 | | Chat View receives notification |
| 6 | | Chat View calls ConversationService.load(id) |
| 7 | | Chat View displays messages |

### Delete Conversation Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [Delete] on card | |
| 2 | | Show confirmation alert (NSAlert) |
| 3 | | Alert style: Warning |
| 4 | | Alert title: "Delete Conversation?" |
| 5 | | Alert message: "Delete '{title}'? This cannot be undone." |
| 6 | | [Cancel] button (default) |
| 7 | | [Delete] button (destructive style) |
| 8a | Click [Cancel] | Alert dismisses, no action |
| 8b | Click [Delete] | |
| 9 | | Call ConversationService.delete(id) |
| 10 | | Animate card out (fade + collapse) |
| 11 | | If was last card, show empty state |

### Back Navigation Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [<] back button | |
| 2 | | Navigate to Chat View |
| 3 | | Chat View shows current/last conversation |

---

## Date Formatting

| Condition | Format | Example |
|-----------|--------|---------|
| Today | "Today, h:mm a" | "Today, 2:30 PM" |
| Yesterday | "Yesterday, h:mm a" | "Yesterday, 10:15 AM" |
| This year | "MMM d, h:mm a" | "Jan 21, 2:30 PM" |
| Past year | "MMM d, yyyy" | "Dec 15, 2024" |

**Message count format:** "{N} messages" (always plural form for simplicity, or "1 message" if exactly 1)

---

## State Management

### View State

| Field | Type | Purpose |
|-------|------|---------|
| conversations | Vec<ConversationMetadata> | Display data from service |

### UI References

| Field | Type | Purpose |
|-------|------|---------|
| scroll_view | NSScrollView | Content scroll container |
| content_stack | NSStackView | Cards container |
| empty_state_view | NSView | Empty state container |
| card_map | HashMap<Uuid, NSView> | For removal on delete |

### Conversation Metadata (from Service)

```rust
struct ConversationMetadata {
    id: Uuid,
    title: Option<String>,
    created_at: DateTime<Utc>,
    message_count: usize,
}
```

---

## Service Dependencies

| Action | Service | Method |
|--------|---------|--------|
| List conversations | ConversationService | list() |
| Delete conversation | ConversationService | delete(id) |
| Load conversation | Via notification | Chat View handles |

---

## Known Issues

1. **No search/filter** - Can't find specific conversations by keyword
2. **No sorting options** - Always newest first, no toggle
3. **Performance** - May need pagination for 100+ conversations
4. **Card click area** - Only buttons respond, not full card click
5. **No multi-select** - Can't bulk delete

---

## Test Coverage

### Visual Tests

- [ ] Cards show title, date, message count
- [ ] Empty state centered when no conversations
- [ ] Delete button has red tint on hover
- [ ] Cards have correct spacing (8px)
- [ ] Long titles truncate with "..."

### Interaction Tests

- [ ] [<] navigates to Chat View
- [ ] [Load] opens conversation in Chat View
- [ ] [Delete] shows confirmation alert
- [ ] Cancel in confirmation dismisses alert
- [ ] Confirm delete removes card with animation
- [ ] Last card deleted transitions to empty state

### Data Tests

- [ ] Conversations sorted newest first
- [ ] "Untitled Conversation" fallback works
- [ ] Date formatting correct for today/yesterday/other
- [ ] Message count displays correctly
- [ ] "1 message" vs "N messages" grammar
