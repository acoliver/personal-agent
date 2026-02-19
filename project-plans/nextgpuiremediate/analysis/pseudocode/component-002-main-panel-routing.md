# Component 002: Main Panel Routing

Plan ID: PLAN-20260219-NEXTGPUIREMEDIATE
Component: Main Panel Routing
Created: 2026-02-19

---

## Overview

This component defines the navigation and routing logic for the MainPanel, which is the root container that manages which view is currently displayed. It uses a stack-based NavigationState and responds to navigation events to switch between views.

---

## Requirement Coverage

### REQ-NAV-001: Stack-Based Navigation

**REQ-NAV-001.1**: Navigation MUST use a stack with Chat as root

- **Full Text**: The navigation system must maintain a stack of views where Chat is always the bottom-most (root) element. The stack can never be empty.
- **GIVEN**: A freshly initialized NavigationState
- **WHEN**: current() is called
- **THEN**: Returns ViewId::Chat
- **Why**: Users always have somewhere to go back to; Chat is the app's primary function

**REQ-NAV-001.2**: Navigate forward MUST push new view onto stack

- **Full Text**: When navigating to a new view, that view is pushed onto the stack, preserving the history of visited views for back navigation.
- **GIVEN**: NavigationState with stack [Chat]
- **WHEN**: navigate(Settings) is called
- **THEN**: Stack becomes [Chat, Settings] and current() returns Settings
- **Why**: Enables back navigation to retrace user's path

**REQ-NAV-001.3**: Navigate back MUST pop current view from stack

- **Full Text**: When the user navigates back, the top view is popped from the stack, returning to the previous view. Cannot pop below root.
- **GIVEN**: NavigationState with stack [Chat, Settings]
- **WHEN**: navigate_back() is called
- **THEN**: Stack becomes [Chat] and current() returns Chat
- **Why**: Standard back navigation UX pattern

**REQ-NAV-001.4**: Navigation to same view MUST be a no-op

- **Full Text**: Navigating to the view that is already current should not modify the stack. This prevents accidental stack growth from repeated clicks.
- **GIVEN**: NavigationState with current() == Settings
- **WHEN**: navigate(Settings) is called
- **THEN**: Stack is unchanged, no duplicate Settings added
- **Why**: Prevents stack pollution from double-clicks or re-renders

### REQ-NAV-002: View Rendering

**REQ-NAV-002.1**: MainPanel MUST render the current view from NavigationState

- **Full Text**: The MainPanel component must display whichever view is currently on top of the navigation stack. Changing the current view must update what is rendered.
- **GIVEN**: NavigationState with current() == History
- **WHEN**: MainPanel renders
- **THEN**: HistoryView is displayed (not ChatView, SettingsView, etc.)
- **Why**: Single source of truth for which view is active

**REQ-NAV-002.2**: View changes MUST trigger re-render

- **Full Text**: When NavigationState changes (navigate or navigate_back), the MainPanel must re-render to show the new current view.
- **GIVEN**: MainPanel displaying ChatView
- **WHEN**: navigate(Settings) is called
- **THEN**: MainPanel re-renders to display SettingsView
- **Why**: Navigation must be visually responsive

**REQ-NAV-002.3**: Navigation MUST emit NavigationEvent::Navigated

- **Full Text**: After navigation completes, a NavigationEvent::Navigated event must be emitted so presenters can react to view changes (e.g., load data).
- **GIVEN**: User clicks Settings button in ChatView
- **WHEN**: Navigation to Settings completes
- **THEN**: EventBus receives NavigationEvent::Navigated { view: Settings }
- **Why**: Presenters need to know when their view becomes active

### REQ-NAV-003: Navigation Routing

**REQ-NAV-003.1**: UserEvent::Navigate MUST be handled by navigation system

- **Full Text**: When a UserEvent::Navigate event is emitted (from a view), the navigation system must process it and update NavigationState accordingly.
- **GIVEN**: View emits UserEvent::Navigate { to: History }
- **WHEN**: Event is handled
- **THEN**: NavigationState.navigate(History) is called
- **Why**: Decouples views from navigation logic

