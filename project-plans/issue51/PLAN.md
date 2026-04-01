# Issue #51: Error Log Viewer — Implementation Plan

**Branch:** `issue51`
**Mockup:** `project-plans/issue51/mockup.html`

## Overview

Errors from LLM providers are currently invisible. The streaming pipeline captures errors through to the AppStore (`StreamError -> reduce_stream_error -> last_error in snapshot -> StreamingState::Error in chat_view`), but the chat view render code never displays the `Error` variant — it only handles `Streaming` and `Idle`. The result is silent failure.

This plan delivers:
1. **Immediate fix:** Render `StreamingState::Error` inline in the chat area
2. **Bug icon:** SVG bug icon in the title bar with unviewed-error count badge
3. **Error ring buffer:** In-memory store of last ~100 errors
4. **Error log view:** Full navigable error history view
5. **Error capture:** Feed all error paths into the ring buffer

## Visual Reference

All UI elements MUST be compared against `project-plans/issue51/mockup.html` during implementation and review. The mockup shows exact layout, placement, and proportions using the green-screen theme.

**Scrollbar note:** The mockup shows browser-default scrollbars in the error list. The GPUI app uses `.overflow_y_scroll()` which renders platform-native scrollbars — the app does NOT have custom-styled scrollbars. The theme catalog defines `scrollbar.thumb` and `scrollbar.track` colors but these are not consumed by any GPUI view today. Ignore the mockup scrollbar appearance; use the same `.overflow_y_scroll()` pattern as all other views.

## Architecture Decisions

- **Error store location:** Global `ErrorLogStore` using `Arc<Mutex<VecDeque<ErrorLogEntry>>>`, similar to how `NavigationChannel` uses global state. Accessible from presenters (async) and views (sync).
- **Ring buffer:** `VecDeque<ErrorLogEntry>` capped at 100. Push front, pop back when full.
- **Unviewed tracking:** Atomic counter (`AtomicUsize`) for unviewed count, reset when error view is opened.
- **No disk persistence:** All in-memory per the issue spec.
- **Theme compliance:** ALL colors via `Theme::*` tokens. No hardcoded colors. The mockup uses CSS variables mapped to the green-screen theme — implementation must use the equivalent `Theme::error()`, `Theme::text_primary()`, etc.
- **SVG bug icon:** Rendered as GPUI path elements using `stroke="currentColor"` to inherit theme color. The SVG is defined in the mockup symbol `#bug-icon`.

## Phases

### Phase 1: Error Data Types & Ring Buffer (~40 LoC production, ~60 LoC tests)

**Files to create:**
- `src/ui_gpui/error_log.rs` — `ErrorLogEntry`, `ErrorSeverityTag`, `ErrorLogStore`

**Types:**
```rust
pub enum ErrorSeverityTag {
    Stream,
    Auth,
    Connection,
    Mcp,
    Internal,
}

pub struct ErrorLogEntry {
    pub id: u64,                          // monotonic counter
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub severity: ErrorSeverityTag,
    pub source: String,                   // e.g. "kimi / kimi-k2-0711"
    pub message: String,                  // human-readable error
    pub raw_detail: Option<String>,       // raw HTTP body / error detail
    pub conversation_title: Option<String>,
    pub conversation_id: Option<uuid::Uuid>,
}

pub struct ErrorLogStore { ... }
```

**ErrorLogStore API:**
- `push(entry)` — add entry, cap at 100, increment unviewed counter
- `entries() -> Vec<ErrorLogEntry>` — return clone of all entries (newest first)
- `unviewed_count() -> usize`
- `mark_all_viewed()`
- `clear()`
- `global() -> &'static ErrorLogStore` — singleton accessor (like `navigation_channel()`)

**Tests:**
- Ring buffer caps at 100
- Push increments unviewed count
- `mark_all_viewed` resets count to 0
- `clear` empties buffer and resets count
- `entries()` returns newest-first order
- Thread safety (push from multiple threads)

### Phase 2: Inline Error Rendering — Immediate Fix (~30 LoC production, ~20 LoC tests)

**Files to modify:**
- `src/ui_gpui/views/chat_view/render.rs` — add `.when()` block for `StreamingState::Error`

In `render_chat_area`, after the existing `.when(matches!(streaming, StreamingState::Streaming { .. }), ...)` block, add:

