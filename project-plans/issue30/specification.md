# Issue 30 Plan (Revised): Model Selector — Position, Performance, and Scroll

## Problem

The provider dropdown on the model selector has three interrelated bugs:

1. **Wrong position** — dropdown uses hardcoded absolute offset that doesn't track the trigger button.
2. **Severe rendering performance** — the models.dev registry contains **4,108 models from 105 providers**. Every state change (click, keypress, scroll tick) triggers a full re-render that creates a `Div` for every model (~24,000 DOM elements). This causes multi-second beach balls.
3. **Broken scroll** — the dropdown overlay is a child of a full-screen backdrop that uses `block_mouse_except_scroll()`. Scroll events pass through the backdrop to the model list behind it. The dropdown can't scroll independently.

## Root Cause

The performance and scroll problems share a root cause: 4,108 model rows are rendered eagerly on every frame. GPUI must layout and paint all of them on the main thread. Combined with the event-blocking model of the backdrop overlay, any user interaction (including scroll) triggers a full re-render of all 24,000+ elements, freezing the app.

## Architectural Decisions

- **Virtual scrolling**: Replace the current div-per-model pattern with GPUI's `uniform_list`, which only renders visible rows (~15–20 at 28px each).
- **Cached derived state**: Pre-compute filtered display rows, provider lists, and search-ready lowercase strings. The render path reads cached data only — never iterates the full model list.
- **Invalidation-on-mutation**: Caches rebuild only when filter inputs or model data actually change, not on every render.
- **Local-only filtering**: All filtering/searching happens in the view state. No async round-trips to the presenter for search or provider filter.
- **Indexed display rows with stale-read guard**: Display rows reference models by `usize` index into the models Vec. Indices are valid only within a single rebuild cycle. `set_models()` atomically rebuilds indices. The render callback uses `.get(ix)` to guard against stale indices between render and callback execution. This avoids the complexity of a `ModelId` newtype while maintaining correctness through the atomic-rebuild invariant.

## Prior Work (Already on Branch)

The following changes are already committed on `issue30` and passing CI:

- **Fix 1 (position)**: Named constants `TOP_BAR_H` / `FILTER_BAR_H` replace hardcoded `80.0 + 28.0` offset. Dropdown now appears at correct position below filter bar.
- **Fix 2a (compute-once)**: `filtered_models()` and `all_providers()` computed once in `render()` and passed to sub-methods (was called 2–3× per render).
- **Fix 2b (remove round-trip)**: `SearchModels` and `FilterModelsByProvider` events removed from view emission. Presenter ignores them (no-ops). All filtering is local.
- **Presenter/view/wiring tests**: Updated to match new local-filtering behavior. All tests pass.

## Remaining Work (This Plan)

### What this plan addresses

1. **Cached state with pre-computed display rows** — eliminate per-render iteration of 4,108 models.
2. **Virtual scrolling via `uniform_list`** — render only visible rows (~15–20) instead of all 4,108.
3. **Scroll isolation** — dropdown scrolls independently; model list doesn't scroll when dropdown is open.
4. **Edge cases from review** — stale-read guard, scroll position reset, stale filter cleanup, empty state, pre-lowercased search.

### What is out of scope