**REQ-NAV-003.2**: UserEvent::NavigateBack MUST trigger back navigation

- **Full Text**: When a UserEvent::NavigateBack event is emitted, navigate_back() must be called on NavigationState.
- **GIVEN**: View emits UserEvent::NavigateBack
- **WHEN**: Event is handled
- **THEN**: NavigationState.navigate_back() is called
- **Why**: Back buttons work consistently across all views

**REQ-NAV-003.3**: ViewId MUST map to correct view component

- **Full Text**: Each ViewId enum variant must be mapped to exactly one view component. The mapping must be exhaustive.
- **GIVEN**: ViewId::ProfileEditor { id: Some(uuid) }
- **WHEN**: MainPanel renders
- **THEN**: ProfileEditorView is rendered with that profile ID
- **Why**: Ensures no orphan ViewIds or missing view components

---

## Pseudocode

### NavigationState Implementation

```pseudocode
001: MODULE NavigationState
002: 
003: STRUCT NavigationState
004:   stack: Vec<ViewId>
005: END STRUCT
006: 
007: // REQ-NAV-001.1: Initialize with Chat as root
008: FUNCTION new() -> NavigationState
009:   RETURN NavigationState {
010:     stack: vec![ViewId::Chat]
011:   }
012: END FUNCTION
013: 
014: // REQ-NAV-001.1, REQ-NAV-002.1: Get current (top) view
015: FUNCTION current(self) -> ViewId
016:   // Stack is never empty (Chat is always at bottom)
017:   RETURN self.stack.last().copied().unwrap_or(ViewId::Chat)
018: END FUNCTION
019: 
020: // Helper: Check if can navigate back
021: FUNCTION can_go_back(self) -> bool
022:   RETURN self.stack.len() > 1
023: END FUNCTION
024: 
025: // REQ-NAV-001.2, REQ-NAV-001.4: Navigate forward to new view
026: FUNCTION navigate(self, to: ViewId)
027:   // REQ-NAV-001.4: Don't push if already at target
028:   IF self.current() == to THEN
029:     tracing::trace!("Navigation to same view {:?}, ignoring", to)
030:     RETURN
031:   END IF
032:   
033:   tracing::debug!("Navigating from {:?} to {:?}", self.current(), to)
034:   
035:   // REQ-NAV-001.2: Push new view onto stack
036:   self.stack.push(to)
037: END FUNCTION
038: 
039: // REQ-NAV-001.3: Navigate back (pop stack)
040: FUNCTION navigate_back(self) -> bool
041:   IF self.stack.len() > 1 THEN
042:     LET from = self.stack.pop()
043:     tracing::debug!("Navigated back from {:?} to {:?}", from, self.current())
044:     RETURN true
045:   ELSE
046:     // Already at root, cannot go back further
047:     tracing::trace!("Already at root (Chat), cannot navigate back")
048:     RETURN false
049:   END IF
050: END FUNCTION
051: 
052: // Get stack depth (for debugging/testing)
053: FUNCTION stack_depth(self) -> usize
054:   RETURN self.stack.len()
055: END FUNCTION
056: 
057: END MODULE
```

### ViewId Enumeration

```pseudocode
058: MODULE ViewId
059: 
060: // REQ-NAV-003.3: All navigable views
061: ENUM ViewId
062:   Chat,                             // Main chat interface
063:   History,                          // Conversation history list
064:   Settings,                         // Profile & MCP management
065:   ProfileEditor { id: Option<Uuid> }, // None = new, Some = edit
066:   McpAdd,                           // Search/select MCP
067:   McpConfigure { id: Uuid },        // Configure specific MCP
068:   ModelSelector,                    // Choose provider/model
069: END ENUM
070: 
071: // REQ-NAV-003.3: ViewId equality (ignoring parameterized variants)
072: // Note: ProfileEditor{None} != ProfileEditor{Some(id)}
073: IMPL PartialEq FOR ViewId
074:   FUNCTION eq(self, other: ViewId) -> bool
075:     MATCH (self, other)
076:       (Chat, Chat) => true,
077:       (History, History) => true,
078:       (Settings, Settings) => true,
079:       (ProfileEditor { id: a }, ProfileEditor { id: b }) => a == b,
080:       (McpAdd, McpAdd) => true,
081:       (McpConfigure { id: a }, McpConfigure { id: b }) => a == b,
082:       (ModelSelector, ModelSelector) => true,
083:       _ => false,
084:     END MATCH
085:   END FUNCTION
086: END IMPL
087: 
088: END MODULE
```

