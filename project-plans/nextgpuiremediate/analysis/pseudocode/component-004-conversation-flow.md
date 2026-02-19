# Pseudocode: Conversation Flow

## Plan ID: PLAN-20260219-NEXTGPUIREMEDIATE
## Component: Conversation Loading and Streaming
## Requirements: REQ-WIRE-005.1, REQ-WIRE-006

---

## Overview

This component handles two critical flows:
1. Loading a conversation from history (REQ-WIRE-006)
2. Processing streaming ViewCommands in ChatView (REQ-WIRE-005.1)

Currently, selecting a conversation only sets it as "active" but doesn't load messages into ChatView. Streaming ViewCommands are not handled by ChatView.

## Current State Analysis

### History Selection Issue
```rust
// history_presenter.rs - handles SelectConversation
UserEvent::SelectConversation { id } => {
    Self::handle_select_conversation(conversation_service, view_tx, id).await;
}

// But handle_select_conversation only sets active, doesn't load messages:
async fn handle_select_conversation(...) {
    conversation_service.set_active(id).await;
    view_tx.send(ViewCommand::ConversationActivated { id }).await;
    // Messages NOT loaded!
}
```

### ChatView Streaming Issue
```rust
// chat_view.rs - handle_command() exists but empty for streaming:
pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
    match command {
        // Only handles a few cases, not streaming
    }
}
```

## Pseudocode: History/Conversation Loading

```pseudocode
001: // New ViewCommand needed for message loading
002: // Add to view_command.rs ViewCommand enum
003: ViewCommand::ConversationLoaded {
004:   id: Uuid,
005:   title: String,
006:   messages: Vec<MessageData>,  // Need to define MessageData
007: }
008:
009: // MessageData for transport
010: STRUCT MessageData {
011:   id: Uuid,
012:   role: MessageRole,
013:   content: String,
014:   thinking: Option<String>,
015:   timestamp: DateTime<Utc>,
016: }
017:
018: // HistoryPresenter or ChatPresenter handles SelectConversation with full loading
019: FUNCTION ChatPresenter::handle_select_conversation(
020:   conversation_service,
021:   view_tx,
022:   conversation_id: Uuid
023: )
024:   // @REQ-WIRE-006.1: Handle SelectConversation
025:   
026:   // Set as active
027:   IF LET Err(e) = conversation_service.set_active(conversation_id).await:
028:     LOG_ERROR "Failed to set active conversation: {:?}", e
029:     view_tx.send(ViewCommand::ShowError {
030:       title: "Error".to_string(),
031:       message: format!("Failed to select conversation: {}", e),
032:       severity: ErrorSeverity::Error,
033:     }).await
034:     RETURN
035:   END IF
036:   
037:   // @REQ-WIRE-006.2: Load messages via service
038:   MATCH conversation_service.get_messages(conversation_id).await:
039:     Ok(messages) =>
040:       // @REQ-WIRE-006.3: Load conversation metadata
041:       MATCH conversation_service.load(conversation_id).await:
042:         Ok(conversation) =>
043:           // Convert messages to transport format
044:           LET message_data: Vec<MessageData> = messages.iter().map(|m| MessageData {
045:             id: m.id,
046:             role: match m.role {
047:               Role::User => MessageRole::User,
048:               Role::Assistant => MessageRole::Assistant,
049:               _ => MessageRole::System,
050:             },
051:             content: m.content.clone(),
052:             thinking: m.thinking.clone(),
053:             timestamp: m.timestamp,
054:           }).collect()
055:           
056:           // Emit ConversationLoaded
057:           view_tx.send(ViewCommand::ConversationLoaded {
058:             id: conversation_id,
059:             title: conversation.title.unwrap_or("Untitled".to_string()),
060:             messages: message_data,
061:           }).await
062:           
063:           // Also emit Activated for UI state
064:           view_tx.send(ViewCommand::ConversationActivated { id: conversation_id }).await
065:         
066:         Err(e) =>
067:           LOG_ERROR "Failed to load conversation: {:?}", e
068:           view_tx.send(ViewCommand::ShowError { ... }).await
069:       END MATCH
070:     
071:     Err(e) =>
072:       LOG_ERROR "Failed to load messages: {:?}", e
073:       view_tx.send(ViewCommand::ShowError { ... }).await
074:   END MATCH
075: END FUNCTION
```

