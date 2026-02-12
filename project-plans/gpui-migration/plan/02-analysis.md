# Phase 02: Analysis

## Phase ID

`PLAN-20250128-GPUI.P02`

## Prerequisites

- Phase 01a completed with PASS
- Evidence file: `project-plans/gpui-migration/plan/.completed/P01A.md`

---

## Purpose

Analyze existing code and create pseudocode for GPUI implementation:

1. Map existing AppKit view structure to GPUI components
2. Document presenter→view data flow
3. Create component hierarchy diagram
4. Write pseudocode for key components

---

## Analysis Tasks

### 1. Existing View Structure Analysis

Map current `src/ui/` views to future `src/ui_gpui/` components:

| Current AppKit | Lines | GPUI Component | Notes |
|----------------|-------|----------------|-------|
| `chat_view.rs` | ~980 | `views/chat_view.rs` | Messages, input, streaming |
| `history_view.rs` | ~779 | `views/history_view.rs` | Conversation list |
| `settings_view.rs` | ~1191 | `views/settings_view.rs` | Settings panels |
| `theme.rs` | ~91 | `theme.rs` | Color system |

### 2. Component Hierarchy

```
MainPanel (Render trait)
├── TabBar (IntoElement)
│   ├── Tab::Chat
│   ├── Tab::History
│   └── Tab::Settings
├── ChatView (Render trait) [when Tab::Chat]
│   ├── TopBar (toolbar buttons)
│   ├── TitleBar (conversation dropdown)
│   ├── MessageList (scrollable)
│   │   ├── UserBubble (IntoElement)
│   │   └── AssistantGroup (IntoElement)
│   │       ├── ModelLabel
│   │       ├── ThinkingSection (collapsible)
│   │       └── AssistantBubble
│   └── InputBar
│       ├── TextField
│       ├── SendButton
│       └── StopButton
├── HistoryView (Render trait) [when Tab::History]
│   └── ConversationList
│       └── ConversationRow (IntoElement)
└── SettingsView (Render trait) [when Tab::Settings]
    ├── ProfileSection
    ├── McpSection
    └── AppearanceSection (transparency slider)
```

### 3. Data Flow Analysis

```
┌─────────────────────────────────────────────────────────────┐
│ GPUI Component (e.g., InputBar)                             │
│                                                             │
│  on_click(Send) ──────────────────────────────────────────┐ │
│                                                           │ │
└───────────────────────────────────────────────────────────│─┘
                                                            │
                                                            ▼
┌─────────────────────────────────────────────────────────────┐
│ EventBus                                                    │
│                                                             │
│  publish(UserEvent::SendMessage { text })                   │
│                                                             │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│ ChatPresenter (subscribes to UserEvent)                     │
│                                                             │
│  handle_user_event(SendMessage) {                           │
│    chat_service.send_message(...)                           │
│  }                                                          │
│                                                             │
│  // Receives ChatEvent::TextDelta from service              │
│  handle_chat_event(TextDelta) {                             │
│    view_tx.send(ViewCommand::AppendText { ... })            │
│  }                                                          │
│                                                             │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│ GPUI Component (receives via mpsc)                          │
│                                                             │
│  // In async task polling view_rx:                          │
│  match view_command {                                       │
│    ViewCommand::AppendText { content } => {                 │
│      self.current_message.push_str(&content);               │
│      cx.notify(); // Triggers re-render                     │
│    }                                                        │
│  }                                                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 4. State Management Analysis

| State | Location | GPUI Pattern |
|-------|----------|--------------|
| Current tab | MainPanel | `struct MainPanel { active_tab: Tab }` |
| Messages | ChatView | `struct ChatView { messages: Vec<Message> }` |
| Streaming state | ChatView | `struct ChatView { is_streaming: bool, current_response: String }` |
| Thinking visibility | ChatView | `struct ChatView { show_thinking: bool }` |
| Conversations | HistoryView | `struct HistoryView { conversations: Vec<ConversationMeta> }` |
| Settings | SettingsView | `struct SettingsView { config: ThemeConfig }` |

### 5. ExactoBar Pattern Mapping

| ExactoBar Pattern | Our Usage |
|-------------------|-----------|
| `Application::new().run()` | App initialization |
| `QuitMode::Explicit` | Menu bar app (no quit on window close) |
| `WindowKind::PopUp` | Popup below status item |
| `WindowBackgroundAppearance::Blurred` | Transparency effect |
| `cx.listener()` for click handlers | Button actions |
| `cx.notify()` for re-render | ViewCommand handling |
| `cx.global::<T>()` | Global state (if needed) |
| mpsc channel for click bridging | NSStatusItem → GPUI |

---

## Pseudocode Development

Create: `project-plans/gpui-migration/analysis/`

### pseudocode/app.md

```
// GPUI Application Setup
// Lines 1-50

