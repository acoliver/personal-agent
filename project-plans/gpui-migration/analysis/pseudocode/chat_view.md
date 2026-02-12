# ChatView Component Pseudocode

## Overview
Main chat interface with messages, input bar, and streaming support.

---

## Struct Definitions (Lines 1-30)

```
1.  // ChatView is a stateless component that renders ChatState
2.  STRUCT ChatView<'a> {
3.    state: &'a ChatState,
4.    bridge: &'a Arc<GpuiBridge>,
5.  }
6.  
7.  // Message structure (from domain)
8.  STRUCT Message {
9.    id: Uuid,
10.   role: Role,
11.   content: String,
12.   thinking: Option<String>,
13.   tool_calls: Vec<ToolCallInfo>,
14.   model_id: Option<String>,
15.   timestamp: DateTime<Utc>,
16.   is_partial: bool,
17. }
18. 
19. ENUM Role { User, Assistant, System }
20. 
21. STRUCT ToolCallInfo {
22.   tool_name: String,
23.   status: ToolCallStatus,
24.   result: Option<String>,
25.   duration_ms: Option<u64>,
26. }
27. 
28. ENUM ToolCallStatus { Started, Running, Completed, Failed }
```

## Constructor and Main Render (Lines 31-60)

```
29. IMPL<'a> ChatView<'a>:
30.   FUNCTION new(state: &'a ChatState, bridge: &'a Arc<GpuiBridge>) -> Self:
31.     Self { state, bridge }
32. 
33. IMPL<'a> IntoElement FOR ChatView<'a>:
34.   TYPE Element = Div
35.   
36.   FUNCTION into_element(self) -> Self::Element:
37.     div()
38.       .id("chat-view")
39.       .size_full()
40.       .flex()
41.       .flex_col()
42.       .bg(theme::BG_DARKEST)
43.       .child(self.render_top_bar())
44.       .child(self.render_title_bar())
45.       .child(self.render_messages())
46.       .child(self.render_input_bar())
```

## Top Bar (Lines 61-100)

```
47. IMPL<'a> ChatView<'a>:
48.   FUNCTION render_top_bar(&self) -> impl IntoElement:
49.     div()
50.       .id("top-bar")
51.       .w_full()
52.       .h(px(44.0))
53.       .flex()
54.       .flex_row()
55.       .items_center()
56.       .px(px(12.0))
57.       .bg(theme::BG_DARKER)
58.       .border_b_1()
59.       .border_color(theme::BORDER)
60.       // Left: App icon and title
61.       .child(
62.         div()
63.           .flex()
64.           .flex_row()
65.           .items_center()
66.           .gap(px(8.0))
67.           .child(Icon::new("ai_eye").size(px(24.0)).color(theme::ACCENT))
68.           .child(
69.             div()
70.               .text_size(px(14.0))
71.               .font_weight(FontWeight::SEMIBOLD)
72.               .text_color(theme::TEXT_PRIMARY)
73.               .child("PersonalAgent")
74.           )
75.       )
76.       // Spacer
77.       .child(div().flex_1())
78.       // Right: Toolbar buttons
79.       .child(
80.         div()
81.           .flex()
82.           .flex_row()
83.           .gap(px(4.0))
84.           .child(self.render_toolbar_button("[T]", "Toggle thinking", self.state.show_thinking,
85.             UserEvent::ToggleThinking))
86.           .child(self.render_toolbar_button("[H]", "History", false,
87.             UserEvent::Navigate { view_id: ViewId::History }))
88.           .child(self.render_toolbar_button("[+]", "New conversation", false,
89.             UserEvent::NewConversation))
90.           .child(self.render_toolbar_button("", "Settings", false,
91.             UserEvent::Navigate { view_id: ViewId::Settings }))
92.       )
93. 
94.   FUNCTION render_toolbar_button(&self, icon: &str, tooltip: &str, active: bool, event: UserEvent) -> impl IntoElement:
95.     LET bridge = self.bridge.clone()
96.     div()
97.       .w(px(28.0))
98.       .h(px(28.0))
99.       .flex()
100.      .items_center()
101.      .justify_center()
102.      .rounded(px(4.0))
103.      .cursor_pointer()
104.      .when(active, |d| d.bg(theme::ACCENT.opacity(0.2)))
105.      .hover(|d| d.bg(theme::BG_DARK))
106.      .text_color(IF active THEN theme::ACCENT ELSE theme::TEXT_SECONDARY)
107.      .child(icon)
108.      .on_click(move |_, cx| {
109.        bridge.emit_user_event(event.clone())
110.      })
```

## Title Bar with Conversation Dropdown (Lines 101-140)

