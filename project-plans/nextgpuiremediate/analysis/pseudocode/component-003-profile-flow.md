# Component 003: Profile Flow

Plan ID: PLAN-20260219-NEXTGPUIREMEDIATE
Component: Profile Flow
Created: 2026-02-19

---

## Overview

This component defines the complete profile management flow including listing profiles, selecting a default, creating new profiles (via ModelSelector → ProfileEditor), editing existing profiles, and deleting profiles. The flow spans SettingsView, SettingsPresenter, ModelSelectorView, ProfileEditorView, and ProfileEditorPresenter.

---

## Requirement Coverage

### REQ-PRF-001: Profile List Display

**REQ-PRF-001.1**: SettingsView MUST display all profiles from ProfileService

- **Full Text**: When the Settings view loads, it must fetch and display all saved profiles. Each profile appears as a row in the profiles list.
- **GIVEN**: ProfileService has 3 profiles saved
- **WHEN**: User navigates to SettingsView
- **THEN**: All 3 profiles are displayed in the profiles list
- **Why**: Users need to see all their configured profiles to manage them

**REQ-PRF-001.2**: Default profile MUST be visually highlighted

- **Full Text**: The currently selected default profile must have a distinct visual indicator (highlight) so users know which profile is active.
- **GIVEN**: Profile "Claude" is set as default in AppSettingsService
- **WHEN**: SettingsView renders
- **THEN**: "Claude" row has highlighted/selected styling
- **Why**: Users need clear feedback about which profile is currently in use

**REQ-PRF-001.3**: Profile format: "{name} ({provider}:{model})"

- **Full Text**: Each profile row displays the profile name followed by the provider and model ID in parentheses.
- **GIVEN**: Profile with name="My Claude", provider="anthropic", model="claude-3-5-sonnet"
- **WHEN**: Rendered in SettingsView
- **THEN**: Displays "My Claude (anthropic:claude-3-5-sonnet)"
- **Why**: Users can identify which model each profile uses at a glance

### REQ-PRF-002: Profile Selection

**REQ-PRF-002.1**: Clicking profile row MUST emit UserEvent::SelectProfile

- **Full Text**: When a user clicks on a profile row (not a button within it), a SelectProfile event must be emitted with the profile's ID.
- **GIVEN**: User clicks on "My Claude" profile row
- **WHEN**: Click event fires
- **THEN**: UserEvent::SelectProfile { id: profile_uuid } is emitted
- **Why**: Decouples view from business logic

**REQ-PRF-002.2**: SettingsPresenter MUST call AppSettingsService.set_default_profile_id()

- **Full Text**: When the presenter receives SelectProfile event, it calls the app settings service to persist the new default.
- **GIVEN**: SettingsPresenter receives SelectProfile { id: uuid }
- **WHEN**: Event is handled
- **THEN**: AppSettingsService.set_default_profile_id(uuid) is called
- **Why**: Persists the selection so it survives app restart

**REQ-PRF-002.3**: ProfileEvent::DefaultChanged MUST update view highlight

- **Full Text**: After the default changes, a ProfileEvent::DefaultChanged is emitted, which causes the view to update its highlight.
- **GIVEN**: Profile "Claude" selected as default
- **WHEN**: AppSettingsService emits ProfileEvent::DefaultChanged
- **THEN**: SettingsView updates highlight to "Claude" row
- **Why**: Confirms the selection was successful

### REQ-PRF-003: Profile CRUD

**REQ-PRF-003.1**: Add profile MUST navigate to ModelSelector then ProfileEditor

- **Full Text**: The "add profile" flow requires first selecting a model (ModelSelector), then configuring the profile (ProfileEditor).
- **GIVEN**: User clicks [+] button in profiles section
- **WHEN**: Click event fires
- **THEN**: Navigate to ModelSelector, after model selection navigate to ProfileEditor with id=None
- **Why**: Model selection provides provider/model/base_url pre-filling

**REQ-PRF-003.2**: Edit profile MUST navigate to ProfileEditor with profile ID

- **Full Text**: Editing an existing profile navigates directly to ProfileEditor with the profile's ID for pre-population.
- **GIVEN**: User clicks [Edit] with profile "Claude" selected
- **WHEN**: Click event fires
- **THEN**: Navigate to ProfileEditor { id: Some(claude_uuid) }
- **Why**: ProfileEditor needs to load existing data

**REQ-PRF-003.3**: Delete profile MUST show confirmation then call ProfileService.delete()