### MainPanel Implementation

```pseudocode
089: MODULE MainPanel
090: 
091: STRUCT MainPanel
092:   navigation: Model<NavigationState>
093:   user_event_emitter: Arc<dyn Fn(UserEvent)>
094:   
095:   // View instances (created once, reused)
096:   chat_view: ChatView,
097:   history_view: HistoryView,
098:   settings_view: SettingsView,
099:   profile_editor_view: ProfileEditorView,
100:   mcp_add_view: McpAddView,
101:   mcp_configure_view: McpConfigureView,
102:   model_selector_view: ModelSelectorView,
103: END STRUCT
104: 
105: IMPL Render FOR MainPanel
106:   // REQ-NAV-002.1, REQ-NAV-002.2: Render current view
107:   FUNCTION render(self, cx: &mut ViewContext<Self>) -> impl IntoElement
108:     LET current_view = self.navigation.read(cx).current()
109:     
110:     tracing::trace!("MainPanel rendering view: {:?}", current_view)
111:     
112:     // REQ-NAV-003.3: Map ViewId to view component
113:     LET content = MATCH current_view
114:       ViewId::Chat => {
115:         self.chat_view.clone().into_any_element()
116:       }
117:       ViewId::History => {
118:         self.history_view.clone().into_any_element()
119:       }
120:       ViewId::Settings => {
121:         self.settings_view.clone().into_any_element()
122:       }
123:       ViewId::ProfileEditor { id } => {
124:         // Pass profile ID to editor
125:         self.profile_editor_view.set_profile_id(id)
126:         self.profile_editor_view.clone().into_any_element()
127:       }
128:       ViewId::McpAdd => {
129:         self.mcp_add_view.clone().into_any_element()
130:       }
131:       ViewId::McpConfigure { id } => {
132:         // Pass MCP ID to configure view
133:         self.mcp_configure_view.set_mcp_id(id)
134:         self.mcp_configure_view.clone().into_any_element()
135:       }
136:       ViewId::ModelSelector => {
137:         self.model_selector_view.clone().into_any_element()
138:       }
139:     END MATCH
140:     
141:     // Wrap in container
142:     RETURN div()
143:       .size_full()
144:       .child(content)
145:   END FUNCTION
146: END IMPL
147: 
148: // Handle navigation commands from NavigationChannel
149: FUNCTION handle_navigation_command(self, command: NavigationCommand, cx: &mut ViewContext<Self>)
150:   MATCH command
151:     NavigationCommand::Navigate { to } => {
152:       self.navigation.update(cx, |nav, _| {
153:         nav.navigate(to.clone())
154:       })
155:       // REQ-NAV-002.3: Emit navigation event
156:       emit_navigation_event(to.clone())
157:       // REQ-NAV-002.2: Trigger re-render
158:       cx.notify()
159:     }
160:     NavigationCommand::NavigateBack => {
161:       LET navigated = self.navigation.update(cx, |nav, _| {
162:         nav.navigate_back()
163:       })
164:       IF navigated THEN
165:         LET current = self.navigation.read(cx).current()
166:         emit_navigation_event(current)
167:         cx.notify()
168:       END IF
169:     }
170:   END MATCH
171: END FUNCTION
172: 
173: END MODULE
```

### NavigationChannel (GPUI ↔ Navigation Communication)

