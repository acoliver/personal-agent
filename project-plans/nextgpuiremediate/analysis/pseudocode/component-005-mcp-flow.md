# Pseudocode: MCP Flow

## Plan ID: PLAN-20260219-NEXTGPUIREMEDIATE
## Component: MCP Add and Configure Flow
## Requirements: REQ-WIRE-003.2, REQ-WIRE-003.3, REQ-WIRE-004.2, REQ-WIRE-004.3, REQ-WIRE-005.4, REQ-WIRE-005.5

---

## Overview

MCP (Model Context Protocol) management has two views:
1. **McpAddView** - Search registry and select MCP to add
2. **McpConfigureView** - Configure selected MCP with credentials/settings

Both have presenters with placeholder implementations and missing UserEvent handlers.

## Current State Analysis

### McpAddPresenter Issues
```rust
// Does NOT handle McpAddNext
UserEvent::SearchMcpRegistry { query, source } => { /* placeholder */ }
UserEvent::SelectMcpFromRegistry { source } => { /* placeholder */ }

// on_search_registry is placeholder:
async fn on_search_registry(...) {
    tracing::info!("Searching MCP registry for: {}", query);  // Just logs!
}
```

### McpConfigurePresenter Issues
```rust
// Does NOT handle SaveMcp
UserEvent::SaveMcpConfig { id, config } => { /* placeholder */ }

// on_save_config is placeholder:
async fn on_save_config(...) {
    tracing::info!("Saving MCP config");  // Just logs!
}
```

## Pseudocode: McpAddPresenter

```pseudocode
001: // Update events/types.rs UserEvent enum
002: ENUM UserEvent {
003:   // Existing - need to implement handler
004:   SearchMcpRegistry { query: String, source: McpRegistrySource },
005:   SelectMcpFromRegistry { source: McpRegistrySource },
006:   
007:   // McpAddNext - user clicks Next after selecting MCP from search results
008:   McpAddNext {
009:     registry_entry_id: String,  // ID from registry
010:     name: String,               // Display name
011:     // Basic metadata from registry
012:   },
013: }
014:
015: FUNCTION McpAddPresenter::handle_user_event(
016:   mcp_registry_service,
017:   view_tx,
018:   event: UserEvent
019: )
020:   MATCH event:
021:     UserEvent::SearchMcpRegistry { query, source } =>
022:       self.on_search_registry(mcp_registry_service, view_tx, query, source).await
023:     
024:     UserEvent::SelectMcpFromRegistry { source } =>
025:       self.on_select_from_registry(mcp_registry_service, view_tx, source).await
026:     
027:     // @REQ-WIRE-003.3: Handle McpAddNext
028:     UserEvent::McpAddNext { registry_entry_id, name } =>
029:       self.on_add_next(mcp_registry_service, view_tx, registry_entry_id, name).await
030:     
031:     _ => ()
032:   END MATCH
033: END FUNCTION
034:
035: // @REQ-WIRE-004.2: Implement search
036: FUNCTION McpAddPresenter::on_search_registry(
037:   mcp_registry_service,
038:   view_tx,
039:   query: String,
040:   source: McpRegistrySource
041: )
042:   LOG_INFO "Searching MCP registry: query='{}' source={:?}", query, source
043:   
044:   MATCH mcp_registry_service.search(&query, &source).await:
045:     Ok(results) =>
046:       // Emit search results to view
047:       // Need ViewCommand for MCP search results
048:       view_tx.send(ViewCommand::McpSearchResults {
049:         results: results.into_iter().map(|r| McpRegistryEntry {
050:           id: r.id,
051:           name: r.name,
052:           description: r.description,
053:           category: r.category,
054:         }).collect()
055:       }).await
056:     
057:     Err(e) =>
058:       LOG_ERROR "MCP registry search failed: {:?}", e
059:       view_tx.send(ViewCommand::ShowError {
060:         title: "Search Failed".to_string(),
061:         message: e.to_string(),
062:         severity: ErrorSeverity::Warning,
063:       }).await
064:   END MATCH
065: END FUNCTION
066:
067: // Handle selection from registry (pre-populate configure view)
068: FUNCTION McpAddPresenter::on_select_from_registry(
069:   mcp_registry_service,
070:   view_tx,
071:   source: McpRegistrySource
072: )
073:   // Mark selection in state (if needed)
074:   LOG_INFO "MCP selected from registry: {:?}", source
075:   // View will use this when user clicks Next
076: END FUNCTION
077:
078: // User clicked Next after selecting MCP
079: FUNCTION McpAddPresenter::on_add_next(
080:   mcp_registry_service,
081:   view_tx,
082:   registry_entry_id: String,
083:   name: String
084: )
085:   LOG_INFO "Proceeding to configure MCP: {} ({})", name, registry_entry_id
086:   
087:   // Get full entry details from registry
088:   MATCH mcp_registry_service.get_entry(&registry_entry_id).await:
089:     Ok(entry) =>
090:       // Navigate to McpConfigure with pre-filled data
091:       view_tx.send(ViewCommand::McpConfigurePrefill {
092:         registry_entry_id: entry.id,
093:         name: entry.name,
094:         command: entry.command,
095:         args: entry.args,
096:         env_vars: entry.required_env_vars,
097:       }).await
098:       
099:       view_tx.send(ViewCommand::NavigateTo {
100:         view: ViewId::McpConfigure
101:       }).await
102:     
103:     Err(e) =>
104:       LOG_ERROR "Failed to get MCP entry details: {:?}", e
105:       view_tx.send(ViewCommand::ShowError { ... }).await
106:   END MATCH
107: END FUNCTION
```