- **Full Text**: Deleting a profile requires confirmation dialog before actually deleting to prevent accidents.
- **GIVEN**: User clicks [-] with profile selected
- **WHEN**: Confirmation dialog confirms
- **THEN**: ProfileService.delete(id) is called
- **Why**: Prevents accidental deletion of important profiles

**REQ-PRF-003.4**: ProfileEvent::Created/Updated/Deleted MUST refresh profile list

- **Full Text**: After any CRUD operation, the profile list must refresh to reflect the changes.
- **GIVEN**: ProfileService.create() succeeds
- **WHEN**: ProfileEvent::Created is emitted
- **THEN**: SettingsView refreshes profile list to show new profile
- **Why**: UI stays in sync with persisted state

### REQ-PRF-004: Profile Editor

**REQ-PRF-004.1**: ProfileEditorView MUST populate fields from profile data

- **Full Text**: When editing an existing profile, all fields must be pre-populated with the profile's current values.
- **GIVEN**: ProfileEditorView opened with id=Some(uuid)
- **WHEN**: View renders
- **THEN**: Name, model, API key, parameters all populated from profile
- **Why**: Users can see and modify existing values

**REQ-PRF-004.2**: Save MUST validate and call ProfileService.create() or .update()

- **Full Text**: Save button validates all required fields, then calls create() for new profiles or update() for existing.
- **GIVEN**: ProfileEditorView with valid data
- **WHEN**: User clicks Save
- **THEN**: ProfileService.create(profile) or .update(id, profile) is called
- **Why**: Persists the profile configuration

**REQ-PRF-004.3**: Auth method change MUST show/hide appropriate auth fields

- **Full Text**: Selecting different auth methods shows the relevant fields (API Key field for ApiKey, keyfile path for KeyFile, nothing for None).
- **GIVEN**: Auth method dropdown changed to "Key File"
- **WHEN**: View re-renders
- **THEN**: API key field hidden, keyfile path field shown
- **Why**: Clean UX - only show relevant fields

---

## Pseudocode

### SettingsPresenter - Profile Handling