```pseudocode
174: MODULE NavigationChannel
175: 
176: ENUM NavigationCommand
177:   Navigate { to: ViewId },
178:   NavigateBack,
179: END ENUM
180: 
181: STRUCT NavigationChannel
182:   sender: async_channel::Sender<NavigationCommand>
183:   receiver: async_channel::Receiver<NavigationCommand>
184: END STRUCT
185: 
186: FUNCTION new() -> NavigationChannel
187:   LET (sender, receiver) = async_channel::bounded(32)
188:   RETURN NavigationChannel { sender, receiver }
189: END FUNCTION
190: 
191: // Called from tokio side (e.g., presenter handling UserEvent::Navigate)
192: FUNCTION send_navigation(self, command: NavigationCommand)
193:   IF self.sender.try_send(command).is_err() THEN
194:     tracing::warn!("Navigation channel full")
195:   END IF
196: END FUNCTION
197: 
198: // Called from GPUI side to poll for commands
199: FUNCTION try_recv(self) -> Option<NavigationCommand>
200:   self.receiver.try_recv().ok()
201: END FUNCTION
202: 
203: END MODULE
```

### Navigation Event Handler (Presenter-Side)

```pseudocode
204: MODULE NavigationEventHandler
205: 
206: // REQ-NAV-003.1, REQ-NAV-003.2: Handle navigation UserEvents
207: FUNCTION handle_navigation_user_event(
208:   event: UserEvent,
209:   nav_channel: Arc<NavigationChannel>
210: )
211:   MATCH event
212:     // REQ-NAV-003.1: Handle Navigate event
213:     UserEvent::Navigate { to } => {
214:       tracing::debug!("Handling UserEvent::Navigate to {:?}", to)
215:       nav_channel.send_navigation(NavigationCommand::Navigate { to })
216:     }
217:     
218:     // REQ-NAV-003.2: Handle NavigateBack event
219:     UserEvent::NavigateBack => {
220:       tracing::debug!("Handling UserEvent::NavigateBack")
221:       nav_channel.send_navigation(NavigationCommand::NavigateBack)
222:     }
223:     
224:     // Not a navigation event
225:     _ => {}
226:   END MATCH
227: END FUNCTION
228: 
229: // REQ-NAV-002.3: Emit NavigationEvent after navigation completes
230: FUNCTION emit_navigation_event(view: ViewId)
231:   LET event = NavigationEvent::Navigated { view: view.clone() }
232:   events::global::emit(AppEvent::Navigation(event))
233:   tracing::debug!("Emitted NavigationEvent::Navigated for {:?}", view)
234: END FUNCTION
235: 
236: END MODULE
```

### MainPanel Event Loop Integration

```pseudocode
237: MODULE MainPanelEventLoop
238: 
239: // Integrate navigation polling into GPUI event loop
240: FUNCTION setup_navigation_polling(
241:   main_panel: View<MainPanel>,
242:   nav_channel: Arc<NavigationChannel>,
243:   cx: &mut AppContext
244: )
245:   // Poll for navigation commands on each frame
246:   cx.spawn(|cx| async move {
247:     LOOP
248:       // Check for navigation commands
249:       WHILE let Some(command) = nav_channel.try_recv() DO
250:         cx.update(|cx| {
251:           main_panel.update(cx, |panel, cx| {
252:             panel.handle_navigation_command(command, cx)
253:           })
254:         }).ok()
255:       END WHILE
256:       
257:       // Yield to allow other async work
258:       smol::Timer::after(Duration::from_millis(16)).await
259:     END LOOP
260:   }).detach()
261: END FUNCTION
262: 
263: END MODULE
```

### View-Specific Navigation Helpers

