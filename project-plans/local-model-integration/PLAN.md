# Plan: Local Model Integration (in-process Granite 4.2 3B)

Plan ID: PLAN-20260903-LOCALMODEL
Generated: 2026-09-03
Total Phases: 5 (+ Phase 0.5 preflight)
UI Prototype: `dev-docs/mockups/local-model-settings.html`
Proof-of-concept: `experiments/localmodel-toy` (committed c5b312f; verified working on M4 Max)

## Critical Reminders

1. Complete preflight verification (Phase 0.5) before implementing.
2. The engine is IN-PROCESS ONLY: llama.cpp compiled into the binary via `llama-cpp-2`, mmap-loaded GGUF, dedicated actor thread. No sockets, no subprocess, no HTTP.
3. Integration tests before unit polish where feasible; tests must not require the GGUF file (gate with `#[ignore]` or file-existence).
4. The analysis (codeanalyzer, 2026-09-03) is the source of truth for file:line references below; re-verify with grep during preflight.

## Requirements

- REQ-LM-001: Local engine settings — model path, context size, GPU layers, idle-unload toggle + timeout; persisted app-level (not per-profile).
- REQ-LM-002: First-install default — when the profiles dir has no profiles AND no default.json, seed a "Granite (local)" profile and make it the default. Existing installs untouched. Idempotent across the two boot-time service instances.
- REQ-LM-003: In-process engine — actor thread owns `LlamaBackend` + `LlamaModel` + `LlamaContext` (all `!Send`); requests serialized; generation cancelable; Metal offload.
- REQ-LM-004: `serdes_ai::Model` implementation — `request`/`request_stream` with tool calling via the Granite 4.2 chat template; must bypass `NormalizingSseModel`.
- REQ-LM-005: Lazy load/unload — load on first request with a local profile; unload on idle timeout, on profile update/delete (existing invalidate hooks), and via explicit Unload button.
- REQ-LM-006: Settings UI — new `LocalModel` category: status card (state, layers, ctx, tok/s), model path, n_ctx, GPU layers, idle settings, Unload/Load buttons.
- REQ-LM-007: Profile editor — `ApiType::Local` no longer bakes `https://api.openai.com/v1` into `base_url`; no API key required; sampling fields map to the engine sampler.
- REQ-LM-008: Chat selector — local profile selectable per-conversation like any other; switching takes effect on next send (existing behavior).

## Key Facts (from the verified PoC + codebase analysis)

Verified llama-cpp-2 0.1.156 API facts (confirmed against vendored source in the toy):
- `LlamaBackend::init()` once-only, process-global; keep the guard alive on the actor thread.
- `LlamaModelParams::default().with_n_gpu_layers(999)` (clamped to layer count; default is already -1/all).
- `LlamaContextParams::default()` has `n_ctx = 512` — MUST set `.with_n_ctx(...)`, `.with_n_threads(...)`, `.with_n_batch(n_ctx)`.
- `sampler.sample(&ctx, idx)` already accepts internally — NEVER call `sampler.accept()` after.
- `model.is_eog_token(t)` ends generation. Reuse ONE `encoding_rs::Decoder` per generation (split UTF-8).
- `str_to_token(&prompt, AddBos)` parses special tokens; ChatML prompt needs `AddBos::Always` after runtime probe.
- Multi-turn: re-render full prompt, `clear_kv_cache()`, decode from pos 0 (no prefix caching in v1).
- Granite template: hand-rendered (toy `render.rs` — port it; minijinja is NOT used and NOT needed).
- Tool-call dialect: XML `<tool_call><function=NAME><parameter=K>V</parameter></function></tool_call>` (toy `toolcall.rs` — port it with its tests).
- Sampling that worked: temp 0.1 with top_k/min_p chain, seed 1234, max_tokens 1024.
- Performance: 53–90 tok/s decode, 41/41 Metal layers, peak RSS 4.35 GiB at n_ctx 8192.