```pseudocode
001: MODULE SettingsPresenter
002: 
003: STRUCT SettingsPresenter
004:   event_bus: Arc<EventBus>
005:   view_command_sink: Arc<ViewCommandSink>
006:   profile_service: Arc<dyn ProfileService>
007:   app_settings: Arc<dyn AppSettingsService>
008:   selected_profile_id: RwLock<Option<Uuid>>
009: END STRUCT
010: 
011: // REQ-PRF-001.1: Load and display profiles
012: ASYNC FUNCTION load_profiles(self)
013:   tracing::debug!("SettingsPresenter: loading profiles")
014:   
015:   // Fetch all profiles from service
016:   LET profiles = MATCH self.profile_service.list().await
017:     Ok(profiles) => profiles,
018:     Err(e) => {
019:       tracing::error!("Failed to load profiles: {}", e)
020:       self.send_error("Failed to load profiles")
021:       RETURN
022:     }
023:   END MATCH
024:   
025:   // Get default profile ID
026:   LET default_id = self.app_settings.get_default_profile_id()
027:     .ok()
028:     .flatten()
029:   
030:   // REQ-PRF-001.3: Format profile items
031:   LET profile_items: Vec<ProfileItem> = profiles
032:     .iter()
033:     .map(|p| ProfileItem {
034:       id: p.id,
035:       display: format!("{} ({}:{})", p.name, p.provider_id, p.model_id),
036:       is_default: Some(p.id) == default_id,
037:     })
038:     .collect()
039:   
040:   // Send to view
041:   // REQ-PRF-001.2: Default profile marked
042:   self.view_command_sink.send(ViewCommand::SetProfiles {
043:     profiles: profile_items,
044:   })
045:   
046:   // REQ-PRF-001.2: Also set default separately for highlight
047:   self.view_command_sink.send(ViewCommand::SetDefaultProfile {
048:     id: default_id,
049:   })
050: END FUNCTION
051: 
052: // REQ-PRF-002.1, REQ-PRF-002.2: Handle profile selection
053: ASYNC FUNCTION handle_select_profile(self, profile_id: Uuid)
054:   tracing::debug!("Handling SelectProfile: {}", profile_id)
055:   
056:   // REQ-PRF-002.2: Call app settings to set default
057:   MATCH self.app_settings.set_default_profile_id(profile_id)
058:     Ok(()) => {
059:       tracing::info!("Default profile set to {}", profile_id)
060:       // ProfileEvent::DefaultChanged will be emitted by AppSettingsService
061:     }
062:     Err(e) => {
063:       tracing::error!("Failed to set default profile: {}", e)
064:       self.send_error("Failed to set default profile")
065:     }
066:   END MATCH
067: END FUNCTION
068: 
069: // REQ-PRF-002.3: Handle default changed event
070: FUNCTION handle_profile_default_changed(self, profile_id: Option<Uuid>)
071:   tracing::debug!("Profile default changed to {:?}", profile_id)
072:   
073:   // Update view highlight
074:   self.view_command_sink.send(ViewCommand::SetDefaultProfile {
075:     id: profile_id,
076:   })
077:   
078:   // Update local tracking
079:   *self.selected_profile_id.write() = profile_id
080: END FUNCTION
081: 
082: // REQ-PRF-003.3: Handle delete profile request
083: ASYNC FUNCTION handle_delete_profile(self, profile_id: Uuid)
084:   tracing::debug!("Handling DeleteProfile request: {}", profile_id)
085:   
086:   // Show confirmation dialog via view command
087:   self.view_command_sink.send(ViewCommand::ShowDeleteProfileConfirmation {
088:     id: profile_id,
089:   })
090: END FUNCTION
091: 
092: // REQ-PRF-003.3: Handle confirmed delete
093: ASYNC FUNCTION handle_confirm_delete_profile(self, profile_id: Uuid)
094:   tracing::info!("Deleting profile: {}", profile_id)
095:   
096:   // Check if this is the default profile
097:   LET current_default = self.app_settings.get_default_profile_id()
098:     .ok()
099:     .flatten()
100:   
101:   MATCH self.profile_service.delete(profile_id).await
102:     Ok(()) => {
103:       // If deleted profile was default, clear default
104:       IF current_default == Some(profile_id) THEN
105:         LET _ = self.app_settings.clear_default_profile()
106:       END IF
107:       tracing::info!("Profile {} deleted", profile_id)
108:       // ProfileEvent::Deleted will trigger list refresh
109:     }
110:     Err(e) => {
111:       tracing::error!("Failed to delete profile: {}", e)
112:       self.send_error("Failed to delete profile")
113:     }
114:   END MATCH
115: END FUNCTION
116: 
117: // REQ-PRF-003.4: Handle profile events that require list refresh
118: ASYNC FUNCTION handle_profile_event(self, event: ProfileEvent)
119:   MATCH event
120:     ProfileEvent::Created { id, name } => {
121:       tracing::info!("Profile created: {} ({})", name, id)
122:       self.load_profiles().await
123:     }
124:     ProfileEvent::Updated { id, name } => {
125:       tracing::info!("Profile updated: {} ({})", name, id)
126:       self.load_profiles().await
127:     }
128:     ProfileEvent::Deleted { id, name } => {
129:       tracing::info!("Profile deleted: {} ({})", name, id)
130:       self.load_profiles().await
131:     }
132:     ProfileEvent::DefaultChanged { profile_id } => {
133:       self.handle_profile_default_changed(profile_id)
134:     }
135:     _ => {}  // Ignore other profile events
136:   END MATCH
137: END FUNCTION
138: 
139: // Main event handler
140: ASYNC FUNCTION handle_event(self, event: AppEvent)
141:   MATCH event
142:     // User events
143:     AppEvent::User(UserEvent::SelectProfile { id }) => {
144:       self.handle_select_profile(id).await
145:     }
146:     AppEvent::User(UserEvent::DeleteProfile { id }) => {
147:       self.handle_delete_profile(id).await
148:     }
149:     AppEvent::User(UserEvent::ConfirmDeleteProfile { id }) => {
150:       self.handle_confirm_delete_profile(id).await
151:     }
152:     
153:     // Profile events
154:     AppEvent::Profile(profile_event) => {
155:       self.handle_profile_event(profile_event).await
156:     }
157:     
158:     // Navigation - load data when Settings becomes active
159:     AppEvent::Navigation(NavigationEvent::Navigated { view: ViewId::Settings }) => {
160:       self.load_profiles().await
161:       // Also load MCPs (handled in MCP flow component)
162:     }
163:     
164:     _ => {}  // Ignore unrelated events
165:   END MATCH
166: END FUNCTION
167: 
168: END MODULE
```

### ProfileEditorPresenter