```pseudocode
264: MODULE ViewNavigationHelpers
265: 
266: // Common navigation pattern for views with back buttons
267: FUNCTION render_back_button(emitter: Arc<dyn Fn(UserEvent)>) -> impl IntoElement
268:   RETURN Button::new("back", "<")
269:     .on_click(move |_, cx| {
270:       emitter(UserEvent::NavigateBack)
271:     })
272: END FUNCTION
273: 
274: // Navigate to Settings (from Chat top bar)
275: FUNCTION navigate_to_settings(emitter: Arc<dyn Fn(UserEvent)>)
276:   emitter(UserEvent::Navigate { to: ViewId::Settings })
277: END FUNCTION
278: 
279: // Navigate to History (from Chat top bar)
280: FUNCTION navigate_to_history(emitter: Arc<dyn Fn(UserEvent)>)
281:   emitter(UserEvent::Navigate { to: ViewId::History })
282: END FUNCTION
283: 
284: // Navigate to ProfileEditor for new profile
285: FUNCTION navigate_to_new_profile(emitter: Arc<dyn Fn(UserEvent)>)
286:   emitter(UserEvent::Navigate { 
287:     to: ViewId::ProfileEditor { id: None } 
288:   })
289: END FUNCTION
290: 
291: // Navigate to ProfileEditor for existing profile
292: FUNCTION navigate_to_edit_profile(emitter: Arc<dyn Fn(UserEvent)>, profile_id: Uuid)
293:   emitter(UserEvent::Navigate { 
294:     to: ViewId::ProfileEditor { id: Some(profile_id) } 
295:   })
296: END FUNCTION
297: 
298: // Navigate to McpConfigure for existing MCP
299: FUNCTION navigate_to_mcp_configure(emitter: Arc<dyn Fn(UserEvent)>, mcp_id: Uuid)
300:   emitter(UserEvent::Navigate { 
301:     to: ViewId::McpConfigure { id: mcp_id } 
302:   })
303: END FUNCTION
304: 
305: // Navigate to ModelSelector (first step in add profile)
306: FUNCTION navigate_to_model_selector(emitter: Arc<dyn Fn(UserEvent)>)
307:   emitter(UserEvent::Navigate { to: ViewId::ModelSelector })
308: END FUNCTION
309: 
310: // Navigate to McpAdd (first step in add MCP)
311: FUNCTION navigate_to_mcp_add(emitter: Arc<dyn Fn(UserEvent)>)
312:   emitter(UserEvent::Navigate { to: ViewId::McpAdd })
313: END FUNCTION
314: 
315: END MODULE
```

---

## Test Scenarios

### Test: Initial State is Chat

```pseudocode
TEST navigation_initial_state_is_chat
  // REQ-NAV-001.1
  GIVEN freshly created NavigationState
  
  THEN nav.current() == ViewId::Chat
  AND nav.stack_depth() == 1
  AND nav.can_go_back() == false
END TEST
```

### Test: Navigate Forward Pushes to Stack

```pseudocode
TEST navigation_forward_pushes_stack
  // REQ-NAV-001.2
  GIVEN NavigationState with stack [Chat]
  
  WHEN nav.navigate(ViewId::Settings)
  
  THEN nav.current() == ViewId::Settings
  AND nav.stack_depth() == 2
  AND nav.can_go_back() == true
END TEST
```

### Test: Navigate Back Pops Stack

```pseudocode
TEST navigation_back_pops_stack
  // REQ-NAV-001.3
  GIVEN NavigationState with stack [Chat, Settings, ProfileEditor]
  
  WHEN nav.navigate_back()
  
  THEN nav.current() == ViewId::Settings
  AND nav.stack_depth() == 2
  
  WHEN nav.navigate_back()
  
  THEN nav.current() == ViewId::Chat
  AND nav.stack_depth() == 1
END TEST
```

### Test: Navigate Back at Root is No-Op

```pseudocode
TEST navigation_back_at_root_noop
  // REQ-NAV-001.3
  GIVEN NavigationState with stack [Chat]
  
  WHEN result = nav.navigate_back()
  
  THEN result == false
  AND nav.current() == ViewId::Chat
  AND nav.stack_depth() == 1
END TEST
```

### Test: Navigate to Same View is No-Op

```pseudocode
TEST navigation_to_same_view_noop
  // REQ-NAV-001.4
  GIVEN NavigationState with stack [Chat, Settings]
  
  WHEN nav.navigate(ViewId::Settings)
  
  THEN nav.stack_depth() == 2  // Not 3
  AND nav.current() == ViewId::Settings
END TEST
```

