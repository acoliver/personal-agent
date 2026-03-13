# Pseudocode 01: Authoritative GPUI App Store

## Overview

Single durable store that receives startup hydration and runtime updates, then exposes snapshots to popup views.

## Store Construction (Lines 1-195)

Pseudocode note: `TranscriptSnapshotMessage` is placeholder pseudocode naming for the concrete transcript payload/snapshot type already established in the codebase (for example `ConversationMessagePayload` if that remains the active boundary type). This plan does not require inventing a new `RenderedMessage` type. When implementing, prefer the concrete repo type name once Phase 00a confirms the active transcript payload boundary.

```text
001  STRUCT GpuiAppStore {
002    chat: ChatStoreState
003    history: HistoryStoreState
004    settings: SettingsStoreState
005    revision: u64
006  }
007
008  STRUCT ChatStoreState {
009    selected_conversation_id: Option<Uuid>
010    selected_conversation_title: String
011    selected_title_provenance: SelectedTitleProvenance
012    selection_generation: u64
013    load_state: ConversationLoadState
014    transcript: Vec<TranscriptSnapshotMessage>
015    streaming: StreamingState
016    last_finalized_stream_guard: Option<FinalizedStreamGuard>
017  }
018
019  STRUCT HistoryStoreState {
020    conversations: Vec<ConversationSummary>
021    selected_conversation_id: Option<Uuid>  // render-selection slice aligned to authoritative chat selection, not a second semantic authority
022  }
023
024  STRUCT SettingsStoreState {
025    profiles: Vec<ProfileSummary>
026    selected_profile_id: Option<Uuid>
027    settings_visible: bool
028  }
029
030  STRUCT StreamingState {
031    thinking_visible: bool
032    thinking_buffer: String
033    stream_buffer: String
034    last_error: Option<String>
035    active_target: Option<Uuid>
036  }
037
038  STRUCT FinalizedStreamGuard {
039    conversation_id: Uuid
040    transcript_len_after_finalize: usize
041  }
042
043  ENUM SelectedTitleProvenance {
044    HistoryBacked(String)
045    LiteralFallback("Untitled Conversation")
046  }
047
048  ENUM BeginSelectionMode {
049    PublishImmediately,
050    BatchNoPublish,
051  }
052
053  ENUM BeginSelectionResult {
054    NoOpSameSelection,
055    BeganSelection { generation: u64 },
056  }
057
058  ENUM ConversationLoadState {
059    Idle,
060    Loading { conversation_id: Uuid, generation: u64 },
061    Ready { conversation_id: Uuid, generation: u64 },
062    Error { conversation_id: Uuid, generation: u64, message: String },
063  }
064
065  FUNCTION new_store() -> GpuiAppStore:
066    RETURN store with empty snapshots,
067      selected_conversation_id = None,
068      selected_conversation_title = "",
069      selected_title_provenance = LiteralFallback("Untitled Conversation"),
070      selection_generation = 0,
071      load_state = Idle,
072      transcript = [],
073      last_finalized_stream_guard = None
074
075  FUNCTION current_snapshot(store) -> GpuiAppSnapshot:
076    CLONE chat/history/settings state required by views
077    INCLUDE revision
078
079  FUNCTION bump_revision(store):
080    store.revision += 1
081
082  FUNCTION publish_snapshot(store, subscribers):
083    LET snapshot = current_snapshot(store)
084    IF subscribers.is_empty():
085      RETURN silently
086    FOR each subscriber:
087      subscriber.receive(snapshot)
088
089  FUNCTION title_value(provenance: SelectedTitleProvenance) -> String:
090    MATCH provenance:
091      HistoryBacked(title) => RETURN title
092      LiteralFallback(title) => RETURN title
093
094  FUNCTION derive_selected_title_provenance(store, conversation_id) -> SelectedTitleProvenance:
095    IF authoritative_history_has_non_empty_title(store.history, conversation_id):
096      RETURN HistoryBacked(authoritative_history_title(store.history, conversation_id))
097    RETURN LiteralFallback("Untitled Conversation")
098
099  FUNCTION begin_selection(store, conversation_id, mode: BeginSelectionMode) -> BeginSelectionResult:
100    IF store.chat.selected_conversation_id == Some(conversation_id)
101      AND store.chat.load_state matches Loading/Ready for store.chat.selection_generation:
102        RETURN NoOpSameSelection
103    LET next_generation = store.chat.selection_generation + 1
104    LET title_provenance = derive_selected_title_provenance(store, conversation_id)
105    store.chat.selected_conversation_id = Some(conversation_id)
106    store.history.selected_conversation_id = Some(conversation_id)
107    store.chat.selected_conversation_title = title_value(title_provenance)
108    store.chat.selected_title_provenance = title_provenance
109    store.chat.selection_generation = next_generation
110    store.chat.load_state = Loading {
111      conversation_id,
112      generation: next_generation,
113    }
114    clear_streaming_ephemera_only(store)
115    store.chat.last_finalized_stream_guard = None
116    NOTE: begin_selection(...) is the authoritative mutation helper for ordinary-runtime selection issuance
117    NOTE: startup reuses this same helper only through reduce_startup_batch(startup_inputs); startup does not get a second reducer branch for selection semantics
118    NOTE: same-conversation retry from Error is explicit here because the Loading/Ready no-op branch above does not match Error state
119    IF mode == PublishImmediately:
120      bump_revision(store)
121      publish_snapshot(store, subscribers)
122    RETURN BeganSelection { generation: next_generation }
123
124  FUNCTION reduce_batch(store, commands) -> bool:
125    LET changed = false
126    FOR each command in commands:
127      changed = reduce_view_command_without_publish(store, command) OR changed
128    IF changed:
129      bump_revision(store)
130      publish_snapshot(store, subscribers)
131    RETURN changed
132
133  FUNCTION reduce_startup_batch(store, startup_inputs) -> bool:
134    ASSERT store.revision == 0
135    ASSERT store.chat.selected_conversation_id == None
136    ASSERT store.chat.selected_conversation_title == ""
137    ASSERT store.chat.selection_generation == 0
138    ASSERT store.chat.load_state == Idle
139    ASSERT store.chat.transcript == []
140    ASSERT store.chat.last_finalized_stream_guard == None
141    LET changed = false
142    changed = mutate_history_snapshot_without_publish(store, startup_inputs.history) OR changed
143    changed = mutate_profile_snapshot_without_publish(store, startup_inputs.profiles, startup_inputs.selected_profile_id) OR changed
144    IF startup_inputs.selected_conversation is None:
145      NOTE: this branch is valid only for fresh startup store state; it does not model a runtime reset path
146    ELSE IF startup_inputs.selected_conversation is Some(selection):
147      MATCH begin_selection(store, selection.conversation_id, BatchNoPublish):
148        NoOpSameSelection =>
149          ASSERT false
150        BeganSelection { generation } =>
151          ASSERT generation == 1
152          changed = true
153      MATCH startup_inputs.startup_mode:
154        ModeA { transcript_result: Success(messages) } =>
155          changed = reduce_view_command_without_publish(store, ConversationMessagesLoaded {
156            conversation_id: selection.conversation_id,
157            selection_generation: 1,
158            messages,
159          }) OR changed
160        ModeA { transcript_result: Failure(message) } =>
161          changed = reduce_view_command_without_publish(store, ConversationLoadFailed {
162            conversation_id: selection.conversation_id,
163            selection_generation: 1,
164            message,
165          }) OR changed
166        ModeB { transcript_unavailable_reason, pending_generation } =>
167          ASSERT pending_generation == 1
168          ASSERT transcript_unavailable_reason in { StartupServiceSeamUnavailable, AsyncOnlySourceBeforeMount, StartupCompositionDoesNotProvideTranscriptOutcome }
169          NOTE: ModeB is allowed only when transcript outcome is genuinely unavailable at startup for the repo-grounded seam class carried in transcript_unavailable_reason
170          NOTE: first visible selected startup snapshot may therefore be explicit Loading for generation 1 only in this bounded mode
171          NOTE: later success/failure for generation 1 must arrive through the ordinary runtime-pump reducer path
172    IF changed:
173      bump_revision(store)
174      publish_snapshot(store, subscribers)
175    RETURN changed
176
177  RULE: popup mount state does not own durable chat transcript
178  RULE: startup hydration and runtime commands mutate same store
179  RULE: views render snapshots and emit intents only
180  RULE: mounted views may keep only ephemeral render cache/UI state and must not own a durable semantic transcript model independent of the store snapshot
181  RULE: any local transcript collection that still exists during migration is a transient rendering cache overwritten from store snapshot on relevant revision changes and on bounded-clear restoration
182  RULE: no mounted view may accept presenter-originated transcript/state updates as an authority path after Phase 05
183  RULE: startup hydration commits one coherent startup batch before popup subscription/mount when startup already knows selected transcript outcome
184  RULE: no first-frame empty/loading flash is allowed for already-known startup transcript data
185  RULE: no popup subscriber exists before that startup batch commits, so no subscriber can observe startup-intermediate Loading/empty state for already-known startup transcript data
186  RULE: any publication before popup subscription is a silent no-op and the first subscriber reads `current_snapshot()` immediately on subscription rather than consuming a queued bootstrap event
187  RULE: if startup selected conversation is known but `startup_inputs.startup_mode` is explicit ModeB because transcript outcome is genuinely unavailable, the committed first visible selected startup snapshot may be explicit Loading for generation 1
188  RULE: if startup has no selected conversation, the committed startup snapshot is explicit no-selection / Idle state with generation 0, empty transcript, and empty selected title
189  RULE: ignored/no-op/stale commands do not change revision and do not publish
190  RULE: no parallel startup-only transcript owner remains after migration
191  RULE: begin_selection(...) is the only ordinary-runtime minting site; startup may call the same function only in BatchNoPublish mode inside reduce_startup_batch(startup_inputs)
192  RULE: reduce_startup_batch(startup_inputs) is the only allowed startup transaction API for selected-conversation hydration
193  RULE: startup authoritative semantics are `begin_selection(..., BatchNoPublish)` plus only success/failure completion; any startup-synthesized `ConversationActivated` is compatibility-only/readback-only and non-authoritative
194  RULE: one drained bridge batch maps to one reducer entrypoint call and may publish at most one publication attempt for that batch
195  RULE: HistoryStoreState.selected_conversation_id is a store-owned render-selection slice aligned to authoritative chat selection, not a second semantic authority
```

