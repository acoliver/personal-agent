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
219:     UserEvent::StartMcpOAuth { id, provider } =>
220:       self.on_start_oauth(mcp_service, view_tx, id, provider).await
221:     
222:     _ => ()
223:   END MATCH
224: END FUNCTION
225:
226: // @REQ-WIRE-004.3: Implement save
227: FUNCTION McpConfigurePresenter::on_save_mcp(
228:   mcp_service,
229:   view_tx,
230:   id: Option<Uuid>,
231:   name: String,
232:   command: String,
233:   args: Vec<String>,
234:   env_vars: HashMap<String, String>,
235:   enabled: bool
236: )
237:   IF LET Some(existing_id) = id:
238:     // UPDATE existing MCP config
239:     LOG_INFO "Updating MCP config: {} ({})", name, existing_id
240:     
241:     MATCH mcp_service.update(existing_id, name.clone(), command, args, env_vars, enabled).await:
242:       Ok(_) =>
243:         view_tx.send(ViewCommand::McpConfigSaved { id: existing_id }).await
244:         view_tx.send(ViewCommand::NavigateTo { view: ViewId::Settings }).await
245:       
246:       Err(e) =>
247:         LOG_ERROR "Failed to update MCP config: {:?}", e
248:         view_tx.send(ViewCommand::ShowError { ... }).await
249:     END MATCH
250:   
251:   ELSE:
252:     // CREATE new MCP config
253:     LOG_INFO "Creating MCP config: {}", name
254:     
255:     MATCH mcp_service.create(name.clone(), command, args, env_vars, enabled).await:
256:       Ok(mcp) =>
257:         view_tx.send(ViewCommand::McpConfigSaved { id: mcp.id }).await
258:         
259:         // Also emit server started if enabled
260:         IF enabled:
261:           view_tx.send(ViewCommand::McpServerStarted {
262:             id: mcp.id,
263:             tool_count: 0,  // Will be updated when server actually starts
264:           }).await
265:         END IF
266:         
267:         view_tx.send(ViewCommand::NavigateTo { view: ViewId::Settings }).await
268:       
269:       Err(e) =>
270:         LOG_ERROR "Failed to create MCP config: {:?}", e
271:         view_tx.send(ViewCommand::ShowError { ... }).await
272:     END MATCH
273:   END IF
274: END FUNCTION
```

## Pseudocode: McpConfigureView

```pseudocode
275: // McpConfigureView state
276: STRUCT McpConfigureState {
277:   id: Option<Uuid>,
278:   registry_entry_id: Option<String>,
279:   name: String,
280:   command: String,
281:   args: Vec<String>,
282:   env_vars: Vec<(String, String, bool)>,  // (name, value, required)
283:   enabled: bool,
284:   is_saving: bool,
285:   validation_errors: Vec<String>,
286: }
287:
288: FUNCTION McpConfigureView::handle_command(cmd: ViewCommand, cx: &mut Context<Self>)
289:   MATCH cmd:
290:     ViewCommand::McpConfigurePrefill { registry_entry_id, name, command, args, env_vars } =>
291:       // Pre-fill from registry entry
292:       self.state.id = None  // New MCP
293:       self.state.registry_entry_id = Some(registry_entry_id)
294:       self.state.name = name
295:       self.state.command = command
296:       self.state.args = args
297:       self.state.env_vars = env_vars.into_iter()
298:         .map(|e| (e.name, String::new(), e.required))
299:         .collect()
300:       self.state.enabled = false  // Default to disabled until configured
301:       cx.notify()
302:     
303:     ViewCommand::McpConfigSaved { id } =>
304:       // @REQ-WIRE-005.5: Handle save result
305:       self.state.is_saving = false
306:       self.state.id = Some(id)
307:       LOG_INFO "MCP config saved: {}", id
308:       // Navigation handled by presenter
309:       cx.notify()
310:     
311:     ViewCommand::ShowError { title, message, .. } =>
312:       self.state.is_saving = false
313:       self.state.validation_errors = vec![format!("{}: {}", title, message)]
314:       cx.notify()
315:     
316:     _ => ()
317:   END MATCH
318: END FUNCTION
319:
320: FUNCTION McpConfigureView::on_save_clicked(cx: &mut Context<Self>)
321:   // Validate
322:   LET errors = self.validate()
323:   IF NOT errors.is_empty():
324:     self.state.validation_errors = errors
325:     cx.notify()
326:     RETURN
327:   END IF
328:   
329:   self.state.is_saving = true
330:   cx.notify()
331:   
332:   // Build env vars map
333:   LET env_map: HashMap<String, String> = self.state.env_vars
334:     .iter()
335:     .filter(|(_, v, _)| !v.is_empty())
336:     .map(|(k, v, _)| (k.clone(), v.clone()))
337:     .collect()
338:   
339:   self.emit(UserEvent::SaveMcp {
340:     id: self.state.id,
341:     name: self.state.name.clone(),
342:     command: self.state.command.clone(),
343:     args: self.state.args.clone(),
344:     env_vars: env_map,
345:     enabled: self.state.enabled,
346:   })
347: END FUNCTION
348:
349: FUNCTION McpConfigureView::validate() -> Vec<String>
350:   LET errors = Vec::new()
351:   
352:   IF self.state.name.trim().is_empty():
353:     errors.push("Name is required")
354:   END IF
355:   
356:   IF self.state.command.trim().is_empty():
357:     errors.push("Command is required")
358:   END IF
359:   
360:   // Check required env vars have values
361:   FOR (name, value, required) IN &self.state.env_vars:
362:     IF *required AND value.is_empty():
363:       errors.push(format!("Environment variable {} is required", name))
364:     END IF
365:   END FOR
366:   
367:   RETURN errors
368: END FUNCTION
```

## McpService Interface Requirements

```pseudocode
369: // Verify McpService has these methods
370: TRAIT McpService {
371:   async fn create(
372:     name: String,
373:     command: String,
374:     args: Vec<String>,
375:     env_vars: HashMap<String, String>,
376:     enabled: bool
377:   ) -> Result<McpConfig, ServiceError>
378:   
379:   async fn update(
380:     id: Uuid,
381:     name: String,
382:     command: String,
383:     args: Vec<String>,
384:     env_vars: HashMap<String, String>,
385:     enabled: bool
386:   ) -> Result<(), ServiceError>
387:   
388:   // ... existing methods
389: }
```

## Files Modified

- `src/events/types.rs` - Update McpAddNext, SaveMcp variants (lines 001-013, 196-207)
- `src/presentation/view_command.rs` - Add McpSearchResults, McpConfigurePrefill (lines 108-132)
- `src/presentation/mcp_add_presenter.rs` - Implement handlers (lines 015-107)
- `src/presentation/mcp_configure_presenter.rs` - Implement handlers (lines 209-274)
- `src/ui_gpui/views/mcp_add_view.rs` - Implement handle_command (lines 133-195)
- `src/ui_gpui/views/mcp_configure_view.rs` - Implement handle_command (lines 275-368)

## Verification Pseudocode

```pseudocode
390: TEST verify_mcp_search_flow():
391:   LET mcp_registry = MockMcpRegistryService::with_entries(vec![
392:     McpEntry { id: "mcp-1", name: "File System", ... },
393:     McpEntry { id: "mcp-2", name: "GitHub", ... },
394:   ])
395:   
396:   // Search
397:   event_bus.publish(AppEvent::User(UserEvent::SearchMcpRegistry {
398:     query: "file".to_string(),
399:     source: McpRegistrySource::default(),
400:   }))
401:   
402:   // Verify results
403:   LET cmd = view_rx.try_recv().unwrap()
404:   ASSERT matches!(cmd, ViewCommand::McpSearchResults { results } if results.len() == 1)
405: END TEST
406:
407: TEST verify_mcp_save_flow():
408:   // Save new MCP
409:   event_bus.publish(AppEvent::User(UserEvent::SaveMcp {
410:     id: None,
411:     name: "Test MCP".to_string(),
412:     command: "npx".to_string(),
413:     args: vec!["-y", "@test/mcp-server"],
414:     env_vars: HashMap::from([("API_KEY".to_string(), "test".to_string())]),
415:     enabled: true,
416:   }))
417:   
418:   // Verify saved
419:   LET cmd = view_rx.try_recv().unwrap()
420:   ASSERT matches!(cmd, ViewCommand::McpConfigSaved { .. })
421: END TEST
```

## Edge Cases

1. **Empty search query**: View validation prevents empty search
2. **No search results**: Show "No results found" in view
3. **Missing required env vars**: View validation catches before emit
4. **Command not found**: Service validation or MCP start failure
5. **OAuth flow**: Complex - may need separate implementation phase