```
111.  FUNCTION render_title_bar(&self) -> impl IntoElement:
112.    div()
113.      .id("title-bar")
114.      .w_full()
115.      .h(px(36.0))
116.      .flex()
117.      .flex_row()
118.      .items_center()
119.      .justify_between()
120.      .px(px(12.0))
121.      .bg(theme::BG_DARKER)
122.      .border_b_1()
123.      .border_color(theme::BORDER)
124.      // Conversation title/dropdown
125.      .child(
126.        div()
127.          .flex()
128.          .flex_row()
129.          .items_center()
130.          .gap(px(4.0))
131.          .cursor_pointer()
132.          .child(
133.            div()
134.              .text_size(px(13.0))
135.              .text_color(theme::TEXT_PRIMARY)
136.              .child(&self.state.conversation_title)
137.          )
138.          .child(Icon::new("chevron_down").size(px(12.0)).color(theme::TEXT_SECONDARY))
139.          // TODO: Dropdown menu for conversation switching
140.      )
141.      // Model selector (if profile selected)
142.      .child(
143.        IF LET Some(profile) = &self.state.selected_profile:
144.          div()
145.            .text_size(px(11.0))
146.            .text_color(theme::TEXT_MUTED)
147.            .child(format!("{}", profile.name))
148.        ELSE:
149.          div()
150.      )
```

## Messages Area (Lines 141-220)

```
151.  FUNCTION render_messages(&self) -> impl IntoElement:
152.    // Scrollable message container
153.    uniform_list(
154.      self.message_items(),
155.      "messages-list",
156.      |this, range, cx| {
157.        this.render_message_range(range)
158.      }
159.    )
160.    .flex_1()
161.    .p(px(12.0))
162. 
163.  FUNCTION message_items(&self) -> Vec<MessageItem>:
164.    LET mut items = Vec::new()
165.    
166.    FOR msg IN &self.state.messages:
167.      items.push(MessageItem::Stored(msg.clone()))
168.    
169.    // Add streaming message if active
170.    IF self.state.is_streaming:
171.      items.push(MessageItem::Streaming {
172.        content: self.state.current_response.clone(),
173.        thinking: self.state.current_thinking.clone(),
174.      })
175.    
176.    items
177. 
178.  FUNCTION render_message_item(&self, item: &MessageItem) -> impl IntoElement:
179.    MATCH item:
180.      MessageItem::Stored(msg) => {
181.        IF msg.role == Role::User:
182.          self.render_user_bubble(&msg.content)
183.        ELSE:
184.          self.render_assistant_group(msg)
185.      }
186.      MessageItem::Streaming { content, thinking } => {
187.        self.render_streaming_assistant(content, thinking)
188.      }
189. 
190.  FUNCTION render_user_bubble(&self, content: &str) -> impl IntoElement:
191.    div()
192.      .w_full()
193.      .flex()
194.      .justify_end()
195.      .child(
196.        div()
197.          .max_w(rems(24.0))
198.          .px(px(12.0))
199.          .py(px(8.0))
200.          .rounded(px(12.0))
201.          .bg(theme::USER_BUBBLE_BG)
202.          .text_color(theme::TEXT_PRIMARY)
203.          .text_size(px(14.0))
204.          .child(content)
205.      )
206. 
207.  FUNCTION render_assistant_group(&self, msg: &Message) -> impl IntoElement:
208.    div()
209.      .w_full()
210.      .flex()
211.      .flex_col()
212.      .gap(px(4.0))
213.      // Model label
214.      .child(
215.        div()
216.          .text_size(px(11.0))
217.          .text_color(theme::TEXT_MUTED)
218.          .child(msg.model_id.as_deref().unwrap_or("Assistant"))
219.      )
220.      // Thinking section (collapsible)
221.      .when(self.state.show_thinking && msg.thinking.is_some(), |d| {
222.        d.child(self.render_thinking_section(msg.thinking.as_ref().unwrap()))
223.      })
224.      // Tool calls
225.      .children(msg.tool_calls.iter().map(|tc| self.render_tool_call(tc)))
226.      // Main content
227.      .child(self.render_assistant_bubble(&msg.content))
228. 
229.  FUNCTION render_thinking_section(&self, thinking: &str) -> impl IntoElement:
230.    div()
231.      .w_full()
232.      .max_w(rems(28.0))
233.      .px(px(12.0))
234.      .py(px(8.0))
235.      .rounded(px(8.0))
236.      .bg(theme::THINKING_BG)
237.      .border_l_2()
238.      .border_color(theme::THINKING_BORDER)
239.      .text_size(px(13.0))
240.      .text_color(theme::TEXT_SECONDARY)
241.      .italic()
242.      .child(thinking)
243. 
244.  FUNCTION render_assistant_bubble(&self, content: &str) -> impl IntoElement:
245.    div()
246.      .max_w(rems(28.0))
247.      .px(px(12.0))
248.      .py(px(8.0))
249.      .rounded(px(12.0))
250.      .bg(theme::ASSISTANT_BUBBLE_BG)
251.      .text_color(theme::TEXT_PRIMARY)
252.      .text_size(px(14.0))
253.      // TODO: Markdown rendering
254.      .child(content)
```

