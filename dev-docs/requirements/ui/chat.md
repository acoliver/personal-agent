# Chat View Requirements

The Chat View is the main/default view of PersonalAgent. It displays conversations with the AI assistant and handles user input. **The view is purely presentational** - it renders data received from services and forwards user actions to services.

---

## Visual Reference

```
┌──────────────────────────────────────────────────────────────┐
│ TOP BAR (44px, dark background #1a1a1a)                      │
│ ┌────┐                                                       │
│ │ @@ │  PersonalAgent          [T] [S] [H] [+] []          │
│ └────┘                                                       │
│  24px   14pt bold             ←── 28px each, 8px spacing ──→ │
├──────────────────────────────────────────────────────────────┤
│ TITLE BAR (32px, dark background)                            │
│                                                              │
│  ┌─────────────────────────────┐                             │
│  │ My Conversation Title     ▼ │  claude-3-5-sonnet          │
│  └─────────────────────────────┘  (muted text)               │
│   NSPopUpButton (200px min)       Model label                │
│                                                              │
│  ┌─────────────────────────────┐  ← Edit field (hidden by    │
│  │ New title here...           │    default, replaces        │
│  └─────────────────────────────┘    dropdown when active)    │
├──────────────────────────────────────────────────────────────┤
│ CHAT AREA (flex height, scrollable, #121212 background)      │
│                                                              │
│                           ┌──────────────────────────────┐   │
│                           │ User message text here       │   │
│                           │ can wrap to multiple lines   │   │
│                           └──────────────────────────────┘   │
│                            ↑ Right-aligned, green (#2a4a2a)  │
│                              max-width 300px, 12px radius    │
│                                                              │
│  ┌──────────────────────────────┐                            │
│  │ ▼ Thinking...                │ ← Collapsible header       │
│  │ ┌──────────────────────────┐ │                            │
│  │ │ Thinking content here... │ │ ← Blue tint (#1a1a2a)      │
│  │ │ italic, secondary color  │ │   Only if show_thinking    │
│  │ └──────────────────────────┘ │                            │
│  └──────────────────────────────┘                            │
│                                                              │
│  ┌──────────────────────────────┐                            │
│  │ Assistant response text      │                            │
│  │ with streaming cursor▌       │                            │
│  └──────────────────────────────┘                            │
│   ↑ Left-aligned, dark gray (#1a1a1a)                        │
│     max-width 300px, 12px radius                             │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ INPUT BAR (50px, dark background #1a1a1a, top border)        │
│                                                              │
│  ┌────────────────────────────────────┐ ┌──────┐ ┌──────┐   │
│  │ Type a message...                  │ │ Send │ │ Stop │   │
│  └────────────────────────────────────┘ └──────┘ └──────┘   │
│   ↑ Flexible width, 32px height         60px ea, 8px gap    │
│     12px left margin                    right-aligned       │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## Layout Specifications

### Overall Dimensions

| Property | Value | Notes |
|----------|-------|-------|
| Popover width | 400px | Fixed |
| Popover height | 500px | Fixed |
| Background | #121212 | Theme.BG_BASE |

### Spacing Standards

| Context | Value | Notes |
|---------|-------|-------|
| Section padding | 12px | Top bar, input bar edges |
| Button spacing | 8px | Between toolbar buttons |
| Message spacing | 8px | Between chat bubbles |
| Input/button gap | 8px | Between input field and Send |

### Typography

| Element | Font | Size | Color |
|---------|------|------|-------|
| App title | System Bold | 14pt | #e5e5e5 |
| Conversation title | System Regular | 13pt | #e5e5e5 |
| Model label | System Regular | 11pt | #888888 |
| Message text | System Regular | 13pt | #e5e5e5 |
| Thinking text | System Italic | 12pt | #888888 |
| Button labels | System Medium | 12pt | #e5e5e5 |
| Placeholder text | System Regular | 13pt | #666666 |

---

## Component Requirements

### Top Bar

**Layout:** Horizontal stack, 44px height, #1a1a1a background

```
[Icon 24px] [12px gap] [Title] [flexible spacer] [T][S][H][+][]
```

| ID | Element | Spec | Behavior |
|----|---------|------|----------|
| TB-1 | App Icon | 24x24 NSImageView | Static, ai_eye.svg |
| TB-2 | Title Label | "PersonalAgent", 14pt bold | Static |
| TB-3 | [T] Button | 28x28, "T" label | Toggle thinking visibility |
| TB-4 | [T] Active State | Blue background when enabled | Visual feedback |
| TB-5 | [S] Button | 28x28, "S" label | Save (currently unused) |
| TB-6 | [H] Button | 28x28, "H" label | Navigate to History |
| TB-7 | [+] Button | 28x28, "+" label | Create new conversation |
| TB-8 | [] Button | 28x28, gear icon | Navigate to Settings |
| TB-9 | Button Style | Borderless, hover highlight | Consistent feel |
| TB-10 | Button Spacing | 8px between buttons | Uniform gaps |

### Title Bar

**Layout:** Horizontal stack, 32px height, below top bar

```
[Dropdown 200px+] [8px] [Model Label] [flexible spacer]
   - OR -