## New ViewCommands for MCP

```pseudocode
108: // Add to view_command.rs
109: ViewCommand::McpSearchResults {
110:   results: Vec<McpRegistryEntry>,
111: }
112:
113: ViewCommand::McpConfigurePrefill {
114:   registry_entry_id: String,
115:   name: String,
116:   command: String,
117:   args: Vec<String>,
118:   env_vars: Vec<EnvVarSpec>,  // { name, required, description }
119: }
120:
121: STRUCT McpRegistryEntry {
122:   id: String,
123:   name: String,
124:   description: String,
125:   category: Option<String>,
126: }
127:
128: STRUCT EnvVarSpec {
129:   name: String,
130:   required: bool,
131:   description: String,
132: }
```

## Pseudocode: McpAddView

```pseudocode
133: // McpAddView state
134: STRUCT McpAddState {
135:   search_query: String,
136:   search_results: Vec<McpRegistryEntry>,
137:   selected_entry_id: Option<String>,
138:   is_searching: bool,
139: }
140:
141: FUNCTION McpAddView::handle_command(cmd: ViewCommand, cx: &mut Context<Self>)
142:   MATCH cmd:
143:     ViewCommand::McpSearchResults { results } =>
144:       // @REQ-WIRE-005.4: Handle registry results
145:       self.state.search_results = results
146:       self.state.is_searching = false
147:       cx.notify()
148:     
149:     ViewCommand::ShowError { title, message, .. } =>
150:       self.state.is_searching = false
151:       // Display error in view
152:       cx.notify()
153:     
154:     ViewCommand::NavigateTo { .. } | ViewCommand::NavigateBack =>
155:       // Handled by MainPanel
156:       ()
157:     
158:     _ => ()
159:   END MATCH
160: END FUNCTION
161:
162: FUNCTION McpAddView::on_search_submit(cx: &mut Context<Self>)
163:   IF self.state.search_query.trim().is_empty():
164:     RETURN
165:   END IF
166:   
167:   self.state.is_searching = true
168:   cx.notify()
169:   
170:   self.emit(UserEvent::SearchMcpRegistry {
171:     query: self.state.search_query.clone(),
172:     source: McpRegistrySource { name: "default".to_string() },
173:   })
174: END FUNCTION
175:
176: FUNCTION McpAddView::on_entry_selected(entry_id: String, cx: &mut Context<Self>)
177:   self.state.selected_entry_id = Some(entry_id)
178:   cx.notify()
179: END FUNCTION
180:
181: FUNCTION McpAddView::on_next_clicked(cx: &mut Context<Self>)
182:   IF LET Some(entry_id) = &self.state.selected_entry_id:
183:     // Find entry name from results
184:     LET name = self.state.search_results
185:       .iter()
186:       .find(|e| &e.id == entry_id)
187:       .map(|e| e.name.clone())
188:       .unwrap_or_default()
189:     
190:     self.emit(UserEvent::McpAddNext {
191:       registry_entry_id: entry_id.clone(),
192:       name,
193:     })
194:   END IF
195: END FUNCTION
```

## Pseudocode: McpConfigurePresenter

