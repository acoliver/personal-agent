# Reusable Components Pseudocode

## Overview
Small reusable components used across views.

---

## Icon Component (Lines 1-40)

```
1.  // Icon component for SVG icons
2.  STRUCT Icon {
3.    name: SharedString,
4.    size: Pixels,
5.    color: Hsla,
6.  }
7.  
8.  IMPL Icon:
9.    FUNCTION new(name: impl Into<SharedString>) -> Self:
10.     Self {
11.       name: name.into(),
12.       size: px(16.0),
13.       color: theme::TEXT_PRIMARY,
14.     }
15.   
16.   FUNCTION size(mut self, size: Pixels) -> Self:
17.     self.size = size
18.     self
19.   
20.   FUNCTION color(mut self, color: Hsla) -> Self:
21.     self.color = color
22.     self
23. 
24. IMPL IntoElement FOR Icon:
25.   TYPE Element = Svg
26.   
27.   FUNCTION into_element(self) -> Self::Element:
28.     svg()
29.       .path(format!("assets/icons/{}.svg", self.name))
30.       .size(self.size)
31.       .text_color(self.color)
```

## Button Component (Lines 41-90)

```
32. STRUCT Button {
33.   label: SharedString,
34.   variant: ButtonVariant,
35.   disabled: bool,
36.   on_click: Option<Box<dyn Fn(&ClickEvent, &mut WindowContext) + 'static>>,
37. }
38. 
39. ENUM ButtonVariant { Primary, Secondary, Danger, Ghost }
40. 
41. IMPL Button:
42.   FUNCTION new(label: impl Into<SharedString>) -> Self:
43.     Self {
44.       label: label.into(),
45.       variant: ButtonVariant::Secondary,
46.       disabled: false,
47.       on_click: None,
48.     }
49.   
50.   FUNCTION variant(mut self, variant: ButtonVariant) -> Self:
51.     self.variant = variant
52.     self
53.   
54.   FUNCTION disabled(mut self, disabled: bool) -> Self:
55.     self.disabled = disabled
56.     self
57.   
58.   FUNCTION on_click(mut self, f: impl Fn(&ClickEvent, &mut WindowContext) + 'static) -> Self:
59.     self.on_click = Some(Box::new(f))
60.     self
61. 
62. IMPL IntoElement FOR Button:
63.   TYPE Element = Div
64.   
65.   FUNCTION into_element(self) -> Self::Element:
66.     LET (bg, fg, hover_bg) = MATCH self.variant:
67.       ButtonVariant::Primary => (theme::ACCENT, theme::TEXT_ON_ACCENT, theme::ACCENT_HOVER)
68.       ButtonVariant::Secondary => (theme::BG_DARK, theme::TEXT_PRIMARY, theme::BG_DARKER)
69.       ButtonVariant::Danger => (theme::ERROR, theme::TEXT_ON_ACCENT, theme::ERROR_HOVER)
70.       ButtonVariant::Ghost => (transparent(), theme::TEXT_SECONDARY, theme::BG_DARK)
71.     
72.     div()
73.       .px(px(16.0))
74.       .py(px(8.0))
75.       .rounded(px(6.0))
76.       .bg(bg)
77.       .text_color(fg)
78.       .text_size(px(14.0))
79.       .font_weight(FontWeight::MEDIUM)
80.       .cursor_pointer()
81.       .when(!self.disabled, |d| d.hover(|d| d.bg(hover_bg)))
82.       .when(self.disabled, |d| d.opacity(0.5).cursor_not_allowed())
83.       .child(&self.label)
84.       .when_some(self.on_click, |d, on_click| d.on_click(on_click))
```

## TextArea Component (Lines 91-140)

```
85. STRUCT TextArea {
86.   id: ElementId,
87.   value: String,
88.   placeholder: SharedString,
89.   disabled: bool,
90.   min_h: Pixels,
91.   max_h: Pixels,
92. }
93. 
94. IMPL TextArea:
95.   FUNCTION new(id: impl Into<ElementId>) -> Self:
96.     Self {
97.       id: id.into(),
98.       value: String::new(),
99.       placeholder: "".into(),
100.      disabled: false,
101.      min_h: px(36.0),
102.      max_h: px(120.0),
103.    }
104.  
105.  FUNCTION placeholder(mut self, text: impl Into<SharedString>) -> Self:
106.    self.placeholder = text.into()
107.    self
108.  
109.  FUNCTION disabled(mut self, disabled: bool) -> Self:
110.    self.disabled = disabled
111.    self
112.  
113.  // ... more builder methods
114. 
115. IMPL IntoElement FOR TextArea:
116.   // Uses GPUI's text input primitives
117.   FUNCTION into_element(self) -> impl IntoElement:
118.     div()
119.       .id(self.id)
120.       .flex_1()
121.       .min_h(self.min_h)
122.       .max_h(self.max_h)
123.       .overflow_y_auto()
124.       // GPUI text editing integration
125.       .child(TextInput::new()
126.         .placeholder(&self.placeholder)
127.         .disabled(self.disabled))
```