[Edit Field - full width minus margins]
```

| ID | Element | Spec | Behavior |
|----|---------|------|----------|
| TT-1 | Dropdown | NSPopUpButton, min 200px width | Shows current title |
| TT-2 | Dropdown Items | All conversations, newest first | From ConversationService |
| TT-3 | Selection | Loads selected conversation | Service call |
| TT-4 | Current Highlight | Checkmark on current item | Standard popup behavior |
| TT-5 | Model Label | 11pt, #888888, right of dropdown | "provider:model" format |
| TT-6 | Edit Field | NSTextField, hidden by default | Replaces dropdown when active |
| TT-7 | Edit Field Width | Matches dropdown width | Consistent layout |
| TT-8 | Edit Placeholder | "Enter conversation title..." | Hint text |

### Chat Scroll Area

**Layout:** NSScrollView with FlippedStackView, flexible height

| ID | Element | Spec | Behavior |
|----|---------|------|----------|
| CS-1 | Container | NSScrollView, vertical only | Clip to bounds |
| CS-2 | Content | FlippedStackView (top-to-bottom) | Custom subclass |
| CS-3 | Background | #121212 | Darker than bars |
| CS-4 | Padding | 12px left/right, 8px top/bottom | Content insets |
| CS-5 | Message Spacing | 8px between bubbles | Vertical stack spacing |
| CS-6 | Auto-scroll | Scroll to bottom on new message | Unless user scrolled up |
| CS-7 | Empty State | "No messages yet" centered, #666666 | When no messages |

### User Message Bubbles

**Layout:** Right-aligned with left spacer

```
[flexible spacer] [bubble max 300px]
```

| ID | Element | Spec |
|----|---------|------|
| UM-1 | Alignment | Right (spacer + bubble in HStack) |
| UM-2 | Background | #2a4a2a (green tint) |
| UM-3 | Text Color | #e5e5e5 |
| UM-4 | Corner Radius | 12px all corners |
| UM-5 | Padding | 10px all sides |
| UM-6 | Max Width | 300px |
| UM-7 | Min Width | None (fits content) |
| UM-8 | Text Wrap | Word wrap, multi-line |
| UM-9 | Font | System 13pt |

### Assistant Message Bubbles

**Layout:** Left-aligned with right spacer

```
[bubble max 300px] [flexible spacer]
```

| ID | Element | Spec |
|----|---------|------|
| AM-1 | Alignment | Left (bubble + spacer in HStack) |
| AM-2 | Background | #1a1a1a (dark gray) |
| AM-3 | Text Color | #e5e5e5 |
| AM-4 | Corner Radius | 12px all corners |
| AM-5 | Padding | 10px all sides |
| AM-6 | Max Width | 300px |
| AM-7 | Min Width | None (fits content) |
| AM-8 | Text Wrap | Word wrap, multi-line |
| AM-9 | Font | System 13pt |
| AM-10 | Streaming Cursor | "▌" appended during streaming |

### Thinking Section

**Layout:** Full-width collapsible section above assistant bubble

```
┌─────────────────────────────────┐
│ ▼ Thinking...        [header]   │  ← Click to collapse/expand
├─────────────────────────────────┤
│ Thinking content...  [content]  │  ← Hidden when collapsed
└─────────────────────────────────┘
```

| ID | Element | Spec |
|----|---------|------|
| TK-1 | Container | Full message width, left-aligned |
| TK-2 | Background | #1a1a2a (blue tint) |
| TK-3 | Header | "▼ Thinking..." or "▶ Thinking..." |
| TK-4 | Header Click | Toggles content visibility |
| TK-5 | Content Font | System Italic 12pt |
| TK-6 | Content Color | #888888 (secondary) |
| TK-7 | Corner Radius | 8px |
| TK-8 | Padding | 8px all sides |
| TK-9 | Visibility | Only when has content AND show_thinking=true |
| TK-10 | Default State | Expanded |

### Input Bar

**Layout:** Horizontal stack, 50px height, #1a1a1a background, 1px top border #333

```
[12px] [Input Field - flex] [8px] [Send 60px] [8px] [Stop 60px] [12px]
```

| ID | Element | Spec |
|----|---------|------|
| IB-1 | Container | HStack, 50px height |
| IB-2 | Background | #1a1a1a |
| IB-3 | Top Border | 1px, #333333 |
| IB-4 | Edge Padding | 12px left and right |
| IB-5 | Input Field | NSTextField, flexible width |
| IB-6 | Input Height | 32px |
| IB-7 | Input Background | #2a2a2a |
| IB-8 | Input Border | 1px, #444444, 6px radius |
| IB-9 | Input Placeholder | "Type a message..." |
| IB-10 | Send Button | 60px wide, 32px tall |
| IB-11 | Stop Button | 60px wide, 32px tall |
| IB-12 | Button Gap | 8px between input and Send, Send and Stop |
| IB-13 | Button Style | Filled, 6px radius |
| IB-14 | Send Color | #2a4a2a (accent green) when enabled |
| IB-15 | Stop Color | #4a2a2a (accent red) when enabled |
| IB-16 | Disabled Color | #333333 (grayed out) |

---

## Behavioral Requirements

### Button States

| Button | Condition | State |
|--------|-----------|-------|
| Send | Input empty | Disabled (gray) |
| Send | Input has text | Enabled (green) |
| Send | Streaming active | Disabled (gray) |
| Stop | Not streaming | Disabled (gray) |
| Stop | Streaming active | Enabled (red) |
| [T] | Thinking disabled | Normal |
| [T] | Thinking enabled | Active (blue bg) |

### Message Sending Flow

| Step | UI Action | Visual Feedback |
|------|-----------|-----------------|
| 1 | User types in input | Text appears |
| 2 | User presses Enter or clicks Send | Input clears |
| 3 | | User bubble appears (right-aligned, green) |
| 4 | | Assistant placeholder appears with "▌" |
| 5 | | Send button → disabled, Stop button → enabled |
| 6 | | Text streams into assistant bubble |
| 7 | | (Optional) Thinking section appears above |
| 8 | On complete | Cursor "▌" removed |
| 9 | | Stop button → disabled, Send button → enabled |

### New Conversation Flow

| Step | Trigger | UI Action |
|------|---------|-----------|
| 1 | Click [+] button | |
| 2 | | Chat area clears |
| 3 | | Dropdown updates with new "New YYYY-MM-DD HH:MM" |
| 4 | | Dropdown hides, Edit field appears |
| 5 | | Edit field has focus, shows default title |
| 6 | | User types new title (or keeps default) |
| 7 | User presses Enter OR clicks elsewhere | |
| 8 | | Edit field hides, Dropdown reappears |
| 9 | | Dropdown shows new title |
| 10 | | Conversation saved via service |

### Rename Conversation Flow

| Step | Trigger | UI Action |
|------|---------|-----------|
| 1 | Double-click dropdown | |
| 2 | | Dropdown hides |
| 3 | | Edit field appears with current title |
| 4 | | Edit field has focus, text selected |
| 5 | User edits title | |
| 6 | User presses Enter OR clicks elsewhere | |
| 7 | | Edit field hides, Dropdown reappears |
| 8 | | Dropdown shows updated title |
| 9 | | Title saved via service |

### Conversation Selection Flow

| Step | Trigger | UI Action |
|------|---------|-----------|
| 1 | Click dropdown | Dropdown opens |
| 2 | | Shows all conversations (newest first) |
| 3 | | Current has checkmark |
| 4 | User clicks different conversation | |
| 5 | | Dropdown closes |
| 6 | | Chat area clears momentarily |
| 7 | | Selected conversation messages load |
| 8 | | Scroll to bottom |
| 9 | | Model label updates |

### Cancel Streaming Flow

| Step | Trigger | UI Action |
|------|---------|-----------|
| 1 | Click Stop button (while streaming) | |
| 2 | | Stop button → disabled |
| 3 | | Streaming stops |
| 4 | | Cursor "▌" removed |
| 5 | | Partial response kept in bubble |
| 6 | | Send button → enabled |

### Thinking Toggle Flow

| Step | Trigger | UI Action |
|------|---------|-----------|
| 1 | Click [T] button | |
| 2 | If was OFF | Button gets blue background |
| 3 | | Future responses will show thinking |
| 4 | If was ON | Button returns to normal |
| 5 | | Future responses hide thinking |
| 6 | | Setting persisted to profile |

---

## Stream Event Handling

The view receives **clean, pre-processed events** from ChatService. No parsing needed.

| Event | UI Action |
|-------|-----------|
| TextDelta { content } | Append content to assistant bubble |
| ThinkingDelta { content } | Append content to thinking section (if visible) |
| ToolCallStart { name } | (Optional) Show tool indicator |
| ToolResult { result } | (Optional) Update tool indicator |
| Complete { text, thinking } | Remove cursor, finalize bubble |
| Error { message } | Show error in chat area |

---

## State Management

### View State (Minimal)

| Field | Type | Purpose |
|-------|------|---------|
| conversation_id | Option<Uuid> | Track current conversation |
| is_streaming | bool | Button states |
| stream_handle | Option<StreamHandle> | For cancellation |
| edit_field_visible | bool | Title edit mode |

### UI References

| Field | Type | Purpose |
|-------|------|---------|
| title_popup | NSPopUpButton | Conversation selection |
| title_edit_field | NSTextField | Rename input |
| model_label | NSTextField | Current model display |
| thinking_button | NSButton | Toggle thinking |
| input_field | NSTextField | Message input |
| send_button | NSButton | Send action |
| stop_button | NSButton | Cancel action |
| messages_container | NSStackView | Message bubbles |
| scroll_view | NSScrollView | Chat scroll area |
| current_assistant_bubble | Option<NSView> | Streaming target |
| current_thinking_section | Option<NSView> | Streaming target |

---

## Service Dependencies

| Action | Service | Method |
|--------|---------|--------|
| Send message | ChatService | send_message() → StreamHandle |
| Cancel streaming | ChatService | cancel(handle) |
| Load conversation | ConversationService | load(id) |
| Create conversation | ConversationService | create() |
| Update title | ConversationService | update_metadata() |
| List conversations | ConversationService | load_all_metadata() |
| Get current profile | ProfileService | get_default() |

---

## Known Issues

1. **Title edit visibility** - Edit field sometimes doesn't appear for new conversations  
2. **Scroll behavior** - Auto-scroll sometimes doesn't trigger
3. **Markdown rendering** - Not implemented, shows raw text
4. **Stop button missing** - Was removed, needs to be re-added

---

## Test Coverage

### Visual Tests

- [ ] User bubble right-aligned with green background
- [ ] Assistant bubble left-aligned with dark background
- [ ] Thinking section blue tint, collapsible
- [ ] Stop button red when enabled, gray when disabled
- [ ] Send button green when enabled, gray when disabled

### Interaction Tests

- [ ] Enter key sends message
- [ ] Shift+Enter adds newline
- [ ] [+] creates new conversation and shows edit field
- [ ] Double-click title shows edit field
- [ ] Enter in edit field commits and hides
- [ ] Click outside edit field commits and hides
- [ ] Stop button cancels streaming
- [ ] Dropdown selection loads conversation

### State Tests

- [ ] Send disabled when input empty
- [ ] Send disabled during streaming
- [ ] Stop disabled when not streaming
- [ ] Stop enabled during streaming
- [ ] [T] button shows active state when thinking enabled