```pseudocode
169: MODULE ProfileEditorPresenter
170: 
171: STRUCT ProfileEditorPresenter
172:   event_bus: Arc<EventBus>
173:   view_command_sink: Arc<ViewCommandSink>
174:   profile_service: Arc<dyn ProfileService>
175:   app_settings: Arc<dyn AppSettingsService>
176:   nav_channel: Arc<NavigationChannel>
177:   current_profile_id: RwLock<Option<Uuid>>  // None = new, Some = edit
178: END STRUCT
179: 
180: // REQ-PRF-004.1: Load profile for editing
181: ASYNC FUNCTION load_profile(self, profile_id: Uuid)
182:   tracing::debug!("Loading profile for editing: {}", profile_id)
183:   
184:   MATCH self.profile_service.get(profile_id).await
185:     Ok(profile) => {
186:       // Convert to editor data
187:       LET editor_data = ProfileEditorData {
188:         name: profile.name.clone(),
189:         model_id: profile.model_id.clone(),
190:         provider_id: profile.provider_id.clone(),
191:         base_url: profile.base_url.clone(),
192:         auth_method: derive_auth_method(&profile),
193:         api_key: profile.api_key.clone(),
194:         keyfile_path: profile.keyfile_path.clone(),
195:         temperature: profile.parameters.temperature,
196:         max_tokens: profile.parameters.max_tokens,
197:         context_limit: profile.parameters.context_limit,
198:         enable_thinking: profile.parameters.enable_thinking,
199:         thinking_budget: profile.parameters.thinking_budget,
200:         show_thinking: profile.parameters.show_thinking,
201:         system_prompt: profile.system_prompt.clone(),
202:       }
203:       
204:       // REQ-PRF-004.1: Populate view fields
205:       self.view_command_sink.send(ViewCommand::SetProfileEditorData {
206:         data: editor_data,
207:         is_new: false,
208:       })
209:       
210:       *self.current_profile_id.write() = Some(profile_id)
211:     }
212:     Err(e) => {
213:       tracing::error!("Failed to load profile: {}", e)
214:       self.send_error("Profile not found")
215:       // Navigate back
216:       self.nav_channel.send_navigation(NavigationCommand::NavigateBack)
217:     }
218:   END MATCH
219: END FUNCTION
220: 
221: // Initialize for new profile (from ModelSelector)
222: FUNCTION init_new_profile(self, selected_model: SelectedModel)
223:   tracing::debug!("Initializing new profile with model: {}", selected_model.model_id)
224:   
225:   // Pre-populate from model selection
226:   LET editor_data = ProfileEditorData {
227:     name: selected_model.model_id.clone(),  // Default name to model
228:     model_id: selected_model.model_id,
229:     provider_id: selected_model.provider_id,
230:     base_url: Some(selected_model.base_url),
231:     auth_method: AuthMethod::ApiKey,  // Default
232:     api_key: None,
233:     keyfile_path: None,
234:     temperature: Some(1.0),
235:     max_tokens: Some(4096),
236:     context_limit: selected_model.context_limit,
237:     enable_thinking: false,
238:     thinking_budget: None,
239:     show_thinking: true,
240:     system_prompt: "You are a helpful assistant.".to_string(),
241:   }
242:   
243:   self.view_command_sink.send(ViewCommand::SetProfileEditorData {
244:     data: editor_data,
245:     is_new: true,
246:   })
247:   
248:   *self.current_profile_id.write() = None
249: END FUNCTION
250: 
251: // REQ-PRF-004.2: Handle save profile
252: ASYNC FUNCTION handle_save_profile(self, data: ProfileEditorData)
253:   tracing::debug!("Saving profile: {}", data.name)
254:   
255:   // Validate required fields
256:   IF data.name.trim().is_empty() THEN
257:     self.view_command_sink.send(ViewCommand::ShowFieldError {
258:       field: "name",
259:       error: "Name is required",
260:     })
261:     RETURN
262:   END IF
263:   
264:   // Validate auth based on method
265:   IF data.auth_method == AuthMethod::ApiKey && data.api_key.is_none() THEN
266:     self.view_command_sink.send(ViewCommand::ShowFieldError {
267:       field: "api_key",
268:       error: "API key is required",
269:     })
270:     RETURN
271:   END IF
272:   
273:   IF data.auth_method == AuthMethod::KeyFile && data.keyfile_path.is_none() THEN
274:     self.view_command_sink.send(ViewCommand::ShowFieldError {
275:       field: "keyfile",
276:       error: "Key file path is required",
277:     })
278:     RETURN
279:   END IF
280:   
281:   // Build profile
282:   LET profile = ModelProfile {
283:     id: self.current_profile_id.read().unwrap_or_else(Uuid::new_v4),
284:     name: data.name.trim().to_string(),
285:     provider_id: data.provider_id,
286:     model_id: data.model_id,
287:     base_url: data.base_url,
288:     api_key: sanitize_api_key(data.api_key),
289:     keyfile_path: data.keyfile_path,
290:     system_prompt: data.system_prompt,
291:     parameters: ModelParameters {
292:       temperature: data.temperature,
293:       max_tokens: data.max_tokens,
294:       context_limit: data.context_limit,
295:       top_p: None,
296:       enable_thinking: data.enable_thinking,
297:       thinking_budget: data.thinking_budget,
298:       show_thinking: data.show_thinking,
299:     },
300:   }
301:   
302:   // REQ-PRF-004.2: Create or update
303:   LET profile_id = *self.current_profile_id.read()
304:   LET result = IF profile_id.is_none() THEN
305:     self.profile_service.create(profile).await
306:   ELSE
307:     self.profile_service.update(profile_id.unwrap(), profile).await
308:   END IF
309:   
310:   MATCH result
311:     Ok(saved_profile) => {
312:       tracing::info!("Profile saved: {}", saved_profile.id)
313:       // Navigate back to Settings
314:       self.nav_channel.send_navigation(NavigationCommand::NavigateBack)
315:       // Optionally set as default if first profile
316:     }
317:     Err(e) => {
318:       tracing::error!("Failed to save profile: {}", e)
319:       self.send_error(&format!("Failed to save: {}", e))
320:     }
321:   END MATCH
322: END FUNCTION
323: 
324: // Handle navigation to ProfileEditor
325: ASYNC FUNCTION handle_navigation(self, view: ViewId)
326:   MATCH view
327:     ViewId::ProfileEditor { id: None } => {
328:       // New profile - wait for model selection data
329:       tracing::debug!("ProfileEditor opened for new profile")
330:     }
331:     ViewId::ProfileEditor { id: Some(profile_id) } => {
332:       // Edit existing profile
333:       self.load_profile(profile_id).await
334:     }
335:     _ => {}  // Ignore other navigation
336:   END MATCH
337: END FUNCTION
338: 
339: // Main event handler
340: ASYNC FUNCTION handle_event(self, event: AppEvent)
341:   MATCH event
342:     AppEvent::User(UserEvent::SaveProfile { profile }) => {
343:       // Convert from event type to editor data and save
344:       self.handle_save_profile(profile.into()).await
345:     }
346:     AppEvent::User(UserEvent::SaveProfileEditor) => {
347:       // Save is triggered, get current data from view state
348:       // (View sends full data in SaveProfile event)
349:     }
350:     AppEvent::Navigation(NavigationEvent::Navigated { view }) => {
351:       self.handle_navigation(view).await
352:     }
353:     _ => {}
354:   END MATCH
355: END FUNCTION
356: 
357: // REQ-PRF-004.3: Helper to derive auth method from profile
358: FUNCTION derive_auth_method(profile: &ModelProfile) -> AuthMethod
359:   IF profile.keyfile_path.is_some() THEN
360:     RETURN AuthMethod::KeyFile
361:   ELSE IF profile.api_key.is_some() THEN
362:     RETURN AuthMethod::ApiKey
363:   ELSE
364:     RETURN AuthMethod::None
365:   END IF
366: END FUNCTION
367: 
368: // Sanitize API key (trim, remove newlines)
369: FUNCTION sanitize_api_key(key: Option<String>) -> Option<String>
370:   key.map(|k| {
371:     k.trim()
372:       .replace("\n", "")
373:       .replace("\r", "")
374:       .to_string()
375:   })
376: END FUNCTION
377: 
378: END MODULE
```