- Keyboard navigation (arrow keys / Enter to select) — noted as future gap.
- Real text input widget for search field — existing IME canvas hack is preserved.
- Provider display names distinct from IDs — currently name == id; preserved as-is.
- Fixing `replace_text_in_range` ignoring the `range` parameter (pre-existing IME limitation, tracked separately).
- Fixing `handle_command()` hardcoding `reasoning: false, vision: false` when converting `ViewCommand::ModelSearchResults` to local `ModelInfo`. The `ViewCommand::ModelInfo` struct lacks reasoning/vision fields, so capability filters are effectively inoperative against production data. This is a pre-existing data pipeline gap — the plan's tests construct `ModelInfo` directly with correct capability values. Fixing the data pipeline is a separate issue.
- End-to-end filter→rebuild→render integration test in GPUI TestAppContext (see Issue #21 in Appendix).

---

## Repository Touchpoints

- `src/ui_gpui/views/model_selector_view/mod.rs` — state struct, caching, `DisplayRow` enum, `rebuild_display_rows()`
- `src/ui_gpui/views/model_selector_view/render.rs` — `uniform_list` integration, scroll handle, backdrop/dropdown overlay
- `src/ui_gpui/views/model_selector_view/command.rs` — call `rebuild_display_rows()` from filter mutations; remove `println!` debug statements
- `src/ui_gpui/views/model_selector_view/ime.rs` — call `rebuild_display_rows()` from IME text changes
- `src/ui_gpui/components/list.rs` — evaluate for consolidation or deprecation (see Phase 1 REFACTOR)
- `tests/` — existing view and presenter tests updated as needed

---

## Subagent Execution Model

### Implementation Lane (Subagent: `rustcoder`)
- Owns RED→GREEN→REFACTOR implementation per phase.
- Produces code, tests, and verification output.
- Runs `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib --tests` before handing back.

### Verification Lane (Subagent: `rustreviewer`)
- Independent review of behavior, architecture fit, and test quality.
- Validates against `dev-docs/goodtests.md` and `dev-docs/RUST-RULES.md`.
- No rubber-stamp reviews; full clean review each pass.

### Orchestrator (Main agent)
- Sequences phases and enforces TDD gates.
- Handles git flow, PR update, CI loops, CodeRabbit remediation.

---

## Phase Dependency Map

Phases are reorganized to eliminate the stale-cache gap identified in the original plan (Issue #27).

**Old order (broken):** P0 (cache) → P1 (virtual scroll) → P2 (dropdown) → P3 (wire rebuild calls)
  — Problem: P1/P2 use old per-render filtering because P3 hasn't wired `rebuild_display_rows()` yet.

**New order (fixed):** P0 (cache + wire) → P1 (virtual scroll) → P2 (dropdown) → P3 (scale + perf) → P4 (integration)

Every phase that introduces a filter mutation also wires the rebuild call immediately. No phase ever has a cache that is out of sync with the filters.

```
P0 (Cached state + rebuild wiring)
 ├── ModelSelectorState: DisplayRow, SearchableModelInfo, rebuild_display_rows()
 ├── command.rs: every filter mutation calls rebuild_display_rows()
 ├── ime.rs: every text mutation calls rebuild_display_rows()
 └── render.rs: reads cached_display_rows only
      ↓
P1 (Virtual scrolling with uniform_list)
 ├── render.rs: replace div-per-model with uniform_list
 ├── UniformListScrollHandle on ModelSelectorView
 ├── Empty state conditional render
 └── Remove components/list.rs (dead code)
      ↓
P2 (Dropdown scroll isolation)
 ├── Backdrop + dropdown as siblings
 ├── Backdrop z-index below top bar
 └── on_scroll_wheel(stop_propagation) on dropdown
      ↓
P3 (Scale testing + performance benchmarks)
 ├── 4K-model scale test
 └── Performance benchmark
      ↓
P4 (Integration verification + PR)
```

---

## GPUI API Reference: `uniform_list`

This section documents the exact GPUI APIs this plan uses. All signatures are verified against the project's GPUI checkout at `c67328a`.

### `uniform_list` function

```rust
// src: crates/gpui/src/elements/uniform_list.rs:22
pub fn uniform_list<R>(
    id: impl Into<ElementId>,
    item_count: usize,
    f: impl 'static + Fn(Range<usize>, &mut Window, &mut App) -> Vec<R>,
) -> UniformList
where
    R: IntoElement
```

- Only renders visible items. Measures the first item's height and assumes all items are the same height.
- **Hardcodes `overflow.y = Overflow::Scroll`** in the base style constructor (line 34). This cannot be suppressed. Scroll isolation for the model list when the dropdown is open is achieved by not rendering the `uniform_list` at all — NOT by toggling `overflow_hidden` on it.
- Callback runs in `App` context, NOT `Context<Self>`. Use `cx.processor()` to bridge into `Context<Self>` — this provides `cx.listener()` and direct `&mut Self` access inside the callback. See `cx.processor()` API reference below.

### `UniformListScrollHandle`

```rust
// line 79: derives Clone, Debug, Default
#[derive(Clone, Debug, Default)]
pub struct UniformListScrollHandle(pub Rc<RefCell<UniformListScrollState>>);
```

- **Derives `Clone`** — no compatibility concern. Store on `ModelSelectorView`, pass to `track_scroll()` by reference.

### `scroll_to_item`

```rust
// line 150
pub fn scroll_to_item(&self, ix: usize, strategy: ScrollStrategy)
```

- **TWO arguments**: `(usize, ScrollStrategy)`. The original plan's `scroll_to_item(0)` is a compile error. Correct call: `scroll_to_item(0, ScrollStrategy::Top)`.
- `ScrollStrategy` is defined in the same module (`gpui::elements::uniform_list::ScrollStrategy`), re-exported at `gpui::ScrollStrategy`. Variants: `Top`, `Center`, `Bottom`, `Nearest`.

### `cx.processor()` — Context bridge for `uniform_list` callbacks

```rust
// src: crates/gpui/src/app/context.rs
pub fn processor<E, R>(
    &self,
    f: impl Fn(&mut T, E, &mut Window, &mut Context<T>) -> R + 'static,
) -> impl Fn(E, &mut Window, &mut App) -> R + 'static
```

- Bridges `Context<Self>` into the `Fn(E, &mut Window, &mut App) -> R` signature that `uniform_list` requires.
- Inside the wrapped callback, `this` is `&mut Self` and `cx` is `&mut Context<Self>` — `cx.listener()` and all normal view methods are available.
- No manual `entity.read(cx)` or `entity.clone()` needed.
- This is the idiomatic GPUI pattern for `uniform_list` callbacks.

### `track_scroll`

```rust
// line 671
pub fn track_scroll(mut self, handle: &UniformListScrollHandle) -> Self
```

- Takes `&UniformListScrollHandle`, not owned. Called on the `UniformList` builder, not on the handle.

### Critical constraints

1. **All rows MUST be pixel-identical in height** — use explicit `h(px(28.0))` on every row, not intrinsic sizing. Provider headers were 24px, model rows were 28px. Both must become 28px. This is a deliberate visual change documented below.
2. **Stale-read guard** — `item_count` is captured at render time but callback reads live state. Use `.get(ix)` with `filter_map` to handle out-of-bounds gracefully.
3. **Scroll reset** — call `scroll_handle.scroll_to_item(0, ScrollStrategy::Top)` when filter changes reduce the list, otherwise `uniform_list` may be scrolled past the end.
4. **Zero items** — `uniform_list` with `item_count = 0` creates a scrollable empty container with no visual content. Always guard with an empty-state check and render a static placeholder instead.

### Visual changes

1. **Provider header height 24px → 28px:** Provider headers currently render at `h(px(24.0))`. `uniform_list` requires all rows to have identical height. Both headers and model rows will use `h(px(28.0))`. The 4px increase is a deliberate visual change. Headers will be visually differentiated by bold text and background color, not by height.

2. **Intra-provider model sort order:** Models within each provider group are now sorted alphabetically by model ID. The current code renders models in API-insertion order. This change ensures deterministic output for `uniform_list` display and testing. The visual difference is minor — models from the same provider will now appear alphabetically instead of in arbitrary order.


---

## TDD Phase Plan

## Phase 0 — Cached State Foundation + Rebuild Wiring

This phase builds the cached state AND wires every filter mutation to call `rebuild_display_rows()`. This eliminates the stale-cache gap from the original plan where P0 created the cache but P3 wired it.

### RED

Write failing tests in `mod.rs` (unit tests on `ModelSelectorState`) and in `command.rs`/`ime.rs` test modules:

#### Unit tests on `ModelSelectorState`

1. **`rebuild_display_rows_produces_correct_flat_list`** — Given 6 models across 3 providers (e.g., "alpha", "beta", "gamma"), after `rebuild_display_rows()`, verify `cached_display_rows` contains `ProviderHeader("alpha")`, `Model(0)`, `Model(1)`, `ProviderHeader("beta")`, `Model(2)`, `Model(3)`, `ProviderHeader("gamma")`, `Model(4)`, `Model(5)` — in that order. Provider order is alphabetical, matching `cached_providers` sort order.

2. **`rebuild_display_rows_applies_all_filters`** — Set provider filter + search query + reasoning filter. Verify only matching models appear in `cached_display_rows`. Verify `cached_display_rows.len()` reflects header + model count. Count assertions: if 1 provider has 1 matching model, that's 2 entries (1 header + 1 model).

3. **`load_models_builds_provider_cache_and_display_rows`** — Call `state.load_models()` with test data. Verify `cached_providers` is sorted lexicographically and deduped. Verify `cached_display_rows` is populated. Verify `searchable_models` is populated with correct lowercase fields.

4. **`load_models_preserves_valid_provider_filter`** — Set `selected_provider = Some("beta")`, call `state.load_models()` with data containing "beta". Verify `selected_provider` remains `Some("beta")` and cache reflects it.

5. **`load_models_clears_stale_provider_filter`** — Set `selected_provider = Some("old-provider")`, call `state.load_models()` with data that doesn't contain "old-provider". Verify `selected_provider` is cleared to `None`.

6. **`search_uses_pre_lowercased_fields`** — Load models with mixed-case IDs like `"CLAUDE-3.5-Sonnet"`, `"openai-GPT-4o"`. Set search query `"claude"`. Verify `cached_display_rows` contains the correct model (proving that case-insensitive search works via the pre-lowered fields in `SearchableModelInfo`, not per-model `to_lowercase()` at query time).

7. **`empty_models_produces_empty_display_rows`** — Verify zero models produces empty `cached_display_rows`, zero provider count.

8. **`display_rows_always_sorted_alphabetically_by_provider`** — Build models where providers arrive in non-alphabetical order (e.g., "gamma", "alpha", "beta"). Verify `cached_display_rows` lists providers in deterministic alphabetical order ("alpha", "beta", "gamma") regardless of insertion order.

9. **`load_models_twice_fully_replaces_cached_state`** — Call `load_models()` with dataset A (3 providers, 10 models). Verify cache matches A. Call `load_models()` again with dataset B (2 providers, 5 models, completely different IDs and providers). Verify `cached_providers`, `cached_display_rows`, `searchable_models` all fully reflect dataset B with no remnants of dataset A. Verify `cached_model_count` and `cached_provider_count` match dataset B.

10. **`rebuild_from_uninitialized_state_produces_empty_rows`** — Create a default `ModelSelectorState` (no `load_models()` called). Call `rebuild_display_rows()`. Verify `cached_display_rows` is empty, counts are zero, no panic.

#### Command/IME wiring tests (GPUI test context — require `cx.add_empty_window()`)

These tests operate on `ModelSelectorView` which requires `Context<Self>` and a GPUI window. Use the existing pattern: `let mut visual_cx = cx.add_empty_window().clone();` then `visual_cx.update(|window, app| { ... })`.

11. **`backspace_rebuilds_display_rows`** — Create a `ModelSelectorView` with models loaded. Set search query "claude". Call `handle_key_down` with backspace. Verify `cached_display_rows` reflects the truncated query "claud" (not stale "claude" results).

12. **`toggle_reasoning_filter_rebuilds_display_rows`** — Load models with and without reasoning. Call `toggle_reasoning_filter()`. Verify `cached_display_rows` is rebuilt (reasoning models only).

13. **`toggle_vision_filter_rebuilds_display_rows`** — Same pattern as above for vision.

14. **`select_provider_filter_rebuilds_display_rows`** — Load multi-provider models. Call `select_provider_filter("anthropic")`. Verify `cached_display_rows` contains only that provider's header + models.

15. **`clear_provider_filter_rebuilds_display_rows`** — Set provider filter, then call `clear_provider_filter()`. Verify all providers appear in `cached_display_rows`.

16. **`ime_replace_text_rebuilds_display_rows`** — Simulate `replace_text_in_range(None, "cla")`. Verify `cached_display_rows` reflects the filtered results.

17. **`ime_replace_and_mark_text_rebuilds_display_rows`** — Simulate `replace_and_mark_text_in_range(None, "d", None)`. Verify `cached_display_rows` is rebuilt.

18. **`handle_command_populates_cache`** (GPUI test context) — Call `handle_command(ViewCommand::ModelSearchResults { models: [...] })` with 3 models across 2 providers. Verify `cached_display_rows` is populated (2 headers + 3 models = 5 entries), `cached_providers` has 2 entries, and `searchable_models` has 3 entries. This tests the primary production code path: data arrives from presenter → `handle_command()` → `set_models()` → `load_models()` → cache populated.

### GREEN

Implement in `mod.rs`:

- Add `DisplayRow` enum:
  ```rust
  #[derive(Clone, Debug)]
  pub(super) enum DisplayRow {
      ProviderHeader(String),
      Model(usize), // index into self.models — stable only within a single rebuild cycle
  }
  ```

  **Index stability invariant (Issue #29):** `Model(usize)` indices are valid only for the current `cached_display_rows` contents. They are rebuilt atomically in `rebuild_display_rows()`. The render callback uses `rows.get(ix)` with a stale-read guard and never caches indices across frames. `set_models()` always calls `rebuild_display_rows()` to re-sync. This is documented with a correctness invariant comment on the `DisplayRow::Model` variant.

- Add `SearchableModelInfo` struct (concrete decision: it is a wrapper, not modifying `ModelInfo`):
  ```rust
  #[derive(Clone, Debug)]
  pub(super) struct SearchableModelInfo {
      pub info: ModelInfo,
      pub id_lower: String,
      pub provider_lower: String,
  }
  ```

  `ModelInfo` remains untouched. `SearchableModelInfo` wraps it with pre-lowered fields. This avoids modifying the public `ModelInfo` API.

- Add cached fields to `ModelSelectorState`:
  ```rust
  pub(super) searchable_models: Vec<SearchableModelInfo>,  // parallel to self.models — invariant: searchable_models[i] ↔ models[i]
  pub(super) cached_providers: Vec<String>,                  // sorted, deduped provider IDs
  pub(super) cached_display_rows: Vec<DisplayRow>,           // pre-computed flattened display list
  pub(super) cached_model_count: usize,                      // model rows in cached_display_rows (updated in rebuild)
  pub(super) cached_provider_count: usize,                   // provider headers in cached_display_rows (updated in rebuild)
  ```

  New cached fields use `pub(super)` to restrict mutation to the model_selector_view module. Pre-existing fields (`models`, `providers`, `search_query`, etc.) remain `pub` — changing their visibility is out of scope for this issue but noted as tech debt.

  **Dead field:** After this change, `providers: Vec<ProviderInfo>` is populated by `load_models()` but never read for rendering or filtering (all display logic uses `cached_providers: Vec<String>`). The field is preserved for backward compatibility with existing callers but should be marked with a `// NOTE: unused for rendering — see cached_providers` comment.

  **Cache invalidation contract:** Direct mutation of `search_query`, `selected_provider`, `filter_reasoning`, or `filter_vision` bypasses the cache. All filter mutations MUST go through the wired methods (`toggle_reasoning_filter()`, `select_provider_filter()`, etc.) which call `rebuild_and_reset_scroll()`. Existing test helpers like `set_search_query()` / `set_selected_provider()` are used only in test setup — tests that need to verify cached state after direct field mutation must call `state.rebuild_display_rows()` explicitly. This is a known constraint of keeping pre-existing `pub` field visibility.

  **Cached counts** (Issue #7): `cached_model_count` and `cached_provider_count` are computed in `rebuild_display_rows()` as a single pass over the result, then stored. This avoids both drift risk (updated atomically with the rows) and per-render iteration overhead. Accessors:
  ```rust
  pub(super) fn cached_filtered_model_count(&self) -> usize { self.cached_model_count }
  pub(super) fn cached_visible_provider_count(&self) -> usize { self.cached_provider_count }
  ```

- Implement `rebuild_display_rows(&mut self)`:
  - Single pass: iterate `cached_providers` (sorted Vec, NOT HashMap keys — this addresses Issue #4).
  - For each provider, filter its models from `searchable_models` using:
    - Provider match (`selected_provider`)
    - Search match (compare `id_lower` / `provider_lower` against `self.search_query.to_lowercase()`, computed once into a local at the top of the function — Issue #6)
    - Reasoning / vision capability match
  - Emit `ProviderHeader(provider_name)` then `Model(idx)` for each matching model.
  - Sort models within each provider group by model ID for deterministic output. (Note: this is a visual behavior change from the current insertion-order display. Documented in "Visual changes" below.)
  - The query is lowered once: `let query_lower = self.search_query.to_lowercase();` at the top of the function. Models use pre-lowered `id_lower` / `provider_lower`. Zero `to_lowercase()` per model.

- Add `ModelSelectorState::load_models()` (new method on State, called by `ModelSelectorView::set_models()`):
  - Takes `providers: Vec<ProviderInfo>, models: Vec<ModelInfo>`.
  - Stores `self.providers = providers; self.models = models;`.
  - Build `searchable_models` by zipping `models` with their pre-lowered fields.
  - Build `cached_providers` (sorted, deduped from model provider IDs).
  - Clear `selected_provider` only if it is not present in the new `cached_providers` (Issue #5: preserve valid filter).
  - Call `rebuild_display_rows()`.
  This separation puts all cache-building logic on `ModelSelectorState` for testability — P0 unit tests can call `state.load_models(...)` directly without needing a `ModelSelectorView`.

  **Parallel-index invariant:** `searchable_models[i]` always corresponds to `models[i]`. Both are built atomically in `load_models()` from the same input. `rebuild_display_rows()` uses `searchable_models` for filtering and `DisplayRow::Model(idx)` indexes into `models` for rendering. The invariant is documented with a comment on the `searchable_models` field and enforced by the fact that only `load_models()` modifies either Vec.

  **Refactored `ModelSelectorView::set_models()` (updated in P0, NOT deferred to P1):** The existing public API must delegate to `state.load_models()` immediately in P0 to avoid stale cache. In P0, the implementation is:
  ```rust
  pub fn set_models(&mut self, providers: Vec<ProviderInfo>, models: Vec<ModelInfo>) {
      self.state.load_models(providers, models);
      // Scroll reset is a no-op in P0 (handle is Default, no uniform_list yet).
      // P1 adds conditional scroll logic here.
      self.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
  }
  ```
  This ensures that every call to `set_models()` — from `handle_command()`, tests, or elsewhere — populates the cache correctly from the start. P1 refines the scroll logic to be conditional (first load or empty result only). Existing callers (`handle_command()`, tests) continue to use `view.set_models()` unchanged.

- Update `render_status_bar()` to use `self.state.cached_model_count()` and `self.state.cached_provider_count()` instead of the current per-render `filtered_models().len()` / `all_providers().len()`. This is the primary consumer of these counts.

- Maintain backward compatibility: `filtered_models()` and `all_providers()` now delegate to the cache. These are kept for backward compatibility with existing tests; they are NOT on the render hot path after `render_status_bar()` is updated. Note: the returned `&ModelInfo` references borrow `self.models` — valid only as long as `self.models` is unchanged. The atomic rebuild invariant (`load_models()` always calls `rebuild_display_rows()`) ensures indices stay in sync. **Return-order change:** `filtered_models()` now returns models in alphabetical-by-provider order (matching `cached_display_rows`), not in insertion order. Affected tests that must be reviewed for order assumptions:
  - `model_info_formatting_and_state_filters_work` (mod.rs ~L281) — uses 2 models from "anthropic" and "openai"; alphabetical order matches insertion order, so no change needed.
  - `provider_dropdown_selection_and_model_emission_follow_real_filter_rules` (mod.rs ~L446) — asserts `filtered_models()[0].id == "claude-3-7-sonnet"` after provider filter; within a single-provider filter result, models are sorted alphabetically by ID, so "claude-3-7-sonnet" is first. No change needed.
  - `handle_command_maps_models_and_filter_events_emit_only_on_changes` (mod.rs ~L331) — asserts `models[0].id == "claude-3-5-sonnet"`; this is insertion order from `set_models()`. Since `models` Vec itself is not reordered (only `cached_display_rows` is), `state.models[0]` remains "claude-3-5-sonnet". No change needed.

  Update any tests that depend on insertion order:
  ```rust
  pub fn filtered_models(&self) -> Vec<&ModelInfo> {
      self.cached_display_rows.iter()
          .filter_map(|row| match row {
              DisplayRow::Model(idx) => self.models.get(*idx),
              DisplayRow::ProviderHeader(_) => None,
          })
          .collect()
  }
  pub fn all_providers(&self) -> Vec<&str> {
      self.cached_providers.iter().map(|s| s.as_str()).collect()
  }
  ```

Implement in `command.rs`:

- Add `scroll_handle: UniformListScrollHandle` to `ModelSelectorView` (initialized as `Default::default()` in `new()`). This is added in P0 so the `rebuild_and_reset_scroll()` helper works immediately. The `uniform_list` integration that reads the handle comes in P1.

- Add `rebuild_and_reset_scroll()` helper on `ModelSelectorView` (see P1 GREEN section for implementation).

- Every mutation to a filter input calls `self.rebuild_and_reset_scroll(cx)`:
  - `toggle_reasoning_filter()` — set flag, then `self.rebuild_and_reset_scroll(cx)`.
  - `toggle_vision_filter()` — same pattern.
  - `clear_provider_filter()` — clear provider, close dropdown, `self.rebuild_and_reset_scroll(cx)`.
  - `select_provider_filter()` — set provider, close dropdown, `self.rebuild_and_reset_scroll(cx)`.
  - `handle_key_down()` backspace — pop char, `self.rebuild_and_reset_scroll(cx)`.
  - `handle_command()` `ModelSearchResults` — calls `set_models()` which internally rebuilds (separate scroll logic — see P1 GREEN).
  - `toggle_provider_dropdown()` — no rebuild needed (display toggle only).
- **Remove all `println!` debug statements** from `command.rs` (lines 10, 29, 44, 46, 92). Replace with `tracing::debug!()` if logging is needed, or remove entirely.

Implement in `ime.rs`:

- `replace_text_in_range()` — after mutating `search_query`, call `self.state.rebuild_display_rows()`, then `cx.notify()`.
- `replace_and_mark_text_in_range()` — same pattern: mutate, rebuild, notify.
- `unmark_text()` — NO rebuild needed. Add a code comment in `ime.rs` on `unmark_text()` explaining: "Called by the IME system when the user confirms a composition candidate. Only clears the marked-byte counter; no search query mutation occurs, so no cache rebuild is needed."

### REFACTOR

- Remove the old `filtered_models()` full-iteration implementation if it is now purely cache-backed.
- Remove the old `all_providers()` full-iteration implementation.
- Verify no call site directly iterates `self.models` for display purposes (only `set_models()` and `rebuild_display_rows()` should touch the full model list).

### Accessibility (future enhancement)

GPUI is a GPU-native framework and may not expose standard `role` or `aria-*` attributes. Virtual scrolling inherently hides off-screen items from accessibility tools. For now:
- Every `DisplayRow::Model` row gets a descriptive `id` attribute including provider and model name (e.g., `"model-anthropic-claude-3-5-sonnet"`). This is already the existing pattern.
- Every `DisplayRow::ProviderHeader` gets a descriptive `id` attribute (e.g., `"provider-header-anthropic"`). Existing pattern.
- Document a TODO in code comments noting that proper accessibility support (screen reader labels, heading semantics) should be added when/if GPUI exposes accessibility APIs.
- This is not a regression — the existing eager-render code has the same accessibility limitations.


---

## Phase 1 — Virtual Scrolling with `uniform_list`

### RED

Write failing tests (unit tests on state + GPUI test context tests):

1. **`cached_display_row_count_matches_expected_uniform_list_count`** — After setting 6 models across 3 providers with no filters, verify `cached_display_rows.len()` == 9 (3 headers + 6 models). This is the exact value passed as `item_count` to `uniform_list`. No separate row-count test needed; this is it.

2. **`stale_read_guard_handles_out_of_bounds`** — Simulate the scenario: `item_count` is captured at 9, but between render and callback, a filter reduces `cached_display_rows` to 3. Build state, rebuild to 9 rows, then rebuild to 3 rows. Walk the indexing logic `(0..9).filter_map(|ix| rows.get(ix))` and verify it returns exactly 3 valid elements, no panic, no zero-height stubs. *Acknowledged limitation:* This tests the indexing pattern in isolation, not inside a real `uniform_list` render. The true GPUI integration is verified by manual testing and by the scale test in P3.

3. **`scroll_position_resets_on_filter_change`** — Load models, set scroll handle offset to non-zero (via `scroll_to_item(5, ScrollStrategy::Top)`). Apply a filter that reduces rows. Call the rebuild + scroll reset sequence. Verify `scroll_handle` state reflects a reset (the `logical_scroll_top_index()` should be 0). Note: `logical_scroll_top_index()` requires the GPUI `test-support` feature (gated with `#[cfg(any(test, feature = "test-support"))]` at uniform_list.rs:222). The project's Cargo.toml already enables this feature. Without a render pass, this verifies the *deferred scroll intent* is set to 0 — actual scroll application requires a full GPUI layout pass. *Acknowledged limitation:* This is structural evidence per RUST-RULES.md — it proves the deferred state is set correctly, not that the user sees a scroll change.

4. **`scroll_to_item_before_first_render`** — Create a fresh `UniformListScrollHandle::default()`. Call `scroll_to_item(0, ScrollStrategy::Top)` immediately. Verify no panic and that `deferred_scroll_to_item` is set. This tests the "before first render" edge case.

5. **`cx_processor_basic_smoke_test`** — Prerequisite test: create a minimal `#[gpui::test]` that uses `cx.processor()` with `uniform_list` to render a 3-item list. Verify the callback receives `&mut Self` and can access state. This validates that `cx.processor()` works with `uniform_list` before building the full integration. If this fails, the `cx.processor()` approach is invalid and the plan must fall back to `entity.read(cx)` + clone.

6. **`empty_state_shows_no_matching_models`** — When `cached_display_rows` is empty, verify that `cached_display_rows.len() == 0` triggers the empty-state code path. This is verified by asserting state emptiness and confirming the render path branches on it. *Acknowledged limitation:* Verifying the actual rendered element ID (`"model-list-empty"` vs `"model-list"`) is structural — it's the best achievable in TestAppContext without element-tree inspection, and serves as a regression guard.

### GREEN

Implement in `render.rs`:

- Add imports:
  ```rust
  use gpui::{uniform_list, UniformListScrollHandle, ScrollStrategy};
  ```

- Add `scroll_handle` field to `ModelSelectorView`:
  ```rust
  pub(super) scroll_handle: UniformListScrollHandle,
  ```
  Initialize in `ModelSelectorView::new()` as `UniformListScrollHandle::default()`.

- **Scroll reset on rebuild** (Issue #2 — `rebuild_display_rows()` is on `State`, scroll handle is on `View`):
  `rebuild_display_rows()` lives on `ModelSelectorState`, which does NOT own the scroll handle. The scroll reset is performed by the caller. Filter mutations always reset scroll to top, because changing what's displayed invalidates the user's scroll context:
  ```rust
  // In command.rs / ime.rs — helper method on ModelSelectorView:
  fn rebuild_and_reset_scroll(&mut self, cx: &mut gpui::Context<Self>) {
      self.state.rebuild_display_rows();
      self.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
      cx.notify();
  }
  ```
  Every filter mutation (search, provider, capability toggle) calls `rebuild_and_reset_scroll()`. This is introduced in **P0** so all mutation sites are wired from the start — no phase gap. The `UniformListScrollHandle` field is added to `ModelSelectorView` in P0 GREEN (it's `Default`, so `scroll_to_item` on it before first render just sets `deferred_scroll_to_item` which is harmless). The `uniform_list` integration that actually uses the handle comes in P1.

  `set_models()` (background data refresh) only resets scroll when the model list actually changed:
  ```rust
  pub fn set_models(&mut self, providers: Vec<ProviderInfo>, models: Vec<ModelInfo>) {
      let had_models = !self.state.models.is_empty();
      self.state.load_models(providers, models);
      if !had_models || self.state.models.is_empty() {
          // First load or empty result — reset to top
          self.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
      }
      // Background refresh with same data → preserve scroll position
  }
  ```

- Replace `render_model_list()` using `cx.processor()` — the idiomatic GPUI pattern for `uniform_list` callbacks:
  ```rust
  fn render_model_list(&self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
      let row_count = self.state.cached_display_rows.len();

      if row_count == 0 {
          return div()
              .id("model-list-empty")
              .flex_1()
              .w_full()
              .bg(Theme::bg_darkest())
              .flex()
              .items_center()
              .justify_center()
              .child(
                  div()
                      .text_size(px(12.0))
                      .text_color(Theme::text_muted())
                      .child("No matching models")
              )
              .into_any_element();
      }

      uniform_list("model-list", row_count, cx.processor(
          // Note: `list_cx` is the Context<Self> provided by processor(), distinct
          // from the outer `cx` in render_model_list(). Do NOT rename to `cx` —
          // shadowing would compile but confuse readers.
          |this: &mut ModelSelectorView, range: Range<usize>, _window, list_cx| {
              range
                  .filter_map(|ix| {
                      let row = this.state.cached_display_rows.get(ix)?;
                      match row {
                          DisplayRow::ProviderHeader(name) => {
                              Some(Self::render_provider_header_uniform(name)
                                  .into_any_element())
                          }
                          DisplayRow::Model(idx) => {
                              // Stale-index guard: skip if models Vec was replaced
                              // between item_count capture and callback execution.
                              // Returning None here means uniform_list gets fewer
                              // elements than expected for this range — it handles
                              // this gracefully by rendering fewer rows.
                              let model = this.state.models.get(*idx)?;
                              Some(Self::render_model_row_uniform(model, list_cx)
                                  .into_any_element())
                          }
                      }
                  })
                  .collect()
          },
      ))
      .track_scroll(&self.scroll_handle)
      .flex_1()
      .w_full()
      .into_any_element()
  }
  ```

  **Why `cx.processor()` (Issue #10):** `cx.processor()` is the idiomatic GPUI bridge between `Context<Self>` and the `Fn(E, &mut Window, &mut App)` callback that `uniform_list` requires. It wraps the closure so the callback receives `(&mut Self, Range<usize>, &mut Window, &mut Context<Self>)`. **Note:** This is a novel pattern in this codebase — the project currently uses `cx.listener()` (9 call sites in render.rs) but has zero `cx.processor()` usage. The implementer should write a minimal standalone test verifying `cx.processor()` works with `uniform_list` before building the full integration. Benefits:
  - **No per-frame data cloning** — borrows `this.state.cached_display_rows` and `this.state.models` directly. Zero heap allocation for the row data.
  - **`cx.listener()` available** — click handlers use `cx.listener()` instead of manual `entity.update()`.
  - **All return arms use `.into_any_element()`** — ensures a single concrete return type (`AnyElement`) for every match branch, which `uniform_list` requires (`Vec<R>` for a single `R: IntoElement`).
  - **Stale-read guard** — `.get(ix)` with `filter_map` handles out-of-bounds indices gracefully.

- `render_provider_header_uniform()` — `h(px(28.0))` (up from 24px), bold text, background color for visual differentiation. Static method (no `cx` needed — no event handlers on headers).

- `render_model_row_uniform()` — `h(px(28.0))`, uses `cx.listener()` for click handler:
  ```rust
  fn render_model_row_uniform(
      model: &ModelInfo,
      cx: &mut gpui::Context<Self>,
  ) -> impl IntoElement {
      let model_id = model.id.clone();
      let provider_id = model.provider_id.clone();
      // ... display fields ...

      div()
          .id(SharedString::from(format!("model-{provider_id}-{model_id}")))
          .h(px(28.0))
          // ... layout ...
          .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _window, _cx| {
              this.select_model(provider_id.clone(), model_id.clone());
          }))
  }
  ```

  Note: Because `cx.processor()` bridges the `uniform_list` callback into `Context<Self>`, `cx.listener()` works normally inside `render_model_row_uniform`. This is the standard GPUI event handler pattern — no manual `entity.update()` needed.

- **Empty state conditional render (Issue #13):** When `cached_display_rows.is_empty()`, render a static "No matching models" message instead of `uniform_list(..., 0, ...)`. The conditional check happens in `render_model_list()` before calling `uniform_list`. `uniform_list` with `item_count = 0` creates a scrollable empty container — always guard with an empty-state check.

- **`uniform_list` hardcodes `overflow.y = Scroll` (Issue #11):** This is a GPUI constraint. When the dropdown is open, scroll isolation is achieved by NOT rendering the `uniform_list` at all (render an empty div in its place). The Phase 2 dropdown isolation handles this by conditionally hiding the model list. Do NOT attempt to suppress `overflow.y` on `uniform_list`.

### REFACTOR

- Remove old `render_model_list()`, `render_model_row()`, `render_provider_header()` that used eager rendering.
- Remove the `filtered` / `providers` parameters from render method signatures since all data comes from cached state.
- **Remove `src/ui_gpui/components/list.rs`** (Issue #23): This component renders all items eagerly with no virtual scrolling. It is not used by the model selector and has no virtual scrolling capability. After introducing `uniform_list` for the model selector, `list.rs` is dead code. Verify no code references it across the entire project: `grep -r "components::list\|components/list\|mod list" src/ tests/ --include="*.rs"`. If it is referenced, deprecation-path it with a `#[deprecated]` attribute and a TODO pointing to `uniform_list`. Check `src/ui_gpui/components/mod.rs` for the `pub mod list;` declaration and remove it.
- Remove any remaining `println!` debug statements found during refactoring.


---

## Phase 2 — Dropdown Scroll Isolation

### RED

Write tests for dropdown overlay behavior:

1. **`backdrop_click_closes_dropdown`** — Toggle dropdown open, then directly call the close-dropdown logic (simulating what the backdrop's `on_mouse_down` handler does). Verify `show_provider_dropdown` is `false`. *Acknowledged limitation:* GPUI TestAppContext dispatches mouse events by coordinates, not element ID. This test verifies the behavioral outcome (dropdown closes) rather than full event wiring. The event wiring is verified by manual testing.

2. **`cancel_clickable_while_dropdown_open`** — Open the provider dropdown, then call the cancel/navigate-back handler (the same handler wired to the Cancel button's `on_mouse_down`). Verify navigation is requested to the Settings view. This proves the cancel path works while the dropdown is open. *Note:* This does not prove the backdrop doesn't cover the Cancel button (that requires coordinate hit-testing unavailable in TestAppContext), but it does prove the cancel handler is reachable. The backdrop's `top(px(TOP_BAR_H))` positioning is verified by code review and manual testing.

3. **`model_list_replaced_with_placeholder_when_dropdown_open`** — When the dropdown is open, the `uniform_list` is NOT rendered. Instead, a static placeholder div is rendered. Test by: load models, open dropdown, call the render path, verify `cached_display_rows` is non-empty but the model list element ID is `"model-list-hidden"` (the placeholder), not `"model-list"` (the `uniform_list`). This is the primary scroll isolation mechanism — with no scrollable model list in the tree, scroll events have nothing to reach.

4. **`dropdown_provider_selection_works_when_open`** — Open dropdown, simulate selecting a provider from the dropdown options, verify `selected_provider` is set and dropdown closes. This verifies the dropdown is interactive (z-order is correct — dropdown renders above backdrop).

### GREEN

Implement dropdown overlay in `render.rs`:

- **Backdrop positioning (Issue #17):** The current code has `top(px(0.0))` which covers the top bar and makes Cancel unclickable when the dropdown is open. Change to `top(px(TOP_BAR_H))` to leave the top bar accessible (matching the `mcp_add_view` pattern at `render.rs:591`):
  ```rust
  div()
      .id("provider-menu-backdrop")
      .absolute()
      .top(px(TOP_BAR_H))  // NOT px(0.0) — top bar must remain clickable
      .left(px(0.0))
      .right(px(0.0))
      .bottom(px(0.0))
      .block_mouse_except_scroll()
      .on_scroll_wheel(cx.listener(|_this, _event: &ScrollWheelEvent, _window, cx| {
          cx.stop_propagation();
      }))
      .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
          this.state.show_provider_dropdown = false;
          cx.notify();
      }))
  ```

  Note: `on_scroll_wheel` on the backdrop consumes scroll events that pass through `block_mouse_except_scroll()`. This matches the established pattern in `mcp_add_view/render.rs`. Combined with the model list placeholder (below), this provides belt-and-suspenders scroll isolation.

- **Scroll event handling (Issue #16):** Two-layer isolation:
  1. The model list is NOT rendered when the dropdown is open — a static placeholder div replaces it. No scrollable target exists.
  2. The backdrop consumes any stray scroll events via `on_scroll_wheel(stop_propagation)`.
  3. The dropdown menu has its own `on_scroll_wheel` to consume events within its bounds.

  ```rust
  // Dropdown menu — positioned below filter bar (trigger button location)
  div()
      .id("provider-menu-overlay")
      .absolute()
      .top(px(TOP_BAR_H + FILTER_BAR_H))  // directly below filter bar where trigger lives
      .right(px(12.0))
      .min_w(px(180.0))
      .max_w(px(320.0))
      .max_h(px(300.0))
      .overflow_y_scroll()
      .on_scroll_wheel(cx.listener(|_this, _event: &ScrollWheelEvent, _window, cx| {
          cx.stop_propagation();
      }))
      // ... rest of dropdown content
  ```

- **Model list hidden when dropdown open:** When `show_provider_dropdown` is true, render a static div placeholder instead of the `uniform_list`:
  ```rust
  if self.state.show_provider_dropdown {
      // Static placeholder — uniform_list hardcodes overflow.y=Scroll,
      // so we can't use it while the dropdown is open.
      div().id("model-list-hidden").flex_1().w_full().bg(Theme::bg_darkest())
  } else {
      self.render_model_list_impl(cx)  // the uniform_list version
  }
  ```

- Provider dropdown reads from `cached_providers`.
- Dropdown items use `cx.listener()` (they are rendered in `render()` context, not inside `uniform_list` callback).
- **IMPORTANT: Do NOT copy `mcp_add_view` overlay structure.** The `mcp_add_view/render.rs` pattern renders the dropdown overlay as a **child** of the backdrop div. This means scroll events on the dropdown bubble to the backdrop's `on_scroll_wheel` handler. In the model selector, backdrop and dropdown are **siblings** (two separate `.child()` calls on root). This is intentional to prevent scroll event bubbling and to allow the dropdown to handle its own scroll independently.
- **GPUI sibling z-order assumption:** Later siblings render on top of earlier siblings. The backdrop is rendered first, then the dropdown menu. GPUI dispatches mouse and scroll events to the topmost element at the event coordinates. This means clicks on dropdown items reach the dropdown (on top), not the backdrop (behind). This assumption should be verified empirically during P2 implementation.
- **Focus/IME preservation:** The `focus_handle` and IME canvas are attached to the root div, not the model list. Replacing the model list with a placeholder does not affect keyboard input. The search field continues to work via the IME `EntityInputHandler` implementation, and backspace/Escape key handling continues via the root `on_key_down`. This is verified by existing tests (e.g., `key_handling_closes_dropdown_navigates_and_backspaces_search_once`) which already test keyboard interaction while the dropdown is open.

### REFACTOR

- Remove the old `overflow_hidden` toggle approach from `render_model_list()` — it doesn't work with `uniform_list`.
- Ensure clean separation: backdrop handles dismiss, dropdown handles selection + scroll.


---

## Phase 3 — Scale Testing + Performance Benchmarks

This phase validates that the cached state + virtual scrolling solution actually solves the original performance problem at production scale.

### RED

1. **`scale_test_4k_models_rebuild_display_rows`** — Create `ModelSelectorState` with 4,108 models across 105 providers (matching models.dev registry size). Call `set_models()`. Verify:
   - `cached_providers.len()` == 105.
   - `cached_display_rows` contains 105 provider headers + 4,108 model entries = 4,213 total.
   - `rebuild_display_rows()` completes without stack overflow or excessive allocation.
   - After setting a search query that matches 10 models, `cached_display_rows.len()` <= 10 + number_of_matching_provider_headers.

2. **`scale_test_4k_models_filter_performance`** — With 4,108 models loaded, measure that `rebuild_display_rows()` completes in under 20ms (asserted with `Instant::now()` timing). The 20ms threshold allows for slow CI runners while still catching significant regressions (expected: < 5ms on modern hardware). Log the actual elapsed time with `eprintln!` for trend tracking. This test should be `#[ignore]` in normal CI and run explicitly during performance validation. Mark with `#[ignore]` and document: `cargo test scale_test_4k -- --ignored` to run.

3. **`scale_test_4k_models_memory_no_growth`** — Load 4,108 models. Apply 10 different filter combinations. Verify `cached_display_rows.len()` tracks correctly each time. Verify no unbounded memory growth (the Vec is replaced, not appended to).

### GREEN

- If any test fails due to performance, optimize `rebuild_display_rows()`:
  - Use `Vec::with_capacity()` to pre-allocate `cached_display_rows` based on model count.
  - Consider binary search for provider lookup if linear scan is slow at 105 providers (unlikely, but profile first).
- If any test fails due to correctness, fix the filter logic.

### REFACTOR

- No refactoring expected. This phase is primarily validation.


---

## Phase 4 — Integration Verification and Cleanup

### Tasks (Orchestrator)

1. Run full verification: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --lib --tests`.
2. Run lizard complexity check: `python -m lizard -C 50 -L 100 -w src/`.
3. Review all modified files for:
   - Dead code, unused imports, stale comments.
   - Any remaining `println!` debug statements (search across all model_selector_view files).
   - Verify `components/list.rs` was removed or deprecated.
4. Commit, push, update PR.
5. Watch CI: `gh pr checks NUM --watch --interval 300` (loop up to 5×).
6. Review and remediate all CodeRabbit comments.
7. Loop until all checks green and all CodeRabbit issues resolved.


---

## Known Risks and Resolutions

| # | Risk | Impact | Resolution |
|---|------|--------|------------|
| 1 | Sub-pixel height drift between provider headers and model rows | Scroll position drift, misaligned clicks | Explicit `h(px(28.0))` on all rows; no intrinsic sizing. This is a GPUI `uniform_list` requirement. |
| 2 | Stale `row_count` after filter change causes OOB in callback | Missing rows | `.get(ix)?` guard in `uniform_list` callback skips invalid indices (returns fewer elements). No zero-height divs that corrupt scroll. Test P1.2. |
| 3 | Scroll position past end after filter reduces list | Blank scroll area | `rebuild_and_reset_scroll()` always resets to top on every filter mutation (including keystrokes). Any filter change invalidates the user's scroll context since the list content changed. `set_models()` conditionally resets (first load or empty result only — background refreshes preserve scroll). Test P1.3. Correct two-arg API. |
| 4 | GPUI sibling z-order assumption wrong for hit-testing | Dropdown won't receive scroll events | Empirical verification in P2. Fallback: `on_scroll_wheel(stop_propagation)` on dropdown. |
| 5 | `set_models()` with stale `selected_provider` | Empty list, no explanation | Clear filter only if provider absent from new data. Preserve valid filter. Test P0.4. |
| 6 | `to_lowercase()` allocations on every keystroke × 4K | Defeats perf optimization | Pre-lowercase at load time in `SearchableModelInfo`. Query lowered once per `rebuild_display_rows()`. |
| 7 | `uniform_list` hardcodes `overflow.y = Scroll` | Can't suppress scroll on model list when dropdown is open | Don't render `uniform_list` when dropdown is open; use static placeholder. Test P2.5. |
| 8 | Backdrop covers top bar | Cancel button unclickable | Backdrop starts at `top(px(TOP_BAR_H))`, not `top(px(0.0))`. Test P2.3. |
| 9 | `Model(usize)` indices stale after `set_models()` between rebuild and render | Wrong model selected on click | `set_models()` always calls `rebuild_display_rows()` atomically. Stale-read guard in callback. Safety comment on `DisplayRow::Model`. |
| 10 | `components/list.rs` duplicate implementation | Maintenance burden, confusion | Remove in P1 REFACTOR. |
| 11 | `println!` debug statements in `command.rs` | Performance noise in production | Remove in P0 GREEN. |

---

## Verification Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --tests

# Complexity check
python3 -m venv .venv-lizard
. .venv-lizard/bin/activate
python -m pip install lizard
python -m lizard -C 50 -L 100 -w src/
```

---

## Definition of Done

- [ ] `uniform_list` renders model list — only visible rows instantiated per frame.
- [ ] Cached state: `rebuild_display_rows()` pre-computes flattened display rows, called on every filter/data mutation.
- [ ] All filter mutations wired to `rebuild_and_reset_scroll()` (rebuild cache + reset scroll to top) — no stale cache between phases.
- [ ] Provider list cached once on `set_models()`, not recomputed per render.
- [ ] Search uses pre-lowered fields in `SearchableModelInfo` — query lowered once per rebuild, zero per-model `to_lowercase()`.
- [ ] Scroll handle resets to top on every filter change (correct two-arg API).
- [ ] Stale-read guard prevents OOB panic in `uniform_list` callback.
- [ ] `set_models()` clears stale provider filter; preserves valid provider filter.
- [ ] Empty state shows "No matching models" as a static div, not a zero-item `uniform_list`.
- [ ] Dropdown scrolls independently; model list replaced with placeholder when dropdown is open.
- [ ] Backdrop does not cover top bar — Cancel button always clickable.
- [ ] `components/list.rs` removed or deprecated.
- [ ] All `println!` debug statements removed from model_selector_view files.
- [ ] 4K-model scale test passes with < 20ms rebuild time (< 5ms expected on modern hardware).
- [ ] All existing tests updated and passing.
- [ ] Full verification passes locally.
- [ ] PR updated, CI green, CodeRabbit issues remediated.

---

## Appendix: Disposition of All 30 Critique Issues

### Issues Fully Addressed (27 of 30)

| # | Issue | Disposition | Where Addressed |
|---|-------|-------------|-----------------|
| 1 | `scroll_to_item(0)` wrong — needs `(0, ScrollStrategy::Top)` | **Valid. Fixed.** | P1 GREEN: all scroll resets use `scroll_to_item(0, ScrollStrategy::Top)`. API Reference documents the two-arg signature verified against GPUI source. |
| 2 | `rebuild_display_rows()` on State can't access scroll_handle on View | **Valid. Fixed.** | P1 GREEN: `rebuild_display_rows()` is data-only. The caller (on `ModelSelectorView`) calls `self.scroll_handle.scroll_to_item(...)` after `self.state.rebuild_display_rows()`. Clean separation of concerns. |
| 3 | `SearchableModelInfo` ambiguous — wrapper or lowercase fields? | **Valid. Fixed.** | P0 GREEN: concrete decision — `SearchableModelInfo` is a **wrapper** struct containing `info: ModelInfo` plus `id_lower: String` and `provider_lower: String`. `ModelInfo` is not modified. |
| 4 | HashMap iteration non-deterministic | **Valid. Fixed.** | P0 GREEN: `rebuild_display_rows()` iterates `cached_providers` (sorted `Vec<String>`), not `HashMap` keys. Test P0.8 explicitly verifies alphabetical ordering. |
| 5 | `set_models()` wipes user's provider filter on every refresh | **Valid. Fixed.** | P0 GREEN: `set_models()` clears `selected_provider` only if the provider is absent from the new `cached_providers`. Test P0.4 verifies preservation. |
| 6 | "Zero `to_lowercase`" false — query still needs lowering per rebuild | **Valid. Fixed.** | P0 GREEN: query is lowered **once** at the top of `rebuild_display_rows()`: `let query_lower = self.search_query.to_lowercase();`. Models use pre-lowered fields. Zero per-model lowercase. |
| 7 | `cached_filtered_count` / `cached_visible_provider_count` redundant, will drift | **Valid. Fixed.** | P0 GREEN: counts are stored as `cached_model_count` / `cached_provider_count`, updated atomically during `rebuild_display_rows()` in a single pass. No per-render iteration. Accessor methods provide read-only access. |
| 8 | `Model(usize)` indices stale if `set_models()` between rebuild and render | **Valid. Fixed.** | P0 GREEN: `load_models()` atomically calls `rebuild_display_rows()`. Stale-read guard (`.get(ix)` + `filter_map`) in callback handles the render-time gap. Correctness invariant comment on `DisplayRow::Model`. |
| 9 | Phase 0 tests unit-only — miss GPUI render bugs | **Valid. Partially addressed.** | P0 adds unit tests for cache correctness. P1 adds GPUI TestAppContext tests for render behavior (stale-read guard, scroll reset, empty state, OOB). P2 adds GPUI tests for dropdown structure. The division is deliberate: cache logic is testable in pure unit tests; render/GPUI behavior requires TestAppContext. |
| 10 | `entity.read(cx)` borrow dangling without `.clone()` in click closure | **Valid. Fixed.** | P1 GREEN: `cx.processor()` eliminates `entity.read(cx)` entirely. The callback receives `&mut Self` directly. Click handlers use `cx.listener()`. No manual entity handle management or per-frame cloning. |
| 11 | `uniform_list` hardcodes `overflow.y = Scroll` | **Valid. Acknowledged as GPUI constraint.** | P1 GREEN: documented in API Reference. P2 works around it by not rendering `uniform_list` when the dropdown is open (static placeholder instead). No attempt to suppress the hardcoded overflow. |
| 12 | Height 24→28px unstated visual change | **Valid. Fixed.** | API Reference section: "Visual change: Provider header height 24px → 28px" is explicitly documented. Headers differentiated by bold text + background, not height. |
| 13 | Empty state needs conditional render | **Valid. Fixed.** | P1 GREEN: `render_model_list()` checks `cached_display_rows.is_empty()` first and returns a static "No matching models" div. Only non-empty state calls `uniform_list`. Test P1.5. |
| 14 | "Uniform height" test unimplementable in TestAppContext | **Valid. Acknowledged.** | Replaced with test P1.1 which verifies the correct row count (the value passed to `uniform_list`). Pixel-exact height cannot be asserted in TestAppContext; instead, the code uses explicit `h(px(28.0))` and visual verification happens in manual testing. |
| 15 | `scroll_to_item` before first render untested | **Valid. Fixed.** | P1 test P1.4 explicitly tests `scroll_to_item(0, ScrollStrategy::Top)` on a fresh `UniformListScrollHandle::default()` before any render, verifying no panic and deferred state is set. |
| 16 | Scroll isolation needs `on_scroll_wheel` consumer on backdrop/dropdown | **Valid. Fixed.** | P2 GREEN: dropdown menu has `on_scroll_wheel` listener that consumes the event. Model list is replaced with placeholder when dropdown is open. |
| 17 | Backdrop covers top bar — can't click Cancel | **Valid. Fixed.** | P2 GREEN: backdrop starts at `top(px(TOP_BAR_H))` (44px), NOT `top(px(0.0))`. Top bar remains fully clickable. Test P2.3. |
| 18 | "overflow_hidden when dropdown open" test unimplementable | **Valid. Acknowledged, approach changed.** | Test replaced with P2.5 which verifies the model list is replaced with a static placeholder (not `uniform_list`) when the dropdown is open. This is testable. |
| 19 | `replace_text_in_range` ignores range param (pre-existing) | **Valid but out of scope.** | Documented as a pre-existing IME limitation. The current behavior (always appending/replacing at end) works for the single-line search field. Fixing full range support is a separate issue. Noted in "What is out of scope" section. |
| 20 | `unmark_text` exclusion undocumented | **Valid. Fixed.** | `unmark_text()` clears `ime_marked_byte_count = 0`. This is correct behavior: it finalizes the IME composition. Documented in a code comment explaining that `unmark_text` is called by the IME system when the user confirms a composition candidate, and the implementation simply clears the marked-byte counter. |
| 21 | No end-to-end filter→rebuild→render test | **Valid but acknowledged as GPUI limitation.** | The closest approximation is P0 tests (verify cache after each mutation) + P1 tests (verify render behavior). A true end-to-end test that exercises GPUI's full render pipeline (keystroke → IME → state mutation → rebuild → uniform_list callback → DOM) is not reliably achievable in `TestAppContext` because `uniform_list` rendering requires a full GPUI window with layout measurement. This is documented as a known gap. |
| 22 | No performance benchmarks | **Valid. Fixed.** | P3 adds scale tests: 4K-model rebuild correctness, < 5ms rebuild timing, and memory stability. |
| 23 | `components/list.rs` exists — two list implementations | **Valid. Fixed.** | P1 REFACTOR: remove `components/list.rs`. It renders all items eagerly, has no virtual scrolling, and is not used by the model selector. Verify no other code references it; if so, deprecation-path with `#[deprecated]`. |
| 24 | No accessibility considerations | **Valid. Acknowledged.** | P0 REFACTOR: accessibility marked as a future enhancement. GPUI may not expose standard `role`/`aria-*` APIs. Existing descriptive `id` attributes on rows preserved. TODO added for when GPUI accessibility APIs become available. Not a regression from current code. |
| 25 | `println!` debug statements should be removed | **Valid. Fixed.** | P0 GREEN: remove all `println!` from `command.rs` (lines 10, 29, 44, 46, 92). P4 verification: grep all model_selector_view files for remaining `println!`. |
| 26 | `UniformListScrollHandle` Clone compatibility | **Invalid (already derives Clone).** | Verified against GPUI source: `UniformListScrollHandle` derives `Clone, Debug, Default` (line 79). No issue exists. |
| 27 | Phase ordering: P0 creates rebuild, P3 wires it. P1–P2 stale. | **Valid. Fixed.** | Phase dependency map reorganized: P0 now includes both cache creation AND rebuild wiring (command.rs + ime.rs). No phase ever has unwired cache. Dependency map section explicitly documents the fix. |
| 28 | No test at 4K model scale | **Valid. Fixed.** | P3 adds dedicated scale tests with 4,108 models × 105 providers. |
| 29 | `Model(usize)` breaks if models reordered. No invariant. | **Valid. Fixed.** | Correctness invariant documented as a comment on `DisplayRow::Model`. Indices are valid only within the current rebuild cycle. `load_models()` atomically rebuilds. Stale-read guard in callback. |
| 30 | Key Constraints documents flaws without resolving | **Valid. Fixed.** | The revised specification resolves every constraint: `scroll_to_item` API (2 args), overflow.y workaround (placeholder div), height uniformity (28px documented), stale-read guard (clone pattern), phase ordering (rewired). The "Known Risks" table now includes specific resolutions, not just mitigations. |

### Summary

- **24 issues valid, fully addressed** in the revised plan.
- **5 issues valid, acknowledged with documented rationale** (14: height test, 18: overflow_hidden test, 19: IME range pre-existing, 21: E2E render test GPUI limitation, 24: accessibility deferred to future GPUI API).
- **1 issue invalid** (26: `UniformListScrollHandle` already derives `Clone`).
