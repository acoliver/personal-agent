# PersonalAgent - UI Wireframes

**Version:** 1.0  
**Date:** 2026-01-14

These wireframes define the exact layout structure using NSStackView and constraints.
Each screen is 400x500 pixels displayed inside an NSPopover.

---

## Layout Legend

```
NSStackView (vertical)   = |----name----|
NSStackView (horizontal) = [----name----]
NSScrollView             = {----name----}
NSButton                 = [Btn]
NSTextField (input)      = [...input...]
NSTextField (label)      = "Label Text"
NSView (spacer)          = <--spacer-->
Height constraint        = (h=XX)
Fixed width              = (w=XX)
Flexible/expands         = (flex)
Content hugging high     = (hug:H)
Content hugging low      = (hug:L)
```

---

## Screen 1: Chat View (Main Screen)

```
===============NSPopover (400x500)=================
| |----------MainStack (vert)------------------| |
| | [--------TopBar (horiz, h=44, hug:H)-----] | |
| | | [Icon*1] "PersonalAgent" <-spacer->    | | |
| | | [T]*2 [S]*3 [H]*4 [+]*5 [Gear]*6       | | |
| | [----------------------------------------] | |
| |                                            | |
| | {------ChatScroll (flex, hug:L)----------} | |
| | { |----MessagesStack (vert)------------| } | |
| | { | [---UserMsg (horiz)---------------]| } | |
| | { | | <spacer> [UserBubble*7]         || } | |
| | { | [----------------------------------]| } | |
| | { |                                    | } | |
| | { | [---AssistantMsg (horiz)----------]| } | |
| | { | | [AssistantBubble*8] <spacer>    || } | |
| | { | [----------------------------------]| } | |
| | { |                                    | } | |
| | { | [---ThinkingBlock (if visible)----]| } | |
| | { | | [ThinkingBubble*9]              || } | |
| | { | [----------------------------------]| } | |
| | { |------------------------------------| } | |
| | {------------------------------------------} | |
| |                                            | |
| | [--------InputBar (horiz, h=50, hug:H)---] | |
| | | [...MessageInput (flex)...] [Send]*10  | | |
| | [----------------------------------------] | |
| |--------------------------------------------| |
===================================================

*1  Icon: NSImageView, 24x24, red eye icon (ai_eye.svg)
*2  [T]: NSButton "T" (w=28) - Toggle thinking visibility
*3  [S]: NSButton "S" (w=28) - Save conversation
*4  [H]: NSButton "H" (w=28) - Show history view
*5  [+]: NSButton "+" (w=28) - New conversation
*6  [Gear]: NSButton (gear icon, w=28) - Show settings
*7  UserBubble: NSTextField (label, multi-line, right-aligned)
    - Background: #2a4a2a (dark green tint)
    - Text: #e5e5e5 (light gray)
    - Corner radius: 12
    - Max width: 300
    - Padding: 10
*8  AssistantBubble: NSTextField (label, multi-line, left-aligned)
    - Background: #1a1a1a (dark gray)
    - Text: #e5e5e5 (light gray)
    - Corner radius: 12
    - Max width: 300
    - Padding: 10
*9  ThinkingBubble: NSTextField (label, multi-line)
    - Background: #1a1a2a (dark blue tint)
    - Text: #888888 (secondary gray)
    - Italic font
    - Only visible when thinking enabled AND content exists
*10 Send: NSButton "Send" (w=60) - Send message
```

### Chat View Constraints

```
TopBar:
  - height = 44 (fixed)
  - leading = MainStack.leading
  - trailing = MainStack.trailing
  - contentHuggingPriority(vertical) = 750 (high)

ChatScroll:
  - height >= 100 (minimum)
  - contentHuggingPriority(vertical) = 1 (low - expands)
  - compressionResistance(vertical) = 250 (low)

InputBar:
  - height = 50 (fixed)
  - contentHuggingPriority(vertical) = 750 (high)

MessageInput:
  - contentHuggingPriority(horizontal) = 1 (low - expands)

Send button:
  - width >= 60
  - contentHuggingPriority(horizontal) = 750 (high)
```

---

## Screen 2: Settings View