## Command Reduction (Lines 196-382)

```text
196  FUNCTION maybe_upgrade_selected_title_from_history(store, conversation_id) -> bool:
197    IF store.chat.selected_conversation_id != Some(conversation_id):
198      RETURN false
199    IF NOT authoritative_history_has_non_empty_title(store.history, conversation_id):
200      RETURN false
201    LET history_title = authoritative_history_title(store.history, conversation_id)
202    MATCH store.chat.selected_title_provenance:
203      HistoryBacked(current_title) =>
204        IF current_title == history_title:
205          RETURN false
206        RETURN false  // bounded title correction never overwrites one history-backed title with another here
207      LiteralFallback(_) =>
208        store.chat.selected_conversation_title = history_title
209        store.chat.selected_title_provenance = HistoryBacked(history_title)
210        RETURN true
211
212  FUNCTION maybe_sync_selected_title(store) -> bool:
213    IF store.chat.selected_conversation_id is None:
214      RETURN false
215    RETURN maybe_upgrade_selected_title_from_history(store, store.chat.selected_conversation_id.unwrap())
216
217  FUNCTION reduce_view_command_without_publish(store, command) -> bool:
218    MATCH command:
219      ConversationListRefreshed { conversations } =>
220        IF conversations == store.history.conversations:
221          RETURN false
222        store.history.conversations = conversations
223        maybe_sync_selected_title(store)
224        RETURN true
225
226      ConversationActivated { id, selection_generation } =>
227        IF store.chat.selected_conversation_id == Some(id)
228          AND store.chat.selection_generation == selection_generation
229          AND store.chat.load_state == Loading { conversation_id: id, generation: selection_generation }:
230          RETURN false
231        IF store.chat.selected_conversation_id == Some(id)
232          AND store.chat.selection_generation == selection_generation:
233          RETURN maybe_upgrade_selected_title_from_history(store, id)
234        IF store.chat.selection_generation > selection_generation:
235          RETURN false
236        IF store.chat.selection_generation < selection_generation:
237          NOTE: ordinary-runtime activation must never advance authoritative generation; higher incoming generation is protocol violation/no-op because begin_selection(...) is the sole ordinary-runtime minting site
238          RETURN false
239        NOTE: startup hydration does not rely on `ConversationActivated` for authoritative state transition; startup uses reduce_startup_batch(startup_inputs) with begin_selection(..., BatchNoPublish) plus only success/failure completion inside the same transaction
240        RETURN false
241
242      ConversationMessagesLoaded { conversation_id, selection_generation, messages } =>
243        IF store.chat.selected_conversation_id != Some(conversation_id):
244          RETURN false
245        IF selection_generation != store.chat.selection_generation:
246          RETURN false
247        IF load_state_targets_different_conversation(store, conversation_id):
248          RETURN false
249        LET mapped = map_payload_to_snapshot_messages(messages)
250        LET next_state = Ready { conversation_id, generation: selection_generation }
251        IF store.chat.transcript == mapped AND store.chat.load_state == next_state:
252          RETURN false
253        store.chat.transcript = mapped
254        store.chat.load_state = next_state
255        clear_streaming_ephemera_only(store)
256        store.chat.last_finalized_stream_guard = None
257        RETURN true
258
259      ConversationLoadFailed { conversation_id, selection_generation, message } =>
260        IF store.chat.selected_conversation_id != Some(conversation_id):
261          RETURN false
262        IF selection_generation != store.chat.selection_generation:
263          RETURN false
264        IF load_state_targets_different_conversation(store, conversation_id):
265          RETURN false
266        LET next_state = Error {
267          conversation_id,
268          generation: selection_generation,
269          message,
270        }
271        IF store.chat.load_state == next_state:
272          RETURN false
273        store.chat.load_state = next_state
274        clear_streaming_ephemera_only(store)
275        RETURN true
276
277      MessageAppended { conversation_id, role, content } =>
278        IF role == Assistant
279          AND store.chat.last_finalized_stream_guard matches Some(guard)
280          AND conversation_id == guard.conversation_id
281          AND len(store.chat.transcript) == guard.transcript_len_after_finalize
282          AND transcript_tail_exists(store.chat.transcript)
283          AND transcript_tail_role(store.chat.transcript) == Assistant
284          AND transcript_tail_content(store.chat.transcript) == content:
285            RETURN false
286        RETURN append_persisted_message_if_target_matches_selected(store, conversation_id, role, content)
287        NOTE: this path remains the durable append path for user messages and existing non-stream append cases
288
289      ShowThinking =>
290        RETURN show_thinking_if_target_matches_selected_or_nil(store, command)
291
292      HideThinking =>
293        RETURN hide_thinking_if_target_matches_selected_or_nil(store, command)
294
295      AppendThinking =>
296        RETURN append_thinking_buffer_if_target_matches_selected_or_nil(store, command)
297
298      AppendStream =>
299        RETURN append_stream_buffer_if_target_matches_selected_or_nil(store, command)
300
301      FinalizeStream { conversation_id, tokens } =>
302        LET resolved_target = resolve_nil_or_explicit_target(store, conversation_id)
303        IF resolved_target is None:
304          RETURN false
305        IF resolved_target != store.chat.selected_conversation_id:
306          RETURN false
307        IF store.chat.streaming.active_target != Some(resolved_target):
308          RETURN false
309        IF store.chat.streaming.stream_buffer is empty:
310          RETURN false
311        LET assistant_payload = ConversationMessagePayload {
312          role: Assistant,
313          content: store.chat.streaming.stream_buffer,
314          thinking_content: non_empty_or_none(store.chat.streaming.thinking_buffer),
315          timestamp: None,
316        }
317        append assistant_payload exactly once to durable transcript
318        store.chat.last_finalized_stream_guard = Some(FinalizedStreamGuard {
319          conversation_id: resolved_target,
320          transcript_len_after_finalize: len(store.chat.transcript),
321        })
322        NOTE: authoritative transcript snapshot stays in the replay-compatible payload shape already used by ConversationMessagesLoaded
323        NOTE: mounted ChatView render may still derive ChatMessage::assistant(..., current_model) from that payload using the existing replay-mapping style rather than persisting a second streamed-message shape
324        NOTE: `tokens` remain available to surrounding runtime/event accounting but do not require inventing a new persisted transcript field in this recovery
325        NOTE: this closes exactly one active stream lifecycle keyed by streaming.active_target + non-empty active stream buffer before clear
326        NOTE: deterministic reducer-side dedupe uses last_finalized_stream_guard plus exact transcript-tail comparison on conversation id, transcript length, assistant role, and assistant content; timestamp/model/provider/finalized-thinking are intentionally ignored because incoming MessageAppended does not carry enough information to compare them safely here
327        NOTE: repo-wide callsite inventory plus named deterministic streamed-interaction test still validate the guard coverage; do not leave duplication safety as an assumed non-issue
328        store.chat.streaming.stream_buffer = ""
329        store.chat.streaming.thinking_buffer = ""
330        store.chat.streaming.thinking_visible = false
331        store.chat.streaming.active_target = None
332        store.chat.streaming.last_error = None
333        RETURN true
334        NOTE: direct-finalize durable model; do not require a second durable append command for streamed assistant output
335
336      StreamCancelled / StreamError =>
337        RETURN clear_or_mark_streaming_ephemera_if_target_matches_selected_or_nil(store, command)
338
339      ShowToolCall / UpdateToolCall =>
340        NOTE: if GPUI still has no observable tool-call handler in scope, leave store unchanged rather than inventing new behavior in this recovery
341        NOTE: explicit unchanged invariant: these commands do not change revision-driving snapshot state and do not change mounted render output in GPUI during this recovery unless later scope intentionally expands them
342        RETURN false
343
344      ChatProfilesUpdated =>
345        RETURN mutate_profiles_snapshot(store, command)
346
347      ShowSettings =>
348        RETURN mutate_settings_visibility(store, command)
349
350      ConversationRenamed =>
351        RETURN mutate_history_and_selected_title_if_targeted(store, command)
352
353      ConversationDeleted =>
354        RETURN mutate_history_and_selected_selection_if_targeted(store, command)
355
356      ConversationCleared =>
357        preserve current store snapshot in this recovery reducer because clear-conversation behavior remains on the existing view path during this plan
358        DO NOT mutate transcript, selected id/title, generation, load state, or revision
359        DO NOT create a local durable mirror; the mounted clear-handling path (currently `src/ui_gpui/views/chat_view.rs::ChatView::handle_command(ViewCommand::ConversationCleared)`, or an evidence-mapped repo-idiomatic equivalent) may use same-turn authoritative snapshot readback already available inside the current mounted update transaction to restore render state from `current_snapshot()` before control returns to the event loop and without waiting for an unrelated future publication
360        FORBIDDEN near-miss shapes: deferred restore through subscriber callback, cx.spawn/task/timer/frame callback, remount-only repair, or storing a second popup-local transcript authority just to survive the clear
361        RETURN false
362
363      NO OTHER ViewCommand variants are in scope for this recovery reducer in this phase set
364      Unhandled command variants =>
365        leave store unchanged
366        RETURN false
367
368  FUNCTION clear_streaming_ephemera_only(store):
369    store.chat.streaming = StreamingState {
370      thinking_visible: false,
371      thinking_buffer: "",
372      stream_buffer: "",
373      last_error: None,
374      active_target: None,
375    }
376    DO NOT clear transcript for ordinary activation
377
378  FUNCTION load_state_targets_different_conversation(store, conversation_id) -> bool:
379    MATCH store.chat.load_state:
380      Loading { conversation_id: active_id, .. } =>
381        RETURN active_id != conversation_id
382      Ready { conversation_id: active_id, .. } =>
383        RETURN active_id != conversation_id
384      Error { conversation_id: active_id, .. } =>
385        RETURN active_id != conversation_id
386      Idle =>
387        RETURN false
```