## Input Bar (Lines 221-280)

```
255.  FUNCTION render_input_bar(&self) -> impl IntoElement:
256.    LET bridge = self.bridge.clone()
257.    LET is_streaming = self.state.is_streaming
258.    
259.    div()
260.      .id("input-bar")
261.      .w_full()
262.      .p(px(12.0))
263.      .bg(theme::BG_DARKER)
264.      .border_t_1()
265.      .border_color(theme::BORDER)
266.      .child(
267.        div()
268.          .w_full()
269.          .flex()
270.          .flex_row()
271.          .items_end()
272.          .gap(px(8.0))
273.          .p(px(8.0))
274.          .rounded(px(12.0))
275.          .bg(theme::INPUT_BG)
276.          .child(
277.            // Text input area
278.            TextArea::new("message-input")
279.              .placeholder("Send a message...")
280.              .flex_1()
281.              .min_h(px(36.0))
282.              .max_h(px(120.0))
283.              .text_size(px(14.0))
284.              .disabled(is_streaming)
285.          )
286.          .child(
287.            // Send or Stop button
288.            IF is_streaming:
289.              self.render_stop_button(bridge.clone())
290.            ELSE:
291.              self.render_send_button(bridge.clone())
292.          )
293.      )
294. 
295.  FUNCTION render_send_button(&self, bridge: Arc<GpuiBridge>) -> impl IntoElement:
296.    div()
297.      .w(px(36.0))
298.      .h(px(36.0))
299.      .flex()
300.      .items_center()
301.      .justify_center()
302.      .rounded_full()
303.      .bg(theme::ACCENT)
304.      .cursor_pointer()
305.      .hover(|d| d.bg(theme::ACCENT_HOVER))
306.      .child(Icon::new("arrow_up").size(px(18.0)).color(theme::TEXT_ON_ACCENT))
307.      .on_click(move |_, cx| {
308.        // Get text from input and emit
309.        // TODO: Access TextArea value
310.        LET text = get_input_text(cx)
311.        IF !text.is_empty():
312.          bridge.emit_user_event(UserEvent::SendMessage { text })
313.      })
314. 
315.  FUNCTION render_stop_button(&self, bridge: Arc<GpuiBridge>) -> impl IntoElement:
316.    div()
317.      .w(px(36.0))
318.      .h(px(36.0))
319.      .flex()
320.      .items_center()
321.      .justify_center()
322.      .rounded_full()
323.      .bg(theme::ERROR)
324.      .cursor_pointer()
325.      .hover(|d| d.bg(theme::ERROR_HOVER))
326.      .child(Icon::new("stop").size(px(18.0)).color(theme::TEXT_ON_ACCENT))
327.      .on_click(move |_, cx| {
328.        bridge.emit_user_event(UserEvent::StopStreaming)
329.      })
```

## Streaming Assistant (Lines 281-310)

```
330.  FUNCTION render_streaming_assistant(&self, content: &str, thinking: &str) -> impl IntoElement:
331.    div()
332.      .w_full()
333.      .flex()
334.      .flex_col()
335.      .gap(px(4.0))
336.      // Model label with streaming indicator
337.      .child(
338.        div()
339.          .flex()
340.          .flex_row()
341.          .gap(px(4.0))
342.          .child(
343.            div()
344.              .text_size(px(11.0))
345.              .text_color(theme::TEXT_MUTED)
346.              .child("Assistant")
347.          )
348.          .child(StreamingIndicator::new())
349.      )
350.      // Thinking section (if showing)
351.      .when(self.state.show_thinking && !thinking.is_empty(), |d| {
352.        d.child(self.render_thinking_section(thinking))
353.      })
354.      // Content (partial)
355.      .when(!content.is_empty(), |d| {
356.        d.child(self.render_assistant_bubble(content))
357.      })
358.      // Cursor animation when waiting
359.      .when(content.is_empty() && thinking.is_empty(), |d| {
360.        d.child(TypingIndicator::new())
361.      })
```

---

## File: `src/ui_gpui/views/chat_view.rs`
## Line Count: ~360
## Dependencies: gpui, crate::ui_gpui::{theme, components, bridge}