### SettingsView - Profile Section

```pseudocode
379: MODULE SettingsViewProfiles
380: 
381: // REQ-PRF-001.1, REQ-PRF-001.2, REQ-PRF-001.3: Render profiles list
382: FUNCTION render_profiles_section(
383:   profiles: &[ProfileItem],
384:   selected_id: Option<Uuid>,
385:   emitter: Arc<dyn Fn(UserEvent)>
386: ) -> impl IntoElement
387:   
388:   // Section header
389:   LET header = div()
390:     .child(Label::new("PROFILES").text_xs().text_color(MUTED))
391:   
392:   // Profile rows
393:   LET rows = profiles.iter().map(|profile| {
394:     // REQ-PRF-001.2: Highlight default
395:     LET is_selected = selected_id == Some(profile.id)
396:     LET bg_color = IF is_selected THEN ACCENT_BLUE ELSE TRANSPARENT
397:     
398:     LET profile_id = profile.id
399:     LET emitter_clone = emitter.clone()
400:     
401:     // REQ-PRF-001.3: Display format
402:     div()
403:       .w_full()
404:       .h(px(24.0))
405:       .bg(bg_color)
406:       .px_2()
407:       .child(Label::new(&profile.display))
408:       .on_click(move |_, _| {
409:         // REQ-PRF-002.1: Emit select event
410:         emitter_clone(UserEvent::SelectProfile { id: profile_id })
411:       })
412:   }).collect::<Vec<_>>()
413:   
414:   // List container
415:   LET list = div()
416:     .w(px(360.0))
417:     .h(px(100.0))
418:     .bg(BG_DARKER)
419:     .border_1()
420:     .border_color(BORDER)
421:     .rounded_sm()
422:     .overflow_y_auto()
423:     .children(rows)
424:   
425:   // Toolbar
426:   LET toolbar = render_profile_toolbar(selected_id, emitter)
427:   
428:   RETURN div()
429:     .flex_col()
430:     .gap_1()
431:     .child(header)
432:     .child(list)
433:     .child(toolbar)
434: END FUNCTION
435: 
436: // REQ-PRF-003.1, REQ-PRF-003.2, REQ-PRF-003.3: Profile toolbar
437: FUNCTION render_profile_toolbar(
438:   selected_id: Option<Uuid>,
439:   emitter: Arc<dyn Fn(UserEvent)>
440: ) -> impl IntoElement
441:   
442:   LET has_selection = selected_id.is_some()
443:   
444:   // Delete button
445:   LET delete_btn = Button::new("delete", "-")
446:     .disabled(!has_selection)
447:     .on_click({
448:       LET id = selected_id
449:       LET emitter = emitter.clone()
450:       move |_, _| {
451:         IF let Some(profile_id) = id THEN
452:           // REQ-PRF-003.3: Emit delete request
453:           emitter(UserEvent::DeleteProfile { id: profile_id })
454:         END IF
455:       }
456:     })
457:   
458:   // Add button
459:   LET add_btn = Button::new("add", "+")
460:     .on_click({
461:       LET emitter = emitter.clone()
462:       move |_, _| {
463:         // REQ-PRF-003.1: Navigate to ModelSelector first
464:         emitter(UserEvent::Navigate { to: ViewId::ModelSelector })
465:       }
466:     })
467:   
468:   // Edit button
469:   LET edit_btn = Button::new("edit", "Edit")
470:     .disabled(!has_selection)
471:     .on_click({
472:       LET id = selected_id
473:       LET emitter = emitter.clone()
474:       move |_, _| {
475:         IF let Some(profile_id) = id THEN
476:           // REQ-PRF-003.2: Navigate to ProfileEditor with ID
477:           emitter(UserEvent::Navigate {
478:             to: ViewId::ProfileEditor { id: Some(profile_id) }
479:           })
480:         END IF
481:       }
482:     })
483:   
484:   RETURN div()
485:     .flex()
486:     .gap_2()
487:     .child(delete_btn)
488:     .child(add_btn)
489:     .child(Spacer::new())
490:     .child(edit_btn)
491: END FUNCTION
492: 
493: END MODULE
```