## Pseudocode: ChatView Command Handling

```pseudocode
076: // @REQ-WIRE-005.1: ChatView handles streaming commands
077: FUNCTION ChatView::handle_command(cmd: ViewCommand, cx: &mut Context<Self>)
078:   MATCH cmd:
079:     // ===== Conversation Loading =====
080:     
081:     ViewCommand::ConversationLoaded { id, title, messages } =>
082:       // @REQ-WIRE-006.4: Display loaded messages
083:       self.conversation_id = Some(id)
084:       self.state.conversation_title = title
085:       self.state.messages = messages.into_iter().map(|m| ChatMessage {
086:         role: match m.role {
087:           MessageRole::User => self::MessageRole::User,
088:           MessageRole::Assistant => self::MessageRole::Assistant,
089:           _ => self::MessageRole::User,  // System not displayed
090:         },
091:         content: m.content,
092:         thinking: m.thinking,
093:         model_id: None,
094:         timestamp: Some(m.timestamp.timestamp() as u64),
095:       }).collect()
096:       self.state.streaming = StreamingState::Idle
097:       self.state.input_text.clear()
098:       cx.notify()
099:     
100:     ViewCommand::ConversationActivated { id } =>
101:       self.conversation_id = Some(id)
102:       cx.notify()
103:     
104:     ViewCommand::ConversationCreated { id, profile_id } =>
105:       self.conversation_id = Some(id)
106:       self.state.messages.clear()
107:       self.state.streaming = StreamingState::Idle
108:       self.state.input_text.clear()
109:       cx.notify()
110:     
111:     ViewCommand::ConversationCleared =>
112:       self.conversation_id = None
113:       self.state.messages.clear()
114:       self.state.streaming = StreamingState::Idle
115:       cx.notify()
116:     
117:     // ===== Message Display =====
118:     
119:     ViewCommand::MessageAppended { conversation_id, role, content } =>
120:       IF self.conversation_id == Some(conversation_id):
121:         self.state.messages.push(ChatMessage {
122:           role: match role {
123:             MessageRole::User => self::MessageRole::User,
124:             MessageRole::Assistant => self::MessageRole::Assistant,
125:             _ => self::MessageRole::User,
126:           },
127:           content,
128:           thinking: None,
129:           model_id: None,
130:           timestamp: Some(chrono::Utc::now().timestamp() as u64),
131:         })
132:         cx.notify()
133:       END IF
134:     
135:     // ===== Streaming =====
136:     
137:     ViewCommand::ShowThinking { conversation_id } =>
138:       IF self.conversation_id == Some(conversation_id):
139:         self.state.streaming = StreamingState::Streaming {
140:           content: String::new(),
141:           done: false,
142:         }
143:         self.state.thinking_content = Some(String::new())
144:         cx.notify()
145:       END IF
146:     
147:     ViewCommand::HideThinking { conversation_id } =>
148:       IF self.conversation_id == Some(conversation_id):
149:         self.state.thinking_content = None
150:         cx.notify()
151:       END IF
152:     
153:     ViewCommand::AppendStream { conversation_id, chunk } =>
154:       IF self.conversation_id == Some(conversation_id):
155:         // Append to current streaming content
156:         IF LET StreamingState::Streaming { ref mut content, .. } = self.state.streaming:
157:           content.push_str(&chunk)
158:         ELSE:
159:           // Start streaming if not already
160:           self.state.streaming = StreamingState::Streaming {
161:             content: chunk,
162:             done: false,
163:           }
164:         END IF
165:         cx.notify()
166:       END IF
167:     
168:     ViewCommand::AppendThinking { conversation_id, content } =>
169:       IF self.conversation_id == Some(conversation_id):
170:         IF LET Some(ref mut thinking) = self.state.thinking_content:
171:           thinking.push_str(&content)
172:         ELSE:
173:           self.state.thinking_content = Some(content)
174:         END IF
175:         cx.notify()
176:       END IF
177:     
178:     ViewCommand::FinalizeStream { conversation_id, tokens } =>
179:       IF self.conversation_id == Some(conversation_id):
180:         // Move streaming content to messages
181:         IF LET StreamingState::Streaming { content, .. } = &self.state.streaming:
182:           IF NOT content.is_empty():
183:             LET msg = ChatMessage::assistant(content.clone(), self.state.current_model.clone())
184:               .with_thinking(self.state.thinking_content.clone().unwrap_or_default())
185:             self.state.messages.push(msg)
186:           END IF
187:         END IF
188:         self.state.streaming = StreamingState::Idle
189:         self.state.thinking_content = None
190:         LOG_INFO "Stream finalized with {} tokens", tokens
191:         cx.notify()
192:       END IF
193:     
194:     ViewCommand::StreamCancelled { conversation_id, partial_content } =>
195:       IF self.conversation_id == Some(conversation_id):
196:         // Save partial content as message
197:         IF NOT partial_content.is_empty():
198:           LET msg = ChatMessage::assistant(partial_content, self.state.current_model.clone())
199:           self.state.messages.push(msg)
200:         END IF
201:         self.state.streaming = StreamingState::Idle
202:         self.state.thinking_content = None
203:         cx.notify()
204:       END IF
205:     
206:     ViewCommand::StreamError { conversation_id, error, recoverable } =>
207:       IF self.conversation_id == Some(conversation_id):
208:         self.state.streaming = StreamingState::Error(error)
209:         self.state.thinking_content = None
210:         cx.notify()
211:       END IF
212:     
213:     // ===== Tool Calls =====
214:     
215:     ViewCommand::ShowToolCall { conversation_id, tool_name, status } =>
216:       IF self.conversation_id == Some(conversation_id):
217:         // Could add tool call to a tool_calls Vec in state
218:         LOG_INFO "Tool call: {} - {}", tool_name, status
219:         cx.notify()
220:       END IF
221:     
222:     ViewCommand::UpdateToolCall { conversation_id, tool_name, status, result, duration } =>
223:       IF self.conversation_id == Some(conversation_id):
224:         LOG_INFO "Tool {} completed: {} ({}ms)", tool_name, status, duration.unwrap_or(0)
225:         cx.notify()
226:       END IF
227:     
228:     // ===== Other =====
229:     
230:     ViewCommand::ToggleThinkingVisibility =>
231:       self.state.show_thinking = NOT self.state.show_thinking
232:       cx.notify()
233:     
234:     ViewCommand::ConversationRenamed { id, new_title } =>
235:       IF self.conversation_id == Some(id):
236:         self.state.conversation_title = new_title
237:         cx.notify()
238:       END IF
239:     
240:     ViewCommand::MessageSaved { conversation_id } =>
241:       // Acknowledgment, no action needed
242:       LOG_DEBUG "Message saved for conversation {}", conversation_id
243:     
244:     _ => ()  // Ignore non-chat commands
245:   END MATCH
246: END FUNCTION
```