1. FUNCTION initialize_app():
2.   CREATE Application::new()
3.   SET quit_mode = QuitMode::Explicit  // Menu bar app
4.   
5.   CREATE tray_bridge = TrayBridge::new()
6.   SPAWN tray_bridge.run()  // NSStatusItem click handling
7.   
8.   CREATE presenter_bridge = PresenterBridge::new(event_bus)
9.   
10.  RUN app.run(|cx| {
11.    // Initialize global state if needed
12.    cx.set_global(AppState::default())
13.    
14.    // Subscribe to tray clicks
15.    SPAWN poll_tray_clicks(tray_bridge.rx, cx)
16.  })

17. FUNCTION poll_tray_clicks(rx, cx):
18.   LOOP:
19.     MATCH rx.recv():
20.       TrayClick::Toggle => toggle_popup(cx)
21.       TrayClick::Quit => cx.quit()

22. FUNCTION toggle_popup(cx):
23.   IF popup_visible:
24.     close_popup(cx)
25.   ELSE:
26.     open_popup(cx)

27. FUNCTION open_popup(cx):
28.   GET status_item_frame from tray_bridge
29.   CALCULATE popup_position below status item
30.   
31.   CREATE window_options = WindowOptions {
32.     kind: WindowKind::PopUp,
33.     bounds: Bounds { origin: popup_position, size: (400, 500) },
34.     background: WindowBackgroundAppearance::Blurred,
35.   }
36.   
37.   cx.open_window(window_options, |window, cx| {
38.     CREATE main_panel = MainPanel::new(presenter_bridge.clone())
39.     RETURN cx.new(|_| main_panel)
40.   })
```

### pseudocode/main_panel.md

```
// MainPanel - Root component with tabs
// Lines 1-80

1. STRUCT MainPanel {
2.   active_tab: Tab,
3.   chat_state: ChatState,
4.   history_state: HistoryState,
5.   settings_state: SettingsState,
6.   presenter_bridge: Arc<PresenterBridge>,
7. }

8. ENUM Tab { Chat, History, Settings }

9. IMPL Render FOR MainPanel:
10.   FUNCTION render(self, window, cx) -> Element:
11.     LET content = MATCH self.active_tab:
12.       Tab::Chat => self.render_chat(cx)
13.       Tab::History => self.render_history(cx)
14.       Tab::Settings => self.render_settings(cx)
15.     
16.     RETURN div()
17.       .size_full()
18.       .flex_col()
19.       .bg(theme.background)
20.       .child(self.render_tab_bar(cx))
21.       .child(content)

22. FUNCTION render_tab_bar(self, cx) -> Element:
23.   RETURN div()
24.     .flex_row()
25.     .h(px(44))
26.     .bg(theme.bar_background)
27.     .child(TabButton::new(Tab::Chat, self.active_tab == Tab::Chat)
28.       .on_click(cx.listener(|this, _, cx| {
29.         this.active_tab = Tab::Chat
30.         cx.notify()
31.       })))
32.     .child(TabButton::new(Tab::History, ...))
33.     .child(TabButton::new(Tab::Settings, ...))

34. FUNCTION render_chat(self, cx) -> Element:
35.   RETURN ChatView::new(&self.chat_state, &self.presenter_bridge)
36.     .into_element()

37. IMPL MainPanel:
38.   FUNCTION new(presenter_bridge) -> Self:
39.     RETURN Self {
40.       active_tab: Tab::Chat,
41.       chat_state: ChatState::default(),
42.       history_state: HistoryState::default(),
43.       settings_state: SettingsState::default(),
44.       presenter_bridge,
45.     }
46.   
47.   FUNCTION handle_view_command(self, cmd, cx):
48.     MATCH cmd:
49.       ViewCommand::ShowMessages(msgs) => {
50.         self.chat_state.messages = msgs
51.         cx.notify()
52.       }
53.       ViewCommand::AppendText(text) => {
54.         self.chat_state.current_response.push_str(&text)
55.         cx.notify()
56.       }
57.       ViewCommand::ShowError(err) => {
58.         self.chat_state.error = Some(err)
59.         cx.notify()
60.       }
61.       ViewCommand::Navigate(ViewId::History) => {
62.         self.active_tab = Tab::History
63.         cx.notify()
64.       }
65.       // ... more cases
```

### pseudocode/chat_view.md

```
// ChatView - Messages and input
// Lines 1-150