```
===============NSPopover (400x500)=================
| |----------MainStack (vert)------------------| |
| | [--------TopBar (horiz, h=44, hug:H)-----] | |
| | | [<]*1 "Settings" <-spacer->            | | |
| | | [RefreshModels]*2                      | | |
| | [----------------------------------------] | |
| |                                            | |
| | {------ProfilesScroll (flex, hug:L)------} | |
| | { |----ProfilesStack (vert, spacing=8)-| } | |
| | { |                                    | } | |
| | { | [---ProfileCard*3---------------]  | } | |
| | { | | "Claude Sonnet"                |  | } | |
| | { | | "anthropic:claude-sonnet-4"    |  | } | |
| | { | | "API Key: sk-...XXXX"          |  | } | |
| | { | | [Select]*4 [Edit]*5 [Del]*6    |  | } | |
| | { | [-------------------------------]  | } | |
| | { |                                    | } | |
| | { | [---ProfileCard----------------]   | } | |
| | { | | "GPT-4 Turbo"                 |  | } | |
| | { | | "openai:gpt-4-turbo"          |  | } | |
| | { | | "API Key: (not set)"          |  | } | |
| | { | | [Select] [Edit] [Del]         |  | } | |
| | { | [-------------------------------]  | } | |
| | { |                                    | } | |
| | { |------------------------------------| } | |
| | {------------------------------------------} | |
| |                                            | |
| | [--------BottomBar (horiz, h=50, hug:H)--] | |
| | | <-spacer-> [+ Add Profile]*7           | | |
| | [----------------------------------------] | |
| |--------------------------------------------| |
===================================================

*1  [<]: NSButton "<" (w=40) - Back to chat
*2  [RefreshModels]: NSButton "Refresh Models" (w=120) - Fetch from models.dev
*3  ProfileCard: NSStackView (vertical, spacing=4)
    - Background: #1a1a1a
    - Corner radius: 8
    - Padding: 12
    - Contains: name label, provider:model label, auth status, action buttons
*4  [Select]: NSButton "Select" - Make this profile active
    - If already active: disabled, title = "Active"
*5  [Edit]: NSButton "Edit" - Open profile editor for this profile
*6  [Del]: NSButton "Del" - Delete this profile (with confirmation)
*7  [+ Add Profile]: NSButton "+ Add Profile" - Open profile editor for new profile
```

### Settings View - Empty State

```
===============NSPopover (400x500)=================
| |----------MainStack (vert)------------------| |
| | [--------TopBar (horiz, h=44)------------] | |
| | | [<] "Settings" <-spacer->              | | |
| | | [RefreshModels]                        | | |
| | [----------------------------------------] | |
| |                                            | |
| | |--------EmptyState (flex)---------------| | |
| | | <-spacer (vert)->                      | | |
| | | "No profiles configured"               | | |
| | | "Add a profile to get started"         | | |
| | | <-spacer (vert)->                      | | |
| | |----------------------------------------| | |
| |                                            | |
| | [--------BottomBar (horiz, h=50)---------] | |
| | | <-spacer-> [+ Add Profile]             | | |
| | [----------------------------------------] | |
| |--------------------------------------------| |
===================================================
```

---

## Screen 3: Profile Editor