## ChatView Render Integration

```pseudocode
247: // ChatView render must show streaming content
248: FUNCTION ChatView::render(window, cx)
249:   DIV()
250:     .flex()
251:     .flex_col()
252:     .size_full()
253:     
254:     // Top bar with title
255:     .child(self.render_top_bar(cx))
256:     
257:     // Messages area
258:     .child(
259:       DIV()
260:         .flex_1()
261:         .overflow_y_auto()
262:         .children(self.state.messages.iter().map(|m| self.render_message(m, cx)))
263:         
264:         // Show streaming content if active
265:         .when_some(self.get_streaming_content(), |d, content| {
266:           d.child(self.render_streaming_bubble(content, cx))
267:         })
268:         
269:         // Show thinking if enabled and content exists
270:         .when(self.state.show_thinking && self.state.thinking_content.is_some(), |d| {
271:           d.child(self.render_thinking_bubble(cx))
272:         })
273:         
274:         // Show error if streaming failed
275:         .when_some(self.get_stream_error(), |d, error| {
276:           d.child(self.render_error_bubble(error, cx))
277:         })
278:     )
279:     
280:     // Input area
281:     .child(self.render_input_area(cx))
282: END FUNCTION
283:
284: FUNCTION ChatView::get_streaming_content() -> Option<&String>
285:   MATCH &self.state.streaming:
286:     StreamingState::Streaming { content, .. } IF NOT content.is_empty() =>
287:       Some(content)
288:     _ => None
289:   END MATCH
290: END FUNCTION
291:
292: FUNCTION ChatView::get_stream_error() -> Option<&String>
293:   MATCH &self.state.streaming:
294:     StreamingState::Error(error) => Some(error)
295:     _ => None
296:   END MATCH
297: END FUNCTION
298:
299: FUNCTION ChatView::render_streaming_bubble(content: &String, cx) -> impl IntoElement
300:   AssistantBubble()
301:     .content(content)
302:     .model_id(&self.state.current_model)
303:     .is_streaming(true)
304: END FUNCTION
```