1. STRUCT ChatState {
2.   messages: Vec<Message>,
3.   current_response: String,
4.   current_thinking: String,
5.   is_streaming: bool,
6.   show_thinking: bool,
7.   conversation_id: Option<Uuid>,
8.   error: Option<ErrorInfo>,
9. }

10. STRUCT ChatView<'a> {
11.   state: &'a ChatState,
12.   bridge: &'a PresenterBridge,
13. }

14. IMPL IntoElement FOR ChatView:
15.   FUNCTION into_element(self) -> Element:
16.     RETURN div()
17.       .flex_col()
18.       .flex_1()
19.       .child(self.render_top_bar())
20.       .child(self.render_title_bar())
21.       .child(self.render_messages())
22.       .child(self.render_input_bar())

23. FUNCTION render_top_bar(self) -> Element:
24.   RETURN div()
25.     .h(px(44))
26.     .flex_row()
27.     .items_center()
28.     .px(px(12))
29.     .bg(theme.bar_background)
30.     .child(Icon::new("ai_eye").size(24))
31.     .child(Label::new("PersonalAgent").bold())
32.     .child(Spacer::flex())
33.     .child(ToolbarButton::new("[T]")
34.       .active(self.state.show_thinking)
35.       .on_click(|cx| emit_event(UserEvent::ToggleThinking)))
36.     .child(ToolbarButton::new("[H]")
37.       .on_click(|cx| emit_event(UserEvent::Navigate(ViewId::History))))
38.     .child(ToolbarButton::new("[+]")
39.       .on_click(|cx| emit_event(UserEvent::NewConversation)))
40.     .child(ToolbarButton::new("")
41.       .on_click(|cx| emit_event(UserEvent::Navigate(ViewId::Settings))))

42. FUNCTION render_messages(self) -> Element:
43.   LET messages_view = div()
44.     .flex_col()
45.     .gap(px(8))
46.     .p(px(12))
47.   
48.   FOR msg IN self.state.messages:
49.     IF msg.role == User:
50.       messages_view = messages_view.child(UserBubble::new(&msg.content))
51.     ELSE:
52.       messages_view = messages_view.child(
53.         AssistantGroup::new(&msg)
54.           .show_thinking(self.state.show_thinking)
55.       )
56.   
57.   // Streaming response
58.   IF self.state.is_streaming:
59.     messages_view = messages_view.child(
60.       AssistantGroup::streaming(
61.         &self.state.current_response,
62.         &self.state.current_thinking,
63.         self.state.show_thinking
64.       )
65.     )
66.   
67.   RETURN ScrollView::new(messages_view)

68. FUNCTION render_input_bar(self) -> Element:
69.   RETURN InputBar::new()
70.     .is_streaming(self.state.is_streaming)
71.     .on_send(|text, cx| emit_event(UserEvent::SendMessage { text }))
72.     .on_stop(|cx| emit_event(UserEvent::StopStreaming))
```

---

## Deliverables

1. Create directory: `project-plans/gpui-migration/analysis/`
2. Create pseudocode files:
   - `analysis/pseudocode/app.md`
   - `analysis/pseudocode/main_panel.md`
   - `analysis/pseudocode/chat_view.md`
   - `analysis/pseudocode/components.md`
3. Create evidence file: `project-plans/gpui-migration/plan/.completed/P02.md`

Evidence file contents:
```markdown
# Phase 02: Analysis Evidence

## Analysis Complete
- View mapping: [YES/NO]
- Component hierarchy: [YES/NO]
- Data flow diagram: [YES/NO]
- State management: [YES/NO]
- ExactoBar patterns: [YES/NO]

## Pseudocode Files Created
- [ ] analysis/pseudocode/app.md ([N] lines)
- [ ] analysis/pseudocode/main_panel.md ([N] lines)
- [ ] analysis/pseudocode/chat_view.md ([N] lines)
- [ ] analysis/pseudocode/components.md ([N] lines)

## Key Decisions
[List architectural decisions made during analysis]

## Open Questions
[List any questions that need resolution]

## Verdict: [PASS|FAIL]
```

---

## Success Criteria

- [ ] All analysis sections completed
- [ ] Pseudocode files created with numbered lines
- [ ] Component hierarchy documented
- [ ] Data flow documented
- [ ] State management documented

---

## Next Phase

After P02 completes with PASS:
→ P02a: Analysis Verification