### ProfileEditorView - Auth Method Handling

```pseudocode
494: MODULE ProfileEditorViewAuth
495: 
496: // REQ-PRF-004.3: Render auth method dropdown and conditional fields
497: FUNCTION render_auth_section(
498:   auth_method: AuthMethod,
499:   api_key: Option<String>,
500:   keyfile_path: Option<String>,
501:   mask_enabled: bool,
502:   emitter: Arc<dyn Fn(ProfileEditorAction)>
503: ) -> impl IntoElement
504:   
505:   // Auth method dropdown
506:   LET dropdown = render_auth_method_dropdown(auth_method, emitter.clone())
507:   
508:   // REQ-PRF-004.3: Conditional auth fields
509:   LET auth_fields = MATCH auth_method
510:     AuthMethod::None => {
511:       // No fields needed
512:       div().child(Label::new("No authentication required"))
513:     }
514:     AuthMethod::ApiKey => {
515:       render_api_key_field(api_key, mask_enabled, emitter.clone())
516:     }
517:     AuthMethod::KeyFile => {
518:       render_keyfile_field(keyfile_path, emitter.clone())
519:     }
520:   END MATCH
521:   
522:   RETURN div()
523:     .flex_col()
524:     .gap_3()
525:     .child(div()
526:       .child(Label::new("AUTH METHOD").text_xs().text_color(MUTED))
527:       .child(dropdown)
528:     )
529:     .child(auth_fields)
530: END FUNCTION
531: 
532: FUNCTION render_auth_method_dropdown(
533:   current: AuthMethod,
534:   emitter: Arc<dyn Fn(ProfileEditorAction)>
535: ) -> impl IntoElement
536:   
537:   LET options = vec![
538:     ("None", AuthMethod::None),
539:     ("API Key", AuthMethod::ApiKey),
540:     ("Key File", AuthMethod::KeyFile),
541:   ]
542:   
543:   // Dropdown implementation
544:   Dropdown::new(options)
545:     .selected(current)
546:     .on_change(move |new_method| {
547:       emitter(ProfileEditorAction::SetAuthMethod(new_method))
548:     })
549: END FUNCTION
550: 
551: FUNCTION render_api_key_field(
552:   api_key: Option<String>,
553:   mask_enabled: bool,
554:   emitter: Arc<dyn Fn(ProfileEditorAction)>
555: ) -> impl IntoElement
556:   
557:   LET label_row = div()
558:     .flex()
559:     .justify_between()
560:     .child(Label::new("API KEY").text_xs().text_color(MUTED))
561:     .child(Checkbox::new("mask", mask_enabled)
562:       .label("Mask")
563:       .on_toggle(move |enabled| {
564:         emitter(ProfileEditorAction::SetMaskEnabled(enabled))
565:       })
566:     )
567:   
568:   LET field = TextInput::new("api_key")
569:     .value(api_key.unwrap_or_default())
570:     .placeholder("sk-...")
571:     .secure(mask_enabled)  // Mask if enabled
572:     .on_change(move |text| {
573:       // Sanitize on every change
574:       LET clean = text.replace("\n", "").replace("\r", "")
575:       emitter(ProfileEditorAction::SetApiKey(Some(clean)))
576:     })
577:   
578:   RETURN div()
579:     .flex_col()
580:     .gap_1()
581:     .child(label_row)
582:     .child(field)
583: END FUNCTION
584: 
585: FUNCTION render_keyfile_field(
586:   keyfile_path: Option<String>,
587:   emitter: Arc<dyn Fn(ProfileEditorAction)>
588: ) -> impl IntoElement
589:   
590:   LET field = TextInput::new("keyfile")
591:     .value(keyfile_path.unwrap_or_default())
592:     .placeholder("/path/to/api_key")
593:     .on_change(move |text| {
594:       LET clean = text.replace("\n", "").replace("\r", "")
595:       emitter(ProfileEditorAction::SetKeyfilePath(Some(clean)))
596:     })
597:   
598:   LET browse_btn = Button::new("browse", "Browse")
599:     .on_click(move |_, _| {
600:       emitter(ProfileEditorAction::BrowseKeyfile)
601:     })
602:   
603:   RETURN div()
604:     .flex_col()
605:     .gap_1()
606:     .child(Label::new("KEY FILE").text_xs().text_color(MUTED))
607:     .child(div()
608:       .flex()
609:       .gap_2()
610:       .child(field.flex_1())
611:       .child(browse_btn)
612:     )
613: END FUNCTION
614: 
615: END MODULE
```