Codebase seams (analysis 2026-09-03):
- `LlmClient::build_model` src/llm/client.rs:427-489 — THE seam; intercept `provider == "local"` FIRST, bypass `NormalizingSseModel` (open-responses precedent comment at :482-487).
- `ProfileServiceImpl::normalize_api_base_url` src/services/profile_impl.rs:35-40 bakes api.openai.com into local profiles today — must skip `provider_id == "local"`.
- Lazy-load template: `src/llm/open_responses.rs` SESSIONS map + `invalidate_profile` already called from `ProfileServiceImpl::update` (:551) and `delete` (:574).
- Actor pattern: `src/db/worker.rs` (dedicated thread + std mpsc + tokio oneshot); streaming push: `src/services/chat_impl/streaming.rs` unbounded mpsc.
- Settings panel pattern: Backup panel (state + render_backup_panel.rs + ViewCommand status updates).
- `ApiType::Local` exists in profile editor (src/ui_gpui/views/profile_editor_view/mod.rs:34-150) but is UI-only today.
- GPUI: no slow work on GPUI thread; UI updates via ViewCommand → flume → 100 ms pump.
- serdes_ai::Model trait at ~/.cargo/git/checkouts/serdesai-4da679e9894f01f3/e675674/serdes-ai-models/src/model.rs:108-168 (pinned rev). `ModelResponseStreamEvent` variants must be read from that checkout and matched exactly.

---

# Phase 0.5: Preflight Verification

## Phase ID
`PLAN-20260903-LOCALMODEL.P05A`

## Purpose
Verify every assumption before writing code.

## Checks
| Assumption | How to verify | Status |
|---|---|---|
| `build_model` shape as described | read src/llm/client.rs:427-509 | [ ] |
| serdes_ai::Model + ModelResponseStreamEvent variants | read pinned checkout model.rs + response types | [ ] |
| ToolDefinition shape (for rendering `<tools>`) | read serdes-ai-models tool types | [ ] |
| ModelRequest carries roles + tool_use/tool_result parts | read ModelRequest definition | [ ] |
| open_responses invalidate hooks callable from profile service | grep invalidate_profile | [ ] |
| `ApiType::Local` + profile_editor tests | read profile_editor_view/tests.rs:200,551-616 | [ ] |
| SettingsCategory/ALL/reducers | read settings_view/types.rs:154-190, app_store.rs:380 | [ ] |
| llama-cpp-2 0.1.156 builds inside the app's dependency tree | `cargo add` dry-run / lockfile check | [ ] |
| capabilities_for("local") | read src/models/capabilities.rs | [ ] |

## Blocking Issues Found
[fill during preflight]

## Verification Gate
- [ ] All rows verified. STOP if any fails.

---

# Phase 01: Local Model Settings + First-Install Seeding

## Phase ID
`PLAN-20260903-LOCALMODEL.P01`

## Prerequisites
Phase 0.5 completed.

## Requirements Implemented

### REQ-LM-001: Local engine settings
**Full Text**: The app persists engine-level local model configuration: GGUF path, context size (n_ctx), GPU layers, idle-unload enabled, idle timeout minutes.
**Behavior**:
- GIVEN: app settings exist at `<data_local>/PersonalAgent/app_settings.json`
- WHEN: local model settings are loaded
- THEN: `LocalModelSettings` is read from `extra_settings["local_model"]` with defaults: path `<data_local>/PersonalAgent/models/granite-4.2-3b-Q8_0.gguf` (overridable by env `PA_LOCAL_GGUF`), n_ctx 8192, gpu_layers 999, idle_unload true, idle_timeout_minutes 5.

**Why This Matters**: One engine serves many profiles; engine knobs are app-level, editable without touching profiles.

### REQ-LM-002: First-install default
**Full Text**: On first install (no profiles, no default.json), a local profile is created and set as default.
**Behavior**:
- GIVEN: profiles dir contains no `*.json` profile files and no default.json
- WHEN: `ProfileServiceImpl::initialize()` runs
- THEN: a profile is created with name "Granite (local)", provider_id "local", model_id "granite-4.2-3b", auth None, base_url "", and it is the default.
- GIVEN: any profile exists or default.json exists
- WHEN: initialize() runs
- THEN: nothing is seeded.

**Why This Matters**: Out of the box, chat works with zero configuration.

## Implementation Tasks

### Files to Create
- `src/services/local_model_settings.rs` — `LocalModelSettings` struct + load/save via AppSettingsService `get_setting`/`set_setting` (BackupSettings precedent). `/// @plan:PLAN-20260903-LOCALMODEL.P01` `/// @requirement:REQ-LM-001`
- `tests/local_model_settings_tests.rs` — defaults, round-trip, env override. `@requirement:REQ-LM-001`

