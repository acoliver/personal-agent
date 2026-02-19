# Pseudocode: Settings View Flow

## Plan ID: PLAN-20260219-NEXTGPUIREMEDIATE
## Component: Settings View Profile/MCP List Management
## Requirements: REQ-WIRE-002.2, REQ-WIRE-002.3, REQ-WIRE-005.2

---

## Overview

SettingsView displays lists of profiles and MCPs. It needs to:
1. Receive and display profile/MCP list updates
2. Handle profile CRUD result commands
3. Handle MCP status and config commands
4. Load initial data when navigated to

Currently, SettingsView has `handle_command()` but it only handles navigation commands.

## Current State Analysis

```rust
// settings_view.rs - handle_command() exists but minimal:
pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
    match command {
        ViewCommand::NavigateTo { .. } | ViewCommand::NavigateBack => {
            // Navigation handled by MainPanel
        }
        _ => {
            // Other commands may be added as needed
        }
    }
    cx.notify();
}
```

## Pseudocode: SettingsView Command Handling

```pseudocode
001: FUNCTION SettingsView::handle_command(cmd: ViewCommand, cx: &mut Context<Self>)
002:   MATCH cmd:
003:     // ===== PROFILE COMMANDS ===== @REQ-WIRE-005.2, @REQ-WIRE-002.2
004:     
005:     ViewCommand::ShowSettings { profiles } =>
006:       // Initial load of settings data
007:       self.state.profiles = profiles.into_iter().map(|p| ProfileItem {
008:         id: p.id,
009:         name: p.name,
010:         provider: p.provider_id.clone(),
011:         model: String::new(),  // Not in summary
012:         is_default: p.is_default,
013:       }).collect()
014:       cx.notify()
015:     
016:     ViewCommand::ProfileCreated { id, name } =>
017:       // Add new profile to list
018:       self.state.profiles.push(ProfileItem {
019:         id,
020:         name,
021:         provider: String::new(),  // Will be updated on next refresh
022:         model: String::new(),
023:         is_default: false,
024:       })
025:       // Select the newly created profile
026:       self.state.selected_profile_id = Some(id)
027:       cx.notify()
028:     
029:     ViewCommand::ProfileUpdated { id, name } =>
030:       // Update profile in list
031:       IF LET Some(profile) = self.state.profiles.iter_mut().find(|p| p.id == id):
032:         profile.name = name
033:       END IF
034:       cx.notify()
035:     
036:     ViewCommand::ProfileDeleted { id } =>
037:       // Remove profile from list
038:       self.state.profiles.retain(|p| p.id != id)
039:       // Clear selection if deleted profile was selected
040:       IF self.state.selected_profile_id == Some(id):
041:         self.state.selected_profile_id = None
042:       END IF
043:       cx.notify()
044:     
045:     ViewCommand::DefaultProfileChanged { profile_id } =>
046:       // Update default flag on all profiles
047:       FOR profile IN &mut self.state.profiles:
048:         profile.is_default = Some(profile.id) == profile_id
049:       END FOR
050:       cx.notify()
051:     
052:     // ===== MCP COMMANDS ===== @REQ-WIRE-005.2, @REQ-WIRE-002.3
053:     
054:     ViewCommand::McpServerStarted { id, tool_count } =>
055:       IF LET Some(mcp) = self.state.mcps.iter_mut().find(|m| m.id == id):
056:         mcp.status = McpStatus::Running
057:         mcp.enabled = true
058:       ELSE:
059:         // New MCP - add to list
060:         self.state.mcps.push(McpItem {
061:           id,
062:           name: format!("MCP {}", id),  // Name will be updated
063:           enabled: true,
064:           status: McpStatus::Running,
065:         })
066:       END IF
067:       LOG_INFO "MCP {} started with {} tools", id, tool_count
068:       cx.notify()
069:     
070:     ViewCommand::McpServerFailed { id, error } =>
071:       IF LET Some(mcp) = self.state.mcps.iter_mut().find(|m| m.id == id):
072:         mcp.status = McpStatus::Error
073:         mcp.enabled = false
074:       END IF
075:       LOG_ERROR "MCP {} failed: {}", id, error
076:       cx.notify()
077:     
078:     ViewCommand::McpStatusChanged { id, status } =>
079:       IF LET Some(mcp) = self.state.mcps.iter_mut().find(|m| m.id == id):
080:         mcp.status = match status {
081:           view_command::McpStatus::Starting => McpStatus::Running,  // Treat starting as running
082:           view_command::McpStatus::Running => McpStatus::Running,
083:           view_command::McpStatus::Stopped => McpStatus::Stopped,
084:           view_command::McpStatus::Failed => McpStatus::Error,
085:           view_command::McpStatus::Unhealthy => McpStatus::Error,
086:         }
087:         mcp.enabled = matches!(status, 
088:           view_command::McpStatus::Starting | view_command::McpStatus::Running)
089:       END IF
090:       cx.notify()
091:     
092:     ViewCommand::McpConfigSaved { id } =>
093:       // Config was saved - refresh may be needed
094:       // For now, just log
095:       LOG_INFO "MCP config saved: {}", id
096:       cx.notify()
097:     
098:     ViewCommand::McpDeleted { id } =>
099:       // Remove MCP from list
100:       self.state.mcps.retain(|m| m.id != id)
101:       // Clear selection if deleted MCP was selected
102:       IF self.state.selected_mcp_id == Some(id):
103:         self.state.selected_mcp_id = None
104:       END IF
105:       cx.notify()
106:     
107:     ViewCommand::McpToolsUpdated { tools } =>
108:       // Could display tool count per MCP
109:       LOG_INFO "MCP tools updated: {} tools total", tools.len()
110:       cx.notify()
111:     
112:     // ===== NOTIFICATION COMMANDS =====
113:     
114:     ViewCommand::ShowNotification { message } =>
115:       // Could show toast/banner
116:       LOG_INFO "Notification: {}", message
117:       // For now, ignore - MainPanel handles notifications
118:       ()
119:     
120:     ViewCommand::ShowError { title, message, severity } =>
121:       // Could show error state
122:       LOG_ERROR "Error in settings: {} - {}", title, message
123:       // For now, ignore - MainPanel handles errors
124:       ()
125:     
126:     // ===== NAVIGATION (handled by MainPanel) =====
127:     ViewCommand::NavigateTo { .. } | ViewCommand::NavigateBack =>
128:       ()
129:     
130:     _ =>
131:       // Ignore unrelated commands
132:       ()
133:   END MATCH
134: END FUNCTION
```