---

## Test Scenarios

### Test: Load Profiles on Settings Navigation

```pseudocode
TEST load_profiles_on_settings_navigation
  // REQ-PRF-001.1
  GIVEN ProfileService with profiles [A, B, C]
  AND SettingsPresenter subscribed
  
  WHEN NavigationEvent::Navigated { view: Settings } is emitted
  
  THEN SettingsPresenter calls profile_service.list()
  AND ViewCommand::SetProfiles sent with 3 profiles
END TEST
```

### Test: Default Profile Highlighted

```pseudocode
TEST default_profile_highlighted
  // REQ-PRF-001.2
  GIVEN Profile B is set as default in AppSettingsService
  AND SettingsView renders profiles [A, B, C]
  
  THEN Profile B row has highlighted background
  AND Profiles A, C have normal background
END TEST
```

### Test: Profile Display Format

```pseudocode
TEST profile_display_format
  // REQ-PRF-001.3
  GIVEN Profile with name="Claude", provider="anthropic", model="claude-3-5-sonnet"
  
  WHEN ProfileItem is created
  
  THEN display == "Claude (anthropic:claude-3-5-sonnet)"
END TEST
```

### Test: Select Profile Emits Event

```pseudocode
TEST select_profile_emits_event
  // REQ-PRF-002.1
  GIVEN SettingsView with profile B visible
  AND event_bus subscriber
  
  WHEN User clicks profile B row
  
  THEN UserEvent::SelectProfile { id: B.id } is emitted
END TEST
```