```
===============NSPopover (400x500)=================
| |----------MainStack (vert)------------------| |
| | [--------TopBar (horiz, h=44, hug:H)-----] | |
| | | [Cancel]*1 <-spacer-> [Save]*2         | | |
| | [----------------------------------------] | |
| |                                            | |
| | {------FormScroll (flex, hug:L)----------} | |
| | { |----FormStack (vert, spacing=16)----| } | |
| | { |                                    | } | |
| | { | "Profile Name"                     | } | |
| | { | [...profileNameInput...]*3         | } | |
| | { |                                    | } | |
| | { | "Provider"                         | } | |
| | { | [ProviderPopup]*4                  | } | |
| | { |                                    | } | |
| | { | "Model"                            | } | |
| | { | [ModelPopup]*5                     | } | |
| | { |                                    | } | |
| | { | "Base URL"                         | } | |
| | { | [...baseUrlInput...]*6             | } | |
| | { |                                    | } | |
| | { | "Authentication"                   | } | |
| | { | [AuthTypePopup]*7                  | } | |
| | { | [...authValueInput...]*8           | } | |
| | { |                                    | } | |
| | { | |---ParametersSection (vert)-----| | } | |
| | { | | "Parameters"                   | | } | |
| | { | |                                | | } | |
| | { | | "Temperature" [Slider]*9       | | } | |
| | { | | [0.0]      [value]      [2.0]  | | } | |
| | { | |                                | | } | |
| | { | | "Max Tokens" [...input...]*10  | | } | |
| | { | |                                | | } | |
| | { | | "Thinking Budget" [...]*11     | | } | |
| | { | |                                | | } | |
| | { | | [x] Enable Thinking*12         | | } | |
| | { | | [x] Show Thinking*13           | | } | |
| | { | |--------------------------------| | } | |
| | { |------------------------------------| } | |
| | {------------------------------------------} | |
| |--------------------------------------------| |
===================================================

*1  [Cancel]: NSButton "Cancel" (w=70) - Discard changes, return to settings
*2  [Save]: NSButton "Save" (w=60) - Save profile, return to settings
*3  profileNameInput: NSTextField, placeholder "My Profile"
*4  ProviderPopup: NSPopUpButton - List providers from models.dev
    - Items: "anthropic", "openai", "google", "groq", "mistral", "ollama", "custom"
    - On change: Update model list, base URL
*5  ModelPopup: NSPopUpButton - List models for selected provider
    - Items populated from models.dev based on selected provider
    - On change: Update base URL if needed
*6  baseUrlInput: NSTextField
    - Auto-populated from models.dev based on provider
    - Editable for custom providers
*7  AuthTypePopup: NSPopUpButton
    - Items: "API Key", "Key File", "None"
*8  authValueInput: NSTextField
    - If "API Key": placeholder "sk-...", secure text entry
    - If "Key File": placeholder "/path/to/keyfile"
    - If "None": hidden
*9  Temperature Slider: NSSlider, min=0.0, max=2.0, default=1.0
*10 maxTokensInput: NSTextField, numeric, placeholder "4096"
*11 thinkingBudgetInput: NSTextField, numeric, placeholder "10000"
    - Only visible when Enable Thinking is checked
*12 enableThinkingCheckbox: NSButton (checkbox) "Enable Thinking"
    - Controls whether thinking tokens are requested from model
*13 showThinkingCheckbox: NSButton (checkbox) "Show Thinking"
    - Controls whether thinking content is displayed in UI
    - Only visible/enabled when Enable Thinking is checked
```

---

## Screen 4: History View

```
===============NSPopover (400x500)=================
| |----------MainStack (vert)------------------| |
| | [--------TopBar (horiz, h=44, hug:H)-----] | |
| | | [<]*1 "History" <-spacer->             | | |
| | [----------------------------------------] | |
| |                                            | |
| | {------HistoryScroll (flex, hug:L)-------} | |
| | { |----HistoryStack (vert, spacing=8)--| } | |
| | { |                                    | } | |
| | { | [---ConversationCard*2-----------] | } | |
| | { | | "Chat about Rust macros"        | | } | |
| | { | | "Jan 14, 2026 2:30 PM"          | | } | |
| | { | | "12 messages"                   | | } | |
| | { | | [Load]*3 [Delete]*4             | | } | |
| | { | [----------------------------------]| } | |
| | { |                                    | } | |
| | { | [---ConversationCard--------------]| } | |
| | { | | "Untitled Conversation"         | | } | |
| | { | | "Jan 13, 2026 10:15 AM"         | | } | |
| | { | | "5 messages"                    | | } | |
| | { | | [Load] [Delete]                 | | } | |
| | { | [----------------------------------]| } | |
| | { |                                    | } | |
| | { | [---ConversationCard--------------]| } | |
| | { | | "Weather API integration"       | | } | |
| | { | | "Jan 12, 2026 4:45 PM"          | | } | |
| | { | | "23 messages"                   | | } | |
| | { | | [Load] [Delete]                 | | } | |
| | { | [----------------------------------]| } | |
| | { |------------------------------------| } | |
| | {------------------------------------------} | |
| |--------------------------------------------| |
===================================================

*1  [<]: NSButton "<" (w=40) - Back to chat
*2  ConversationCard: NSStackView (vertical, spacing=4)
    - Background: #1a1a1a
    - Corner radius: 8
    - Padding: 12
    - Title: First line of first user message, or "Untitled Conversation"
    - Date: Formatted from filename timestamp
    - Message count: Number of messages in conversation
*3  [Load]: NSButton "Load" - Load this conversation into chat view
    - Closes history, switches to chat with loaded conversation
*4  [Delete]: NSButton "Delete" - Delete conversation file
    - Should show confirmation alert
```

### History View - Empty State