## Initial Data Loading

```pseudocode
135: // SettingsView should request data when becoming visible
136: // This could be triggered by MainPanel when navigating to Settings
137: 
138: FUNCTION SettingsView::request_refresh(cx: &mut Context<Self>)
139:   LOG_INFO "SettingsView requesting data refresh"
140:   
141:   // Emit event to request profile list
142:   // SettingsPresenter should handle this and emit ShowSettings
143:   self.emit(UserEvent::Navigate { to: ViewId::Settings })
144:   // Or a dedicated RefreshSettings event
145: END FUNCTION
146:
147: // Alternative: SettingsPresenter loads data on system startup
148: // and emits ShowSettings proactively
149:
150: // In main_panel.rs, when navigating TO settings:
151: FUNCTION MainPanel::navigate_to_settings(cx)
152:   IF self.navigation.current() != ViewId::Settings:
153:     self.navigation.navigate(ViewId::Settings)
154:     
155:     // Trigger data load if not already loaded
156:     IF LET Some(ref settings_view) = self.settings_view:
157:       settings_view.update(cx, |view, cx| {
158:         IF view.state.profiles.is_empty():
159:           view.request_refresh(cx)
160:         END IF
161:       })
162:     END IF
163:     
164:     cx.notify()
165:   END IF
166: END FUNCTION
```

## SettingsPresenter Enhancements

```pseudocode
167: // SettingsPresenter should load initial data on start
168: // and respond to refresh requests
169:
170: FUNCTION SettingsPresenter::start()
171:   // ... existing start code ...
172:   
173:   // Load initial profile/MCP data
174:   self.load_initial_data().await
175: END FUNCTION
176:
177: FUNCTION SettingsPresenter::load_initial_data()
178:   // Load profiles
179:   MATCH self.profile_service.list().await:
180:     Ok(profiles) =>
181:       LET profile_summaries: Vec<ProfileSummary> = profiles.iter().map(|p| ProfileSummary {
182:         id: p.id,
183:         name: p.name.clone(),
184:         provider_id: p.provider_id.clone(),
185:         is_default: p.is_default,
186:       }).collect()
187:       
188:       self.view_tx.send(ViewCommand::ShowSettings {
189:         profiles: profile_summaries,
190:       }).await
191:     
192:     Err(e) =>
193:       LOG_ERROR "Failed to load profiles: {:?}", e
194:   END MATCH
195:   
196:   // Load MCPs
197:   MATCH self.mcp_service.list().await:
198:     Ok(mcps) =>
199:       FOR mcp IN mcps:
200:         self.view_tx.send(ViewCommand::McpStatusChanged {
201:           id: mcp.id,
202:           status: if mcp.enabled { McpStatus::Running } else { McpStatus::Stopped },
203:         }).await
204:       END FOR
205:     
206:     Err(e) =>
207:       LOG_ERROR "Failed to load MCPs: {:?}", e
208:   END MATCH
209: END FUNCTION
210:
211: // Handle refresh request
212: FUNCTION SettingsPresenter::handle_user_event(event)
213:   MATCH event:
214:     UserEvent::Navigate { to: ViewId::Settings } =>
215:       // User navigated to settings - refresh data
216:       self.load_initial_data().await
217:     
218:     // ... existing handlers ...
219:   END MATCH
220: END FUNCTION
```