## Files Modified

- `src/presentation/view_command.rs` - Add ConversationLoaded, MessageData (lines 001-016)
- `src/presentation/chat_presenter.rs` - Enhance handle_select_conversation (lines 019-075)
- `src/ui_gpui/views/chat_view.rs` - Full handle_command implementation (lines 076-246)

## Verification Pseudocode

```pseudocode
305: TEST verify_conversation_loading():
306:   // Setup
307:   LET conversation_service = MockConversationService::with_conversation(test_uuid, vec![
308:     Message { role: Role::User, content: "Hello" },
309:     Message { role: Role::Assistant, content: "Hi there!" },
310:   ])
311:   
312:   // Emit SelectConversation
313:   event_bus.publish(AppEvent::User(UserEvent::SelectConversation { id: test_uuid }))
314:   
315:   // Wait for processing
316:   sleep(Duration::from_millis(100)).await
317:   
318:   // Verify ConversationLoaded emitted
319:   LET cmd = view_rx.try_recv().unwrap()
320:   ASSERT matches!(cmd, ViewCommand::ConversationLoaded { id, messages, .. } 
321:     if id == test_uuid && messages.len() == 2)
322: END TEST
323:
324: TEST verify_streaming_display():
325:   // Setup ChatView
326:   LET chat_view = ChatView::new(ChatState::default(), cx)
327:   chat_view.conversation_id = Some(test_uuid)
328:   
329:   // Simulate streaming
330:   chat_view.handle_command(ViewCommand::ShowThinking { conversation_id: test_uuid }, cx)
331:   chat_view.handle_command(ViewCommand::AppendStream { conversation_id: test_uuid, chunk: "Hello".to_string() }, cx)
332:   chat_view.handle_command(ViewCommand::AppendStream { conversation_id: test_uuid, chunk: " World".to_string() }, cx)
333:   
334:   // Verify state
335:   ASSERT matches!(chat_view.state.streaming, StreamingState::Streaming { content, .. } if content == "Hello World")
336:   
337:   // Finalize
338:   chat_view.handle_command(ViewCommand::FinalizeStream { conversation_id: test_uuid, tokens: 2 }, cx)
339:   
340:   // Verify message added
341:   ASSERT chat_view.state.streaming == StreamingState::Idle
342:   ASSERT chat_view.state.messages.len() == 1
343:   ASSERT chat_view.state.messages[0].content == "Hello World"
344: END TEST
```

## Edge Cases

1. **SelectConversation for non-existent conversation**: Service returns error, presenter emits ShowError
2. **Streaming for wrong conversation_id**: Filter by conversation_id, ignore mismatches
3. **Multiple rapid AppendStream**: All chunks accumulated correctly
4. **FinalizeStream with empty content**: Don't add empty message
5. **StreamError after partial content**: Show error, optionally save partial