## StreamingIndicator Component (Lines 141-170)

```
128. // Animated dots for streaming state
129. STRUCT StreamingIndicator;
130. 
131. IMPL StreamingIndicator:
132.   FUNCTION new() -> Self:
133.     Self
134. 
135. IMPL IntoElement FOR StreamingIndicator:
136.   FUNCTION into_element(self) -> impl IntoElement:
137.     div()
138.       .flex()
139.       .flex_row()
140.       .gap(px(2.0))
141.       .child(Dot::new().delay(0))
142.       .child(Dot::new().delay(150))
143.       .child(Dot::new().delay(300))
144. 
145. STRUCT Dot {
146.   delay_ms: u64,
147. }
148. 
149. IMPL Dot:
150.   FUNCTION new() -> Self:
151.     Self { delay_ms: 0 }
152.   
153.   FUNCTION delay(mut self, ms: u64) -> Self:
154.     self.delay_ms = ms
155.     self
156. 
157. IMPL IntoElement FOR Dot:
158.   FUNCTION into_element(self) -> impl IntoElement:
159.     div()
160.       .w(px(6.0))
161.       .h(px(6.0))
162.       .rounded_full()
163.       .bg(theme::TEXT_MUTED)
164.       // Animation via GPUI animation system
165.       .with_animation(
166.         Animation::pulse()
167.           .duration(Duration::from_millis(1000))
168.           .delay(Duration::from_millis(self.delay_ms))
169.       )
```

## TypingIndicator Component (Lines 171-200)

```
170. // Cursor blink animation
171. STRUCT TypingIndicator;
172. 
173. IMPL TypingIndicator:
174.   FUNCTION new() -> Self:
175.     Self
176. 
177. IMPL IntoElement FOR TypingIndicator:
178.   FUNCTION into_element(self) -> impl IntoElement:
179.     div()
180.       .flex()
181.       .items_center()
182.       .h(px(20.0))
183.       .child(
184.         div()
185.           .w(px(2.0))
186.           .h(px(16.0))
187.           .bg(theme::ACCENT)
188.           .with_animation(
189.             Animation::blink()
190.               .duration(Duration::from_millis(500))
191.           )
192.       )
```

## ConversationRow Component (Lines 201-250)

```
193. // Row in conversation history list
194. STRUCT ConversationRow<'a> {
195.   conversation: &'a ConversationMeta,
196.   is_selected: bool,
197.   bridge: &'a Arc<GpuiBridge>,
198. }
199. 
200. IMPL<'a> ConversationRow<'a>:
201.   FUNCTION new(conv: &'a ConversationMeta, bridge: &'a Arc<GpuiBridge>) -> Self:
202.     Self {
203.       conversation: conv,
204.       is_selected: false,
205.       bridge,
206.     }
207.   
208.   FUNCTION selected(mut self, selected: bool) -> Self:
209.     self.is_selected = selected
210.     self
211. 
212. IMPL<'a> IntoElement FOR ConversationRow<'a>:
213.   FUNCTION into_element(self) -> impl IntoElement:
214.     LET id = self.conversation.id
215.     LET bridge = self.bridge.clone()
216.     
217.     div()
218.       .w_full()
219.       .px(px(12.0))
220.       .py(px(10.0))
221.       .flex()
222.       .flex_col()
223.       .gap(px(2.0))
224.       .rounded(px(6.0))
225.       .cursor_pointer()
226.       .when(self.is_selected, |d| d.bg(theme::BG_DARK))
227.       .hover(|d| d.bg(theme::BG_DARKER))
228.       // Title
229.       .child(
230.         div()
231.           .text_size(px(14.0))
232.           .text_color(theme::TEXT_PRIMARY)
233.           .truncate()
234.           .child(&self.conversation.title)
235.       )
236.       // Timestamp
237.       .child(
238.         div()
239.           .text_size(px(11.0))
240.           .text_color(theme::TEXT_MUTED)
241.           .child(format_relative_time(self.conversation.updated_at))
242.       )
243.       .on_click(move |_, cx| {
244.         bridge.emit_user_event(UserEvent::SelectConversation { id })
245.       })
```