```rust
.when(matches!(streaming, StreamingState::Error(..)), |d| {
    let error_msg = match &streaming {
        StreamingState::Error(msg) => msg.clone(),
        _ => String::new(),
    };
    d.child(
        div().id("stream-error-inline")
            // error indicator box matching mockup layout
    )
})
```

Layout: horizontal flex with bug icon SVG + vertical stack of "Stream Error" title + error message text. Border and background use `Theme::error()` with opacity. Compare result against mockup Section 4.

**Tests:**
- `StreamingState::Error("msg")` produces non-empty render (structural/snapshot test if feasible, otherwise verify state transition)

### Phase 3: Bug Icon SVG Component (~35 LoC)

**Files to create:**
- `src/ui_gpui/components/bug_icon.rs` — reusable SVG bug icon as GPUI element

The icon is rendered using GPUI's `svg()` or path-based drawing. Uses `currentColor` pattern (inherits `text_color` from parent). Must be recognizable at 14px (title bar) and usable at larger sizes in the inline error indicator.

**Files to modify:**
- `src/ui_gpui/components/mod.rs` — add `pub mod bug_icon;` and re-export

**Tests:**
- Component constructs without panic

### Phase 4: Bug Icon in Title Bar (~40 LoC production, ~20 LoC tests)

**Files to modify:**
- `src/ui_gpui/views/chat_view/render_bars.rs` — add bug icon button to `render_title_bar`, to the right of the profile selector pill

The bug icon button:
- Hidden (`visibility: hidden` equivalent) when `ErrorLogStore::global().unviewed_count() == 0`
- Visible with theme-colored count badge when unviewed errors > 0
- On click: navigate to `ViewId::ErrorLog` via `navigation_channel()`
- Badge: small circle positioned top-right with count text, using `Theme::error()` bg and `Theme::selection_fg()` text (matching the green-on-black pattern in green-screen theme)

Compare placement against mockup Section 2 — icon sits after `[profile selector ▼]`.

**Tests:**
- Verify bug button visibility logic with 0 vs >0 unviewed errors

### Phase 5: Error Log View (~150 LoC production, ~40 LoC tests)

**Files to create:**
- `src/ui_gpui/views/error_log_view.rs` — new view with top bar + scrollable error list

**Structure (matching mockup Section 3):**
- Top bar: `[← Back]` button + "Error Log" title + count label + `[Clear All]` button
- Scrollable error list using `.overflow_y_scroll()` (NOT custom scrollbars)
- Each error entry card: severity tag, source, timestamp, error message, conversation context
- Raw response detail is deferred (expandable toggle can come in a follow-up)

**View state:**
- Read from `ErrorLogStore::global().entries()` on mount and on notify
- Call `ErrorLogStore::global().mark_all_viewed()` when view is rendered (clears badge)
- `← Back` navigates back via `navigation_channel()`
- `Clear All` calls `ErrorLogStore::global().clear()` and notifies

**Files to modify:**
- `src/ui_gpui/views/mod.rs` — add `pub mod error_log_view;` and re-export

**Tests:**
- View constructs with empty error store
- View constructs with populated error store
- Clear all empties the list
- Mark viewed on render

### Phase 6: Navigation Integration (~30 LoC production, ~15 LoC tests)

**Files to modify:**
- `src/presentation/view_command.rs` — add `ErrorLog` to `ViewId` enum
- `src/ui_gpui/views/main_panel/mod.rs` — add `error_log_view: Option<Entity<ErrorLogView>>` field
- `src/ui_gpui/views/main_panel/render.rs` — add `ViewId::ErrorLog` to `render_view_content` and `focus_current_view`
- `src/ui_gpui/views/main_panel/startup.rs` — initialize `ErrorLogView` alongside other views
- `src/ui_gpui/navigation.rs` — no changes needed (already supports any `ViewId`)

**Tests:**
- Navigation to `ViewId::ErrorLog` and back
- `ViewId::ErrorLog` serialization round-trip

### Phase 7: Error Capture — Feed All Error Paths to Ring Buffer (~50 LoC)

**Files to modify:**
- `src/presentation/error_presenter.rs` — in `handle_event`, push errors to `ErrorLogStore::global()`
- `src/ui_gpui/app_store.rs` or `app_store_streaming.rs` — when `StreamError` is reduced, also push to error log store