### Files to Modify
- `src/services/mod.rs` — register module.
- `src/services/profile_impl.rs` — `initialize()` calls idempotent `seed_default_local_profile()` (empty list + no default.json ⇒ create via existing `create()` path so first-profile auto-default logic applies :470-481). `@requirement:REQ-LM-002`
- `src/config/provider_defaults.rs` / `src/services/profile_impl.rs:normalize_api_base_url` — do NOT default-fill base_url for provider "local" (persist "" instead of api.openai.com). `@requirement:REQ-LM-007` (part 1)
- `tests/profile_seeding_tests.rs` (new) — empty dir seeds; existing profiles don't; default.json-present doesn't; second initialize() is a no-op. `@requirement:REQ-LM-002`

## Verification Commands
```bash
cargo test local_model_settings profile_seeding
grep -r "@plan:PLAN-20260903-LOCALMODEL.P01" src tests | wc -l   # Expected: > 0
grep -rn "api.openai.com" src/services/profile_impl.rs            # local must not map here
```

---

# Phase 02: Local Engine Actor + serdes_ai::Model Implementation

## Phase ID
`PLAN-20260903-LOCALMODEL.P02`

## Prerequisites
Phase 01 completed.

## Requirements Implemented

### REQ-LM-003: In-process engine actor
**Full Text**: A dedicated OS thread owns all `!Send` llama.cpp state. Jobs arrive over std mpsc: Load, Unload, Generate (streaming via tokio unbounded mpsc), Abort, Status. All requests serialize; cancellation aborts between tokens; idle timer unloads.
**Behavior**:
- GIVEN: engine not loaded
- WHEN: a Generate arrives
- THEN: the actor loads the GGUF (status → Loading → Loaded/Error), then generates.
- GIVEN: a stream dropped by the consumer
- WHEN: the guard drops
- THEN: an Abort is sent and the actor stops generating at the next token boundary.

**Why This Matters**: llama.cpp state cannot cross threads; the actor keeps it off the async runtime while exposing an async-friendly interface.

### REQ-LM-004: serdes_ai::Model implementation
**Full Text**: `LocalLlamaModel` implements `Model::request`/`request_stream`: renders messages+tools via the Granite template, generates, parses tool calls, emits `ModelResponseStreamEvent`s (Text deltas, ToolUse parts, StreamComplete with usage). Wrapped model must NOT be `NormalizingSseModel`.
**Behavior**:
- GIVEN: requests with tools
- WHEN: generation emits `<tool_call>` blocks
- THEN: the stream yields ToolUse parts and the agent loop executes tools.

**Why This Matters**: The entire app (chat, agent loop, tool dispatch) works unmodified.

## Implementation Tasks

### Files to Create
- `src/llm/local/mod.rs` — module root; `local_model_for(profile) -> Arc<dyn Model>` registry keyed by profile settings hash (not per-conversation); `unload()`, `status()`, `invalidate_local()` (called by the existing profile-service invalidation sites); engine singleton behind `OnceLock` (tolerate both boot service stacks). `@requirement:REQ-LM-005`
- `src/llm/local/engine.rs` — actor thread, job enum, load/unload/generate/abort/idle logic, status machine (NotLoaded/Loading/Loaded{layers,ctx,last_tok_s}/Error{msg}). Generation loop ported from the toy (decode loop, sampler chain, decoder reuse, tok/s). `@requirement:REQ-LM-003`
- `src/llm/local/llama_model.rs` — `LocalLlamaModel` implementing `serdes_ai::Model`; `profile()` returns a capability-correct serdes ModelProfile (tools + streaming supported); `request` = collect `request_stream`. `@requirement:REQ-LM-004`
- `src/llm/local/render.rs` — port from toy (system+tools, user, assistant tool_calls, tool_response, generation prompt, thinking disabled). `@requirement:REQ-LM-004`
- `src/llm/local/toolcall.rs` — port from toy incl. its 18 tests. `@requirement:REQ-LM-004`
- `src/llm/local/generator.rs` — `trait Generator` seam: `generate(prompt, tools, settings, callbacks) -> GenOutcome` so the Model impl is testable without a GGUF; real impl wraps the engine actor, scripted impl for tests. `@requirement:REQ-LM-004`
- `tests/local_engine_tests.rs` — scripted-generator tests: text-only stream; tool-call parse to ToolUse events; multi tool_call; cancel; load-on-first-use; idle unload (short timeout); serialization of concurrent requests. `#[ignore]` e2e with real GGUF if `PA_LOCAL_GGUF` present. `@requirement:REQ-LM-003 REQ-LM-004`