### Test: MainPanel Renders Correct View

```pseudocode
TEST main_panel_renders_correct_view
  // REQ-NAV-002.1
  GIVEN MainPanel with navigation.current() == ViewId::History
  
  WHEN MainPanel.render() is called
  
  THEN HistoryView is rendered
  AND ChatView is NOT rendered
  AND SettingsView is NOT rendered
END TEST
```

### Test: Navigation Emits Event

```pseudocode
TEST navigation_emits_navigated_event
  // REQ-NAV-002.3
  GIVEN EventBus subscriber
  AND NavigationState at Chat
  
  WHEN nav.navigate(ViewId::Settings) completes
  AND emit_navigation_event(Settings) is called
  
  THEN subscriber receives NavigationEvent::Navigated { view: Settings }
END TEST
```

### Test: UserEvent::Navigate Handled

```pseudocode
TEST user_event_navigate_handled
  // REQ-NAV-003.1
  GIVEN NavigationEventHandler with nav_channel
  
  WHEN handle_navigation_user_event(
    UserEvent::Navigate { to: ViewId::History }
  )
  
  THEN nav_channel received NavigationCommand::Navigate { to: History }
END TEST
```

### Test: UserEvent::NavigateBack Handled

```pseudocode
TEST user_event_navigate_back_handled
  // REQ-NAV-003.2
  GIVEN NavigationEventHandler with nav_channel
  
  WHEN handle_navigation_user_event(UserEvent::NavigateBack)
  
  THEN nav_channel received NavigationCommand::NavigateBack
END TEST
```

### Test: ViewId Maps to Correct Component

```pseudocode
TEST viewid_maps_to_correct_component
  // REQ-NAV-003.3
  FOR EACH (view_id, expected_view) IN [
    (ViewId::Chat, ChatView),
    (ViewId::History, HistoryView),
    (ViewId::Settings, SettingsView),
    (ViewId::ProfileEditor { id: None }, ProfileEditorView),
    (ViewId::ProfileEditor { id: Some(uuid) }, ProfileEditorView),
    (ViewId::McpAdd, McpAddView),
    (ViewId::McpConfigure { id: uuid }, McpConfigureView),
    (ViewId::ModelSelector, ModelSelectorView),
  ] DO
    GIVEN MainPanel with navigation.current() == view_id
    WHEN rendered
    THEN expected_view is rendered
  END FOR
END TEST
```

---

## Navigation Flow Examples

### Settings → ProfileEditor → ModelSelector → Back

```
Initial: [Chat]

1. User clicks Settings button
   → UserEvent::Navigate { to: Settings }
   → Stack: [Chat, Settings]
   → NavigationEvent::Navigated { view: Settings }

2. User clicks + to add profile
   → UserEvent::Navigate { to: ModelSelector }
   → Stack: [Chat, Settings, ModelSelector]
   → NavigationEvent::Navigated { view: ModelSelector }

3. User selects model, clicks Next
   → UserEvent::Navigate { to: ProfileEditor { id: None } }
   → Stack: [Chat, Settings, ModelSelector, ProfileEditor]
   → NavigationEvent::Navigated { view: ProfileEditor }

4. User clicks Cancel
   → UserEvent::NavigateBack
   → Stack: [Chat, Settings, ModelSelector]
   → NavigationEvent::Navigated { view: ModelSelector }

5. User clicks Cancel again
   → UserEvent::NavigateBack
   → Stack: [Chat, Settings]
   → NavigationEvent::Navigated { view: Settings }
```

---

## Error Handling

| Error Condition | Handling Strategy | User Impact |
|-----------------|-------------------|-------------|
| Navigation channel full | Log warning, retry | Brief delay |
| Invalid ViewId | Compiler prevents (exhaustive match) | None |
| Navigate back at root | Return false, no change | None |
| Missing view component | Compiler prevents | None |