```
===============NSPopover (400x500)=================
| |----------MainStack (vert)------------------| |
| | [--------TopBar (horiz, h=44)------------] | |
| | | [<] "History" <-spacer->               | | |
| | [----------------------------------------] | |
| |                                            | |
| | |--------EmptyState (flex)---------------| | |
| | | <-spacer (vert)->                      | | |
| | | "No saved conversations"               | | |
| | | "Start chatting to create history"     | | |
| | | <-spacer (vert)->                      | | |
| | |----------------------------------------| | |
| |--------------------------------------------| |
===================================================
```

---

## Color Palette (Dark Theme)

```
Name              Hex       RGB               Usage
---------------------------------------------------------
BG_DARKEST        #0d0d0d   (13, 13, 13)      Main background
BG_DARK           #1a1a1a   (26, 26, 26)      Cards, bubbles
BG_MID            #242424   (36, 36, 36)      Input fields, hover
BG_LIGHT          #2a2a2a   (42, 42, 42)      Borders, separators
TEXT_PRIMARY      #e5e5e5   (229, 229, 229)   Main text
TEXT_SECONDARY    #888888   (136, 136, 136)   Secondary text
TEXT_MUTED        #666666   (102, 102, 102)   Disabled, hints
ACCENT_BLUE       #007AFF   (0, 122, 255)     Links, active
ACCENT_GREEN      #2a4a2a   (42, 74, 42)      User message bg
ACCENT_RED        #FF3B30   (255, 59, 48)     Delete, error
THINKING_BG       #1a1a2a   (26, 26, 42)      Thinking bubble bg
```

---

## NSStackView Configuration Reference

### Vertical Stack (Main Container)
```rust
stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
stack.setSpacing(0.0);
stack.setDistribution(NSStackViewDistribution::Fill);
stack.setTranslatesAutoresizingMaskIntoConstraints(false);
```

### Horizontal Stack (Bars)
```rust
stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
stack.setSpacing(8.0);
stack.setEdgeInsets(NSEdgeInsets { top: 8.0, left: 12.0, bottom: 8.0, right: 12.0 });
stack.setTranslatesAutoresizingMaskIntoConstraints(false);
```

### Content Hugging & Compression Resistance
```rust
// Fixed-height items (bars, buttons)
view.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
view.setContentCompressionResistancePriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);

// Flexible items (scroll views, text fields)
view.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
view.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Vertical);
```

### Explicit Height Constraints
```rust
// For fixed-height items, add explicit constraint
let height = view.heightAnchor().constraintEqualToConstant(44.0);
height.setActive(true);

// For minimum height (scroll views)
let min_height = view.heightAnchor().constraintGreaterThanOrEqualToConstant(100.0);
min_height.setActive(true);
```

---

## Implementation Checklist

### Chat View
- [ ] MainStack (vertical, Distribution::Fill)
- [ ] TopBar (horizontal, h=44 fixed)
  - [ ] Icon (NSImageView, 24x24)
  - [ ] Title label
  - [ ] Spacer (hug:L horizontal)
  - [ ] T, S, H, +, Gear buttons (w=28 each)
- [ ] ChatScroll (NSScrollView, hug:L vertical)
  - [ ] MessagesStack (vertical, spacing=12)
  - [ ] Message bubbles with proper alignment
- [ ] InputBar (horizontal, h=50 fixed)
  - [ ] TextField (hug:L horizontal)
  - [ ] Send button (w=60)

### Settings View
- [ ] MainStack (vertical, Distribution::Fill)
- [ ] TopBar (horizontal, h=44 fixed)
  - [ ] Back button
  - [ ] Title
  - [ ] Spacer
  - [ ] Refresh button
- [ ] ProfilesScroll (NSScrollView, hug:L vertical)
  - [ ] ProfilesStack (vertical, spacing=8)
  - [ ] ProfileCard components
- [ ] BottomBar (horizontal, h=50 fixed)
  - [ ] Add Profile button

### Profile Editor
- [ ] MainStack (vertical, Distribution::Fill)
- [ ] TopBar (horizontal, h=44 fixed)
  - [ ] Cancel button
  - [ ] Spacer
  - [ ] Save button
- [ ] FormScroll (NSScrollView, hug:L vertical)
  - [ ] FormStack (vertical, spacing=16)
  - [ ] All form fields with labels
  - [ ] Parameters section

### History View
- [ ] MainStack (vertical, Distribution::Fill)
- [ ] TopBar (horizontal, h=44 fixed)
  - [ ] Back button
  - [ ] Title
- [ ] HistoryScroll (NSScrollView, hug:L vertical)
  - [ ] HistoryStack (vertical, spacing=8)
  - [ ] ConversationCard components