```pseudocode
196: // Update UserEvent for SaveMcp
197: ENUM UserEvent {
198:   // SaveMcp - user clicks Save in McpConfigureView
199:   SaveMcp {
200:     id: Option<Uuid>,           // None for new, Some for edit
201:     name: String,
202:     command: String,
203:     args: Vec<String>,
204:     env_vars: HashMap<String, String>,
205:     enabled: bool,
206:   },
207: }
208:
209: FUNCTION McpConfigurePresenter::handle_user_event(
210:   mcp_service,
211:   view_tx,
212:   event: UserEvent
213: )
214:   MATCH event:
215:     // @REQ-WIRE-003.2: Handle SaveMcp
216:     UserEvent::SaveMcp { id, name, command, args, env_vars, enabled } =>
217:       self.on_save_mcp(mcp_service, view_tx, id, name, command, args, env_vars, enabled).await
218:     
219:     UserEvent::SaveMcpConfig { id, config } =>
220:       // Legacy event - convert to SaveMcp
221:       self.on_save_config(mcp_service, view_tx, id, config).await
222:     
223:     UserEvent::StartMcpOAuth { id, provider } =>
224:       self.on_start_oauth(mcp_service, view_tx, id, provider).await
225:     
226:     _ => ()
227:   END MATCH
228: END FUNCTION
229:
230: // @REQ-WIRE-004.3: Implement save
231: FUNCTION McpConfigurePresenter::on_save_mcp(
232:   mcp_service,
233:   view_tx,
234:   id: Option<Uuid>,
235:   name: String,
236:   command: String,
237:   args: Vec<String>,
238:   env_vars: HashMap<String, String>,
239:   enabled: bool
240: )
241:   IF LET Some(existing_id) = id:
242:     // UPDATE existing MCP config
243:     LOG_INFO "Updating MCP config: {} ({})", name, existing_id
244:     
245:     MATCH mcp_service.update(existing_id, name.clone(), command, args, env_vars, enabled).await:
246:       Ok(_) =>
247:         view_tx.send(ViewCommand::McpConfigSaved { id: existing_id }).await
248:         view_tx.send(ViewCommand::NavigateTo { view: ViewId::Settings }).await
249:       
250:       Err(e) =>
251:         LOG_ERROR "Failed to update MCP config: {:?}", e
252:         view_tx.send(ViewCommand::ShowError { ... }).await
253:     END MATCH
254:   
255:   ELSE:
256:     // CREATE new MCP config
257:     LOG_INFO "Creating MCP config: {}", name
258:     
259:     MATCH mcp_service.create(name.clone(), command, args, env_vars, enabled).await:
260:       Ok(mcp) =>
261:         view_tx.send(ViewCommand::McpConfigSaved { id: mcp.id }).await
262:         
263:         // Also emit server started if enabled
264:         IF enabled:
265:           view_tx.send(ViewCommand::McpServerStarted {
266:             id: mcp.id,
267:             tool_count: 0,  // Will be updated when server actually starts
268:           }).await
269:         END IF
270:         
271:         view_tx.send(ViewCommand::NavigateTo { view: ViewId::Settings }).await
272:       
273:       Err(e) =>
274:         LOG_ERROR "Failed to create MCP config: {:?}", e
275:         view_tx.send(ViewCommand::ShowError { ... }).await
276:     END MATCH
277:   END IF
278: END FUNCTION
```

## Pseudocode: McpConfigureView

```pseudocode
279: // McpConfigureView state
280: STRUCT McpConfigureState {
281:   id: Option<Uuid>,
282:   registry_entry_id: Option<String>,
283:   name: String,
284:   command: String,
285:   args: Vec<String>,
286:   env_vars: Vec<(String, String, bool)>,  // (name, value, required)
287:   enabled: bool,
288:   is_saving: bool,
289:   validation_errors: Vec<String>,
290: }
291:
292: FUNCTION McpConfigureView::handle_command(cmd: ViewCommand, cx: &mut Context<Self>)
293:   MATCH cmd:
294:     ViewCommand::McpConfigurePrefill { registry_entry_id, name, command, args, env_vars } =>
295:       // Pre-fill from registry entry
296:       self.state.id = None  // New MCP
297:       self.state.registry_entry_id = Some(registry_entry_id)
298:       self.state.name = name
299:       self.state.command = command
300:       self.state.args = args
301:       self.state.env_vars = env_vars.into_iter()
302:         .map(|e| (e.name, String::new(), e.required))
303:         .collect()
304:       self.state.enabled = false  // Default to disabled until configured
305:       cx.notify()
306:     
307:     ViewCommand::McpConfigSaved { id } =>
308:       // @REQ-WIRE-005.5: Handle save result
309:       self.state.is_saving = false
310:       self.state.id = Some(id)
311:       LOG_INFO "MCP config saved: {}", id
312:       // Navigation handled by presenter
313:       cx.notify()
314:     
315:     ViewCommand::ShowError { title, message, .. } =>
316:       self.state.is_saving = false
317:       self.state.validation_errors = vec![format!("{}: {}", title, message)]
318:       cx.notify()
319:     
320:     _ => ()
321:   END MATCH
322: END FUNCTION
323:
324: FUNCTION McpConfigureView::on_save_clicked(cx: &mut Context<Self>)
325:   // Validate
326:   LET errors = self.validate()
327:   IF NOT errors.is_empty():
328:     self.state.validation_errors = errors
329:     cx.notify()
330:     RETURN
331:   END IF
332:   
333:   self.state.is_saving = true
334:   cx.notify()
335:   
336:   // Build env vars map
337:   LET env_map: HashMap<String, String> = self.state.env_vars
338:     .iter()
339:     .filter(|(_, v, _)| !v.is_empty())
340:     .map(|(k, v, _)| (k.clone(), v.clone()))
341:     .collect()
342:   
343:   self.emit(UserEvent::SaveMcp {
344:     id: self.state.id,
345:     name: self.state.name.clone(),
346:     command: self.state.command.clone(),
347:     args: self.state.args.clone(),
348:     env_vars: env_map,
349:     enabled: self.state.enabled,
350:   })
351: END FUNCTION
352:
353: FUNCTION McpConfigureView::validate() -> Vec<String>
354:   LET errors = Vec::new()
355:   
356:   IF self.state.name.trim().is_empty():
357:     errors.push("Name is required")
358:   END IF
359:   
360:   IF self.state.command.trim().is_empty():
361:     errors.push("Command is required")
362:   END IF
363:   
364:   // Check required env vars have values
365:   FOR (name, value, required) IN &self.state.env_vars:
366:     IF *required AND value.is_empty():
367:       errors.push(format!("Environment variable {} is required", name))
368:     END IF
369:   END FOR
370:   
371:   RETURN errors
372: END FUNCTION
```