## ProfileCard Component (Lines 251-300)

```
246. // Card for profile selection/editing
247. STRUCT ProfileCard<'a> {
248.   profile: &'a ProfileMeta,
249.   is_default: bool,
250.   bridge: &'a Arc<GpuiBridge>,
251. }
252. 
253. IMPL<'a> ProfileCard<'a>:
254.   FUNCTION new(profile: &'a ProfileMeta, bridge: &'a Arc<GpuiBridge>) -> Self:
255.     Self {
256.       profile,
257.       is_default: false,
258.       bridge,
259.     }
260.   
261.   FUNCTION is_default(mut self, is_default: bool) -> Self:
262.     self.is_default = is_default
263.     self
264. 
265. IMPL<'a> IntoElement FOR ProfileCard<'a>:
266.   FUNCTION into_element(self) -> impl IntoElement:
267.     LET id = self.profile.id
268.     LET bridge = self.bridge.clone()
269.     
270.     div()
271.       .w_full()
272.       .p(px(12.0))
273.       .flex()
274.       .flex_row()
275.       .items_center()
276.       .justify_between()
277.       .rounded(px(8.0))
278.       .bg(theme::BG_DARK)
279.       .border_1()
280.       .border_color(IF self.is_default THEN theme::ACCENT ELSE theme::BORDER)
281.       // Left: Name and model
282.       .child(
283.         div()
284.           .flex()
285.           .flex_col()
286.           .gap(px(2.0))
287.           .child(
288.             div()
289.               .text_size(px(14.0))
290.               .font_weight(FontWeight::MEDIUM)
291.               .text_color(theme::TEXT_PRIMARY)
292.               .child(&self.profile.name)
293.           )
294.           .child(
295.             div()
296.               .text_size(px(12.0))
297.               .text_color(theme::TEXT_SECONDARY)
298.               .child(&self.profile.model_id)
299.           )
300.       )
301.       // Right: Actions
302.       .child(
303.         div()
304.           .flex()
305.           .flex_row()
306.           .gap(px(4.0))
307.           .child(
308.             Button::new("Edit")
309.               .variant(ButtonVariant::Ghost)
310.               .on_click(move |_, cx| {
311.                 bridge.emit_user_event(UserEvent::EditProfile { id })
312.               })
313.           )
314.           .when(!self.is_default, |d| {
315.             d.child(
316.               Button::new("Set Default")
317.                 .variant(ButtonVariant::Ghost)
318.                 .on_click(move |_, cx| {
319.                   bridge.emit_user_event(UserEvent::SetDefaultProfile { id })
320.                 })
321.             )
322.           })
323.       )
```

## Toggle Component (Lines 301-340)

```
324. // On/off toggle switch
325. STRUCT Toggle {
326.   value: bool,
327.   disabled: bool,
328.   on_change: Option<Box<dyn Fn(bool, &mut WindowContext) + 'static>>,
329. }
330. 
331. IMPL Toggle:
332.   FUNCTION new(value: bool) -> Self:
333.     Self {
334.       value,
335.       disabled: false,
336.       on_change: None,
337.     }
338.   
339.   FUNCTION on_change(mut self, f: impl Fn(bool, &mut WindowContext) + 'static) -> Self:
340.     self.on_change = Some(Box::new(f))
341.     self
342. 
343. IMPL IntoElement FOR Toggle:
344.   FUNCTION into_element(self) -> impl IntoElement:
345.     LET value = self.value
346.     
347.     div()
348.       .w(px(44.0))
349.       .h(px(24.0))
350.       .rounded_full()
351.       .bg(IF value THEN theme::ACCENT ELSE theme::BG_DARK)
352.       .cursor_pointer()
353.       .p(px(2.0))
354.       .child(
355.         div()
356.           .w(px(20.0))
357.           .h(px(20.0))
358.           .rounded_full()
359.           .bg(white())
360.           .when(value, |d| d.translate_x(px(20.0)))
361.           .with_animation(Animation::ease_out().duration(Duration::from_millis(150)))
362.       )
363.       .when_some(self.on_change, |d, on_change| {
364.         d.on_click(move |_, cx| on_change(!value, cx))
365.       })
```

---

## File: `src/ui_gpui/components/mod.rs`
## Total Line Count: ~400
## Exports: Icon, Button, TextArea, StreamingIndicator, TypingIndicator, ConversationRow, ProfileCard, Toggle