### Files to Modify
- `src/llm/mod.rs` — register `local` module.
- `src/llm/client.rs` `build_model` — first branch: `provider == "local"` → `crate::llm::local::local_model_for(&self.profile)`; NO NormalizingSseModel wrap (comment why, mirror open-responses precedent :482-487). Also skip `set_api_key_env` and the 120 s timeout for local (pass through but engine ignores load-phase timeouts — document). `@requirement:REQ-LM-004`
- `src/models/capabilities.rs` — ensure `capabilities_for("local")` enables tools + streaming (adjust if currently off). `@requirement:REQ-LM-004`

## Design Constraints (binding)
- Port the toy's verified generation loop faithfully; do not redesign the sampler chain (penalties/top_k/min_p/temp/dist; greedy when temperature ≤ 0.01).
- Map `ModelSettings`: temperature, top_p (clamp into chain), max_tokens (cap generation), seed (dist seed; None → entropy), stop (extra stop strings), ignore timeout during load.
- `ModelRequest` conversion must read the actual part types from the pinned checkout (text parts, tool_use blocks, tool_result) — do not guess.
- Emission granularity: text deltas per token via `PartDelta`; tool calls as complete `PartStart`+`PartEnd` ToolUse when `</tool_call>` closes (parser is block-based); `StreamComplete` carries token counts from the actor.
- Engine ignores `parallel_tool_calls=false` (emits parsed blocks sequentially; document).

## Verification Commands
```bash
cargo test local_engine toolcall render
cargo clippy --all-targets -- -D warnings
PA_LOCAL_GGUF=tmp/models/granite-4.2-3b-Q8_0.gguf cargo test local_engine -- --ignored
```

---

# Phase 03: Profile Editor + Invalidations

## Phase ID
`PLAN-20260903-LOCALMODEL.P03`

## Prerequisites
Phase 02 completed.

## Requirements Implemented

### REQ-LM-007: Profile editor honesty for Local
**Full Text**: Local profiles persist no base_url, no key; model_id is a display label; temperature/max_tokens edit the sampler.
**Behavior**:
- GIVEN: profile editor with ApiType::Local
- WHEN: saved
- THEN: persisted JSON has base_url "", auth None, provider_id "local"; no api.openai.com anywhere.

