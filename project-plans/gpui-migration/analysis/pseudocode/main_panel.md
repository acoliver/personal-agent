# MainPanel Component Pseudocode

## Overview
Root component containing tab navigation and view switching.

---

## Struct Definitions (Lines 1-30)

```
1.  // Main panel state
2.  STRUCT MainPanel {
3.    active_tab: Tab,
4.    chat_state: ChatState,
5.    history_state: HistoryState,
6.    settings_state: SettingsState,
7.    gpui_bridge: Arc<GpuiBridge>,
8.  }
9.  
10. // Tab enum
11. ENUM Tab {
12.   Chat,
13.   History,
14.   Settings,
15. }
16. 
17. // Chat view state (managed by MainPanel, passed to ChatView)
18. STRUCT ChatState {
19.   messages: Vec<Message>,
20.   current_response: String,
21.   current_thinking: String,
22.   is_streaming: bool,
23.   show_thinking: bool,
24.   conversation_id: Option<Uuid>,
25.   conversation_title: String,
26.   available_conversations: Vec<ConversationMeta>,
27.   error: Option<ErrorInfo>,
28.   selected_profile: Option<ProfileMeta>,
29.   available_profiles: Vec<ProfileMeta>,
30. }
```

## Constructor (Lines 31-50)

```
31. IMPL MainPanel:
32.   FUNCTION new(gpui_bridge: Arc<GpuiBridge>) -> Self:
33.     Self {
34.       active_tab: Tab::Chat,
35.       chat_state: ChatState {
36.         messages: Vec::new(),
37.         current_response: String::new(),
38.         current_thinking: String::new(),
39.         is_streaming: false,
40.         show_thinking: false,
41.         conversation_id: None,
42.         conversation_title: "New Conversation".to_string(),
43.         available_conversations: Vec::new(),
44.         error: None,
45.         selected_profile: None,
46.         available_profiles: Vec::new(),
47.       },
48.       history_state: HistoryState::default(),
49.       settings_state: SettingsState::default(),
50.       gpui_bridge,
51.     }
```

## Render Implementation (Lines 52-90)

```
52. IMPL Render FOR MainPanel:
53.   FUNCTION render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement:
54.     // Main container with dark theme
55.     div()
56.       .id("main-panel")
57.       .size_full()
58.       .flex()
59.       .flex_col()
60.       .bg(theme::BG_DARKEST)
61.       .child(self.render_tab_bar(cx))
62.       .child(self.render_content(cx))
63. 
64.   FUNCTION render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement:
65.     div()
66.       .id("tab-bar")
67.       .w_full()
68.       .h(px(36.0))
69.       .flex()
70.       .flex_row()
71.       .items_center()
72.       .justify_center()
73.       .gap(px(8.0))
74.       .bg(theme::BG_DARKER)
75.       .border_b_1()
76.       .border_color(theme::BORDER)
77.       .child(self.render_tab_button(Tab::Chat, "Chat", cx))
78.       .child(self.render_tab_button(Tab::History, "History", cx))
79.       .child(self.render_tab_button(Tab::Settings, "Settings", cx))
80. 
81.   FUNCTION render_tab_button(&self, tab: Tab, label: &str, cx: &mut Context<Self>) -> impl IntoElement:
82.     LET is_active = self.active_tab == tab
83.     
84.     div()
85.       .id(SharedString::from(format!("tab-{:?}", tab)))
86.       .px(px(16.0))
87.       .py(px(6.0))
88.       .rounded(px(4.0))
89.       .cursor_pointer()
90.       .when(is_active, |d| d.bg(theme::BG_DARK))
91.       .text_color(IF is_active THEN theme::TEXT_PRIMARY ELSE theme::TEXT_SECONDARY)
92.       .child(label)
93.       .on_click(cx.listener(move |this, _, cx| {
94.         this.active_tab = tab
95.         cx.notify()
96.       }))
```

## Content Rendering (Lines 91-120)

```
97.   FUNCTION render_content(&self, cx: &mut Context<Self>) -> impl IntoElement:
98.     div()
99.       .id("content")
100.      .flex_1()
101.      .overflow_hidden()
102.      .child(
103.        MATCH self.active_tab:
104.          Tab::Chat => ChatView::new(&self.chat_state, &self.gpui_bridge).into_any_element()
105.          Tab::History => HistoryView::new(&self.history_state, &self.gpui_bridge).into_any_element()
106.          Tab::Settings => SettingsView::new(&self.settings_state, &self.gpui_bridge).into_any_element()
107.      )
```

## ViewCommand Handler (Lines 121-220)