Error capture points (from the issue):
- Stream errors (already flow through `ErrorPresenter::handle_chat_error`)
- Auth failures (subset of stream errors with 401/403 responses)
- Connection timeouts (subset of stream errors)
- MCP errors (`ErrorPresenter::handle_mcp_error`)
- System errors (`ErrorPresenter::handle_system_event`)

Severity classification heuristic:
- Message contains "401" or "403" or "unauthorized" or "forbidden" → `Auth`
- Message contains "timeout" or "connection" or "refused" → `Connection`
- MCP events → `Mcp`
- All other chat/stream errors → `Stream`
- System errors → `Internal`

Conversation context: extract from the active conversation title in the store snapshot at the time of the error.

**Tests:**
- Stream error creates `ErrorLogEntry` with correct severity
- MCP error creates entry with `Mcp` severity
- Auth-pattern error gets `Auth` tag
- Connection-pattern error gets `Connection` tag

### Phase 8: Verification & Polish

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --lib --tests`
- Lizard complexity check (all new functions under L100 threshold)
- Visual comparison of running app against mockup.html
- Verify theme compliance: switch themes and confirm all error UI adapts

## File Summary

| Action | Path |
|--------|------|
| CREATE | `src/ui_gpui/error_log.rs` |
| CREATE | `src/ui_gpui/components/bug_icon.rs` |
| CREATE | `src/ui_gpui/views/error_log_view.rs` |
| MODIFY | `src/ui_gpui/mod.rs` |
| MODIFY | `src/ui_gpui/components/mod.rs` |
| MODIFY | `src/ui_gpui/views/mod.rs` |
| MODIFY | `src/ui_gpui/views/chat_view/render.rs` |
| MODIFY | `src/ui_gpui/views/chat_view/render_bars.rs` |
| MODIFY | `src/ui_gpui/views/main_panel/mod.rs` |
| MODIFY | `src/ui_gpui/views/main_panel/render.rs` |
| MODIFY | `src/ui_gpui/views/main_panel/startup.rs` |
| MODIFY | `src/presentation/view_command.rs` |
| MODIFY | `src/presentation/error_presenter.rs` |

## Deepthinker Review Feedback (incorporated)

1. **Single ingest point:** All errors flow through `ErrorPresenter` only — do NOT also push from `app_store_streaming.rs`. This avoids duplicate entries.
2. **Store notification:** `ErrorLogStore::push()` increments an atomic revision counter. The chat title bar reads `unviewed_count()` every render frame (cheap atomic read, same pattern as `navigation_channel().has_pending()`). ErrorLogView reads `entries()` on mount.
3. **Severity classification:** Use structured signals first (HTTP status codes parsed from error messages, MCP event types), string heuristics as fallback only. Add precedence tests.
4. **ViewId exhaustive matches:** Audit ALL `match` on `ViewId` across the codebase — not just the files listed. Add wildcard/catch-all arms where appropriate.
5. **Empty state:** ErrorLogView shows "No errors recorded" centered text when buffer is empty.
6. **Bug icon accessibility:** Hidden icon is non-focusable. Visible icon has conceptual tooltip "Error Log".
7. **Additional files to audit:** `chat_view/mod.rs` re-exports, test module registrations, any `ViewId` match sites beyond main_panel.

## Review Checklist

- [ ] All UI colors use `Theme::*` tokens — no hardcoded hex values
- [ ] Bug icon placement matches mockup Section 2 (right of profile pill in title bar)
- [ ] Inline error box matches mockup Section 4 (in chat area below messages)
- [ ] Error log view layout matches mockup Section 3 (back button, title, entries)
- [ ] Error entries show: severity tag, source, timestamp, message, conversation context
- [ ] Ring buffer caps at 100, newest-first order
- [ ] Unviewed count badge appears/disappears correctly
- [ ] Viewing error log clears the unviewed count
- [ ] Navigation to/from error log works (stack-based)
- [ ] All error paths feed into the ring buffer
- [ ] Scrolling uses `.overflow_y_scroll()` — no custom scrollbar styling
- [ ] No new clippy warnings, fmt clean, all tests pass
- [ ] Functions stay under lizard L100 complexity threshold