**Why This Matters**: A local profile must never silently route to OpenAI (analysis risk #1).

## Implementation Tasks

### Files to Modify
- `src/ui_gpui/views/profile_editor_view/mod.rs` — hide Base URL field for Local (keep Model label field); ensure `requires_api_key()` false path hides key UI (verify existing behavior from issue #182 tests).
- `src/services/profile_impl.rs` — `update`/`delete` also call `crate::llm::local::invalidate_local()` next to the existing `open_responses::invalidate_profile` calls (:551, :574).
- `tests/profile_editor_local_tests.rs` (extend existing profile_editor tests) — save produces clean JSON; migration of a legacy "local" profile with api.openai.com base_url is normalized to "". `@requirement:REQ-LM-007`

## Verification Commands
```bash
cargo test profile_editor profile_seeding config_settings
grep -rn "invalidate_local" src/services/profile_impl.rs   # Expected: 2 call sites
```

---

# Phase 04: Settings UI — Local Model Section

## Phase ID
`PLAN-20260903-LOCALMODEL.P04`

## Prerequisites
Phase 03 completed. UI prototype: `dev-docs/mockups/local-model-settings.html`.

## Requirements Implemented

### REQ-LM-006: Settings UI
**Full Text**: Settings gains a LocalModel category with status card, model path field + Choose (text path entry v1), context size, GPU layers, idle toggle + minutes, Unload now button.
**Behavior**:
- GIVEN: settings open on Local Model
- WHEN: rendered
- THEN: status card shows live engine state (poll while visible, Backup pattern); fields show persisted values; Save persists via the P01 service; Unload now calls `local::unload()`.
- GIVEN: model file missing at configured path
- WHEN: status card renders
- THEN: state shows NotLoaded with "model file not found: <path>".

**Why This Matters**: The engine is invisible without an observable surface; config must be editable without editing JSON.

## Implementation Tasks

### Files to Create
- `src/ui_gpui/views/settings_view/render_local_model_panel.rs` — per mockup section 1. `@requirement:REQ-LM-006`
- `src/ui_gpui/views/settings_view/local_model_actions.rs` — field actions, save, unload/load button handlers (Backup actions pattern).

### Files to Modify
- `src/ui_gpui/views/settings_view/types.rs` — `SettingsCategory::LocalModel` + ALL + display_name "Local Model" (between Models and Skills).
- `src/ui_gpui/views/settings_view/mod.rs` — state fields (`local_model_settings`, `local_model_status`, `local_model_error`), ActiveField variants for the new inputs.
- `src/ui_gpui/views/settings_view/command.rs` — `ViewCommand::LocalModelSettingsLoaded`, `LocalModelStatusUpdated`.
- `src/presentation/settings_presenter.rs` — load settings on category activation, poll status, handle save/unload UserEvents.
- `src/events/types.rs` — `UserEvent::SaveLocalModelSettings`, `UnloadLocalModel` (follow existing naming).
- `src/ui_gpui/app_store.rs` — reduce new ViewCommands; `is_store_managed` update.
- `tests/gpui_wiring_local_model_tests.rs` (new) — category enumeration, reducer handles new commands, render smoke (gpui test harness if available; else logic-level tests of state transitions).

## Verification Commands
```bash
cargo test settings gpui_wiring app_store
cargo clippy --all-targets -- -D warnings
```

---

# Phase 05: Lazy Load/Unload Wiring + Full Verification

## Phase ID
`PLAN-20260903-LOCALMODEL.P05`

## Prerequisites
Phases 01-04 completed.

## Requirements Implemented

### REQ-LM-005 + REQ-LM-008: lazy load, idle unload, selector
**Full Text**: The model loads only when a request arrives with a local profile selected in a conversation; unloads after the configured idle timeout, on profile update/delete, and via the Unload button. The conversation selector treats the local profile like any other.
**Behavior**:
- GIVEN: conversation using a remote profile, engine unloaded
- WHEN: user sends
- THEN: nothing loads.
- GIVEN: conversation switched to the local profile
- WHEN: user sends
- THEN: engine loads (first send slower), generation streams.
- GIVEN: engine idle beyond timeout
- WHEN: the actor timer fires
- THEN: model+context drop; memory returns (mmap unmaps).

**Why This Matters**: Default-enabled must not mean always-resident: 4.4 GB is held only while actually used.

## Implementation Tasks

### Files to Modify
- `src/llm/local/engine.rs` — idle timer in actor loop (wake on deadline vs job recv; unload drops `LlamaModel`/`LlamaContext`, keeps thread + backend guard alive for reuse).
- `src/llm/local/mod.rs` — `status()` snapshot for UI.
- `src/ui_gpui/views/chat_view/` — profile dropdown rows gain an engine-status dot for local entries (data from status snapshot in ChatProfilesUpdated payload; optional cosmetic, low risk).
- Full sweep: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib --tests` (entire suite; fix fallout in tests listed in analysis risk #11).

### Manual Verification (run and paste output)
```bash
ls ~/.library/Application\ Support/PersonalAgent/profiles/ 2>/dev/null || echo "(fresh)"
# move profiles dir aside for a true first-install test, then:
cargo run --bin personal_agent_gpui   # popout mode: tray → Open Pop-out (or PA_AUTO_OPEN_POPUP=1)
# Expected: chat usable immediately with "Granite (local)" selected; first send loads model;
# Settings → Local Model shows Loaded + tok/s; Unload now frees memory (check Activity Monitor);
# switching conversation to another profile + idle 5 min unloads automatically.
```

## Success Criteria
- Fresh-install chat works with zero configuration using the local model.
- `ps`/Activity Monitor confirms ~0 resident model memory when unloaded; ~4.4 GiB loaded at n_ctx 8192.
- No socket/process spawned (verify: `lsof -p <pid> | grep -c TCP` shows no new listeners).
- Full suite green; fmt/clippy clean.

## Failure Recovery
Per-phase `git checkout -- <files>`; engine module is additive, seam changes are three call sites.