### Test: Select Profile Updates Default

```pseudocode
TEST select_profile_updates_default
  // REQ-PRF-002.2
  GIVEN SettingsPresenter receives SelectProfile { id: B.id }
  
  WHEN handle_select_profile runs
  
  THEN app_settings.set_default_profile_id(B.id) is called
END TEST
```

### Test: DefaultChanged Updates View

```pseudocode
TEST default_changed_updates_view
  // REQ-PRF-002.3
  GIVEN ProfileEvent::DefaultChanged { profile_id: Some(B.id) } emitted
  
  WHEN SettingsPresenter handles event
  
  THEN ViewCommand::SetDefaultProfile { id: Some(B.id) } is sent
END TEST
```

### Test: Add Profile Navigation Flow

```pseudocode
TEST add_profile_navigation_flow
  // REQ-PRF-003.1
  GIVEN SettingsView with [+] button
  
  WHEN User clicks [+]
  
  THEN UserEvent::Navigate { to: ModelSelector } is emitted
  
  WHEN Model is selected
  
  THEN UserEvent::Navigate { to: ProfileEditor { id: None } } is emitted
END TEST
```

### Test: Edit Profile Navigation

```pseudocode
TEST edit_profile_navigation
  // REQ-PRF-003.2
  GIVEN Profile C selected in SettingsView
  
  WHEN User clicks [Edit]
  
  THEN UserEvent::Navigate { to: ProfileEditor { id: Some(C.id) } } is emitted
END TEST
```

### Test: Delete Profile Confirmation

```pseudocode
TEST delete_profile_confirmation
  // REQ-PRF-003.3
  GIVEN Profile B selected
  
  WHEN User clicks [-]
  
  THEN ViewCommand::ShowDeleteProfileConfirmation { id: B.id } is sent
  
  WHEN User confirms deletion
  THEN UserEvent::ConfirmDeleteProfile { id: B.id } is emitted
  THEN profile_service.delete(B.id) is called
END TEST
```

### Test: Profile CRUD Refreshes List

```pseudocode
TEST profile_crud_refreshes_list
  // REQ-PRF-003.4
  GIVEN SettingsPresenter subscribed
  
  WHEN ProfileEvent::Created { id, name } is emitted
  THEN load_profiles() is called
  
  WHEN ProfileEvent::Updated { id, name } is emitted  
  THEN load_profiles() is called
  
  WHEN ProfileEvent::Deleted { id, name } is emitted
  THEN load_profiles() is called
END TEST
```

### Test: Edit Profile Populates Fields

```pseudocode
TEST edit_profile_populates_fields
  // REQ-PRF-004.1
  GIVEN ProfileEditorView with profile_id = C.id
  AND ProfileService returns profile C
  
  WHEN ProfileEditorPresenter handles navigation
  
  THEN ViewCommand::SetProfileEditorData sent with C's data
END TEST
```

### Test: Save Profile Validation

```pseudocode
TEST save_profile_validation
  // REQ-PRF-004.2
  GIVEN ProfileEditorData with empty name
  
  WHEN handle_save_profile is called
  
  THEN ViewCommand::ShowFieldError { field: "name", error: "Name is required" } sent
  AND profile_service.create/update is NOT called
END TEST
```

### Test: Auth Method Shows Correct Fields

```pseudocode
TEST auth_method_shows_correct_fields
  // REQ-PRF-004.3
  GIVEN ProfileEditorView with auth_method = AuthMethod::KeyFile
  
  WHEN View renders auth section
  
  THEN Keyfile path field is visible
  AND API key field is NOT visible
END TEST
```

---

## Error Handling

| Error Condition | Handling Strategy | User Impact |
|-----------------|-------------------|-------------|
| Profile list load fails | Show error banner | See error, can retry |
| Set default fails | Show error banner | Selection not persisted |
| Profile not found (edit) | Navigate back, show error | Returns to Settings |
| Delete fails | Show error banner | Profile not deleted |
| Save validation fails | Highlight field | Can correct and retry |
| Save persist fails | Show error banner | Can retry |