## McpItem Display Enhancement

```pseudocode
221: // SettingsView should show more MCP info
222: STRUCT McpItem {
223:   id: Uuid,
224:   name: String,
225:   enabled: bool,
226:   status: McpStatus,
227:   tool_count: Option<usize>,  // Add this
228:   error_message: Option<String>,  // Add this for error display
229: }
230:
231: FUNCTION SettingsView::render_mcp_row(mcp: &McpItem, cx) -> impl IntoElement
232:   DIV()
233:     .flex()
234:     .items_center()
235:     .w_full()
236:     .h(px(28.0))
237:     .px(px(8.0))
238:     
239:     // Status indicator
240:     .child(
241:       DIV()
242:         .size(px(8.0))
243:         .rounded_full()
244:         .bg(match mcp.status {
245:           McpStatus::Running => Theme::success(),
246:           McpStatus::Stopped => Theme::text_muted(),
247:           McpStatus::Error => Theme::error(),
248:         })
249:         .mr(px(8.0))
250:     )
251:     
252:     // Name
253:     .child(
254:       DIV()
255:         .flex_1()
256:         .text_size(px(12.0))
257:         .child(mcp.name.clone())
258:     )
259:     
260:     // Tool count (if running)
261:     .when_some(mcp.tool_count.filter(|_| mcp.status == McpStatus::Running), |d, count| {
262:       d.child(
263:         DIV()
264:           .text_size(px(10.0))
265:           .text_color(Theme::text_muted())
266:           .child(format!("{} tools", count))
267:           .mr(px(8.0))
268:       )
269:     })
270:     
271:     // Toggle switch
272:     .child(self.render_mcp_toggle(mcp, cx))
273: END FUNCTION
```

## Files Modified

- `src/ui_gpui/views/settings_view.rs` - Full handle_command implementation (lines 001-134)
- `src/presentation/settings_presenter.rs` - Add load_initial_data (lines 167-220)
- `src/ui_gpui/views/main_panel.rs` - Trigger refresh on navigation (lines 150-166)

## Verification Pseudocode

```pseudocode
274: TEST verify_profile_list_display():
275:   // Setup
276:   LET settings_view = SettingsView::new(cx)
277:   
278:   // Simulate ShowSettings command
279:   settings_view.handle_command(ViewCommand::ShowSettings {
280:     profiles: vec![
281:       ProfileSummary { id: uuid1, name: "Claude", provider_id: "anthropic", is_default: true },
282:       ProfileSummary { id: uuid2, name: "GPT-4", provider_id: "openai", is_default: false },
283:     ],
284:   }, cx)
285:   
286:   // Verify state
287:   ASSERT settings_view.state.profiles.len() == 2
288:   ASSERT settings_view.state.profiles[0].is_default == true
289: END TEST
290:
291: TEST verify_profile_deletion():
292:   LET settings_view = SettingsView::new(cx)
293:   settings_view.state.profiles = vec![
294:     ProfileItem { id: uuid1, name: "Test", ... },
295:   ]
296:   settings_view.state.selected_profile_id = Some(uuid1)
297:   
298:   // Delete profile
299:   settings_view.handle_command(ViewCommand::ProfileDeleted { id: uuid1 }, cx)
300:   
301:   // Verify removed and deselected
302:   ASSERT settings_view.state.profiles.is_empty()
303:   ASSERT settings_view.state.selected_profile_id.is_none()
304: END TEST
305:
306: TEST verify_mcp_status_update():
307:   LET settings_view = SettingsView::new(cx)
308:   settings_view.state.mcps = vec![
309:     McpItem { id: uuid1, name: "FS", status: McpStatus::Stopped, ... },
310:   ]
311:   
312:   // Update status
313:   settings_view.handle_command(ViewCommand::McpStatusChanged {
314:     id: uuid1,
315:     status: view_command::McpStatus::Running,
316:   }, cx)
317:   
318:   // Verify
319:   ASSERT settings_view.state.mcps[0].status == McpStatus::Running
320:   ASSERT settings_view.state.mcps[0].enabled == true
321: END TEST
```

## Edge Cases

1. **ShowSettings with empty profiles**: Display "No profiles" message
2. **ProfileDeleted for non-existent ID**: Silently ignore (already deleted)
3. **McpStatusChanged for unknown MCP**: Could add new entry or ignore
4. **Rapid status changes**: Latest status wins
5. **Navigation while loading**: Cancel previous load, start new one