## McpService Interface Requirements

```pseudocode
373: // Verify McpService has these methods
374: TRAIT McpService {
375:   async fn create(
376:     name: String,
377:     command: String,
378:     args: Vec<String>,
379:     env_vars: HashMap<String, String>,
380:     enabled: bool
381:   ) -> Result<McpConfig, ServiceError>
382:   
383:   async fn update(
384:     id: Uuid,
385:     name: String,
386:     command: String,
387:     args: Vec<String>,
388:     env_vars: HashMap<String, String>,
389:     enabled: bool
390:   ) -> Result<(), ServiceError>
391:   
392:   // ... existing methods
393: }
```

## Files Modified

- `src/events/types.rs` - Update McpAddNext, SaveMcp variants (lines 001-013, 196-207)
- `src/presentation/view_command.rs` - Add McpSearchResults, McpConfigurePrefill (lines 108-132)
- `src/presentation/mcp_add_presenter.rs` - Implement handlers (lines 015-107)
- `src/presentation/mcp_configure_presenter.rs` - Implement handlers (lines 209-278)
- `src/ui_gpui/views/mcp_add_view.rs` - Implement handle_command (lines 133-195)
- `src/ui_gpui/views/mcp_configure_view.rs` - Implement handle_command (lines 279-372)

## Verification Pseudocode

```pseudocode
394: TEST verify_mcp_search_flow():
395:   LET mcp_registry = MockMcpRegistryService::with_entries(vec![
396:     McpEntry { id: "mcp-1", name: "File System", ... },
397:     McpEntry { id: "mcp-2", name: "GitHub", ... },
398:   ])
399:   
400:   // Search
401:   event_bus.publish(AppEvent::User(UserEvent::SearchMcpRegistry {
402:     query: "file".to_string(),
403:     source: McpRegistrySource::default(),
404:   }))
405:   
406:   // Verify results
407:   LET cmd = view_rx.try_recv().unwrap()
408:   ASSERT matches!(cmd, ViewCommand::McpSearchResults { results } if results.len() == 1)
409: END TEST
410:
411: TEST verify_mcp_save_flow():
412:   // Save new MCP
413:   event_bus.publish(AppEvent::User(UserEvent::SaveMcp {
414:     id: None,
415:     name: "Test MCP".to_string(),
416:     command: "npx".to_string(),
417:     args: vec!["-y", "@test/mcp-server"],
418:     env_vars: HashMap::from([("API_KEY".to_string(), "test".to_string())]),
419:     enabled: true,
420:   }))
421:   
422:   // Verify saved
423:   LET cmd = view_rx.try_recv().unwrap()
424:   ASSERT matches!(cmd, ViewCommand::McpConfigSaved { .. })
425: END TEST
```

## Edge Cases

1. **Empty search query**: View validation prevents empty search
2. **No search results**: Show "No results found" in view
3. **Missing required env vars**: View validation catches before emit
4. **Command not found**: Service validation or MCP start failure
5. **OAuth flow**: Complex - may need separate implementation phase