```
108. IMPL MainPanel:
109.   // Handle ViewCommand from presenter layer
110.   FUNCTION handle_view_command(&mut self, cmd: ViewCommand, cx: &mut Context<Self>):
111.     MATCH cmd:
112.       // Chat commands
113.       ViewCommand::ConversationCreated { id, profile_id } => {
114.         self.chat_state.conversation_id = Some(id)
115.         self.chat_state.messages.clear()
116.         self.chat_state.current_response.clear()
117.         self.chat_state.current_thinking.clear()
118.         self.chat_state.conversation_title = "New Conversation".to_string()
119.         cx.notify()
120.       }
121.       
122.       ViewCommand::MessageAppended { conversation_id, role, content } => {
123.         IF self.chat_state.conversation_id == Some(conversation_id):
124.           self.chat_state.messages.push(Message { role, content, ..default() })
125.           cx.notify()
126.       }
127.       
128.       ViewCommand::ShowThinking { conversation_id } => {
129.         IF self.chat_state.conversation_id == Some(conversation_id):
130.           self.chat_state.is_streaming = true
131.           self.chat_state.current_thinking.clear()
132.           cx.notify()
133.       }
134.       
135.       ViewCommand::HideThinking { conversation_id } => {
136.         IF self.chat_state.conversation_id == Some(conversation_id):
137.           // Thinking complete, keep content
138.           cx.notify()
139.       }
140.       
141.       ViewCommand::AppendStream { conversation_id, chunk } => {
142.         IF self.chat_state.conversation_id == Some(conversation_id):
143.           self.chat_state.current_response.push_str(&chunk)
144.           cx.notify()
145.       }
146.       
147.       ViewCommand::AppendThinkingStream { conversation_id, chunk } => {
148.         IF self.chat_state.conversation_id == Some(conversation_id):
149.           self.chat_state.current_thinking.push_str(&chunk)
150.           cx.notify()
151.       }
152.       
153.       ViewCommand::FinalizeStream { conversation_id, tokens } => {
154.         IF self.chat_state.conversation_id == Some(conversation_id):
155.           // Move streaming content to message
156.           LET content = std::mem::take(&mut self.chat_state.current_response)
157.           LET thinking = std::mem::take(&mut self.chat_state.current_thinking)
158.           self.chat_state.messages.push(Message {
159.             role: Role::Assistant,
160.             content,
161.             thinking: IF thinking.is_empty() THEN None ELSE Some(thinking),
162.             ..default()
163.           })
164.           self.chat_state.is_streaming = false
165.           cx.notify()
166.       }
167.       
168.       ViewCommand::StreamError { conversation_id, error, recoverable } => {
169.         IF self.chat_state.conversation_id == Some(conversation_id):
170.           self.chat_state.error = Some(ErrorInfo { message: error, recoverable })
171.           self.chat_state.is_streaming = false
172.           cx.notify()
173.       }
174.       
175.       ViewCommand::StreamCancelled { conversation_id, partial_content } => {
176.         IF self.chat_state.conversation_id == Some(conversation_id):
177.           IF LET Some(content) = partial_content:
178.             self.chat_state.messages.push(Message {
179.               role: Role::Assistant,
180.               content,
181.               is_partial: true,
182.               ..default()
183.             })
184.           self.chat_state.is_streaming = false
185.           self.chat_state.current_response.clear()
186.           cx.notify()
187.       }
188.       
189.       // Navigation commands
190.       ViewCommand::Navigate { view_id } => {
191.         self.active_tab = MATCH view_id:
192.           ViewId::Chat => Tab::Chat
193.           ViewId::History => Tab::History
194.           ViewId::Settings => Tab::Settings
195.           _ => self.active_tab  // Ignore unknown
196.         cx.notify()
197.       }
198.       
199.       // History commands
200.       ViewCommand::ConversationListLoaded { conversations } => {
201.         self.history_state.conversations = conversations
202.         cx.notify()
203.       }
204.       
205.       // Settings commands
206.       ViewCommand::ProfilesLoaded { profiles, default_id } => {
207.         self.settings_state.profiles = profiles
208.         self.settings_state.default_profile_id = default_id
209.         cx.notify()
210.       }
211.       
212.       // Error handling
213.       ViewCommand::ShowError { error, context } => {
214.         self.chat_state.error = Some(ErrorInfo { message: error, context, ..default() })
215.         cx.notify()
216.       }
217.       
218.       ViewCommand::ClearError => {
219.         self.chat_state.error = None
220.         cx.notify()
221.       }
222.       
223.       // ... remaining 20+ ViewCommand variants handled similarly
224.       _ => {
225.         tracing::warn!("Unhandled ViewCommand: {:?}", cmd)
226.       }
```

---

## File: `src/ui_gpui/views/main_panel.rs`
## Line Count: ~250
## Dependencies: gpui, crate::ui_gpui::{theme, components, views}
