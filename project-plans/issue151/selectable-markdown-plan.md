# Plan: Per-message selectable Markdown

Plan ID: PLAN-20260713-ISSUE151
Generated: 2026-07-13
Issue: https://github.com/acoliver/personal-agent/issues/151
Failed precedent: https://github.com/acoliver/personal-agent/pull/159

## Goal

Implement native-feeling text selection in finalized user and assistant messages on macOS and Linux without replacing rich Markdown with flat text. Selection is confined to one message in this increment.

## Clean-room and precedent note

PersonalAgent is MIT. Zed's selectable Markdown implementation at the pinned GPUI revision is GPL-3.0-or-later. Its code was inspected only to confirm GPUI API feasibility and broad interaction concepts. This implementation must use original PersonalAgent structures, names, control flow, and tests; it must not copy Zed Markdown code or depend on Zed's Markdown/UI crates.

PR #159 is not a base for this implementation. In particular, this plan forbids its global flattened transcript, alternate flat-text render path, phantom layouts/layout sinks, first-click arming, deferred event replay, and direct right-click copy.

## Requirements

### REQ-151-001: Drag selection

**Full text:** A first-attempt left-button drag in a finalized user or assistant message selects a partial range of visible text. Forward and reverse drags work, including wrapped lines, and selection cannot enter another message.

### REQ-151-002: Preserved rendering

**Full text:** Selecting text paints visible feedback over the existing Markdown rendering. Headings, inline styles, code, lists, blockquotes, tables, image fallback text, and links retain their existing layout and styling.

### REQ-151-003: Copy

**Full text:** Command+C on macOS and Control+C on Linux copy the selected visible plain text, excluding Markdown delimiters. Transcript selection takes precedence over composer/sidebar/title copy fallbacks.

### REQ-151-004: Multi-click

**Full text:** Double-click selects a Unicode-safe word. Triple-click selects the semantic block under the pointer (paragraph, heading, code block, list-item paragraph, table cell, or fallback block).

### REQ-151-005: Context menu

**Full text:** Right-click on message text opens a real in-app context menu containing Copy. The menu captures the current selection before focus changes and dismisses on Copy, outside click, Escape, or transcript replacement.

### REQ-151-006: Links

**Full text:** A no-drag click on a safe HTTP/HTTPS link opens it. A drag beginning on link text selects text and does not open the link. Unsafe links never open.

### REQ-151-007: Unicode and freshness

**Full text:** Every selection boundary is a valid UTF-8 byte boundary. A selection is never reused after its conversation, displayed message content, streaming revision, or emoji-filtered display changes.

### REQ-151-008: Platform and automation

**Full text:** The behavior works on macOS and Linux. Cross-platform GPUI interaction tests drive mouse, keyboard, context menu, and clipboard paths. A real Linux X11 automation script drives the compiled application and verifies selection against the configured model response without human intervention.

## Architecture

### Per-message visible document

Each selectable message has a canonical visible document generated from the same Markdown IR and at the same time as the real GPUI child tree:

- `text`: visible plain text used for selection and copy.
- `leaves`: visible-document ranges paired with the exact `TextLayout` handles belonging to rendered `StyledText` leaves.
- `links`: visible-document ranges and destinations.
- `blocks`: semantic ranges used by triple-click.
- explicit logical separators between rendered blocks/cells/items.

Copy slices this visible document, never raw Markdown or a separately transformed string.

### Custom element lifecycle

A `SelectableMarkdown` custom element owns the real rendered child and its current-frame visible-document metadata through GPUI request-layout, prepaint, and paint:

1. Build the existing rich Markdown hierarchy and metadata once.
2. Insert each tracked `StyledText` into that hierarchy and retain its cloneable layout handle.
3. Request layout and prepaint the same child.
4. Use only those measured current-frame layouts for pointer hit testing.
5. Paint selection quads over that unchanged child.
6. Register current-frame mouse down/move/up listeners so the very first drag works and movement outside the body clamps to the starting message.

No geometry is stored in `ChatView`.

### Selection ownership

`ChatView` owns transient message selection and context-menu state, not persisted/presenter `ChatState`. Selection stores a message revision and visible-document snapshot, anchor/head, and character/word/block mode. All transcript mutations route through one clear/invalidate path.

### Link arbitration

Selectable leaves retain link styles but message-level gesture handling decides link activation. Same-link down/up without exceeding the drag threshold opens the URL. Movement beyond the threshold starts selection from the original down offset and cancels link activation.

### Context menu

A root-level absolute overlay follows existing chat dropdown patterns. It captures selected text when opened, clamps to the viewport, blocks underlying clicks, and shows platform-appropriate shortcut text.

## Test-first phases

### Phase 0: Preflight and first-frame proof

- Verify GPUI custom `Element` lifecycle and test-support APIs at the pinned revision.
- Create a minimal rendered-leaf test harness.
- Failing then passing GPUI test: first mouse-down after first draw hit-tests a measured real-tree layout without panic, warm-up click, sink, arming, or replay.

**Blocking gate:** Do not proceed if first-frame current-tree hit testing cannot be proven.

### Phase 1: Visible document and selection model

Tests first:

- forward/reverse/empty ranges;
- UTF-8 safety over every byte position in ASCII, accented, CJK, combining, and emoji samples;
- Unicode-safe word boundaries;
- semantic block selection and word/block drag extension;
- stale revision rejection;
- visible text and separators for every Markdown block variant;
- links and style runs map to exact visible ranges;
- selected visible text excludes Markdown syntax.

Implement pure model helpers independent of geometry.

### Phase 2: Rich renderer metadata

Tests first for paragraphs, headings, code, blockquotes, nested lists, tables, task markers, links, image fallbacks, and malformed-table normalization:

- ranges are ordered, in bounds, non-overlapping where required, and character-aligned;
- leaf text plus separators reconstructs the visible document;
- link ranges match labels;
- semantic block ranges match triple-click units;
- the same rich tree is used regardless of selection state.

Refactor Markdown rendering around an internal builder while preserving public wrappers.

### Phase 3: Pointer interaction and selection painting

GPUI interaction tests first:

- first-attempt forward and reverse drag;
- wrapped-line and multi-leaf drag;
- drag outside body and one-message clamping;
- double-click word and triple-click semantic block;
- selection quads per leaf without bridging table/list gaps;
- Markdown styles and link metadata remain unchanged during selection;
- no whole-message clipboard side effect during drag or multi-click.

Implement `SelectableMarkdown` current-frame event handling and overlay painting.

### Phase 4: Links, keyboard, invalidation, and context menu

Tests first:

- safe link click opens exactly once;
- link drag selects and does not open;
- unsafe links do not open;
- Command+C/Control+C copy exact highlighted text;
- no selection preserves existing copy fallback;
- emoji-filtered visible text equals clipboard text;
- conversation/message/stream/filter changes clear stale selection;
- right-click shows Copy, captures text, executes copy, clamps position, and dismisses correctly.

Wire message callbacks into `ChatView` and add the root overlay.

### Phase 5: Deterministic app automation

Add `scripts/e2e/run_issue151_selection_e2e.sh` and an X11 Python driver that:

1. Builds and launches the real app in a temporary config/data environment.
2. Configures or uses the existing ZAI profile/key without printing secrets.
3. Sends a prompt requesting deterministic rich Markdown with unique ASCII and Unicode tokens.
4. Drives first drag, reverse drag, double-click, triple-click, Control+C, right-click Copy, link click-vs-drag, scrolling, and conversation change through `xdotool`.
5. Asserts the real clipboard with `xclip`, intercepts external URL opening, captures screenshots/logs, and emits a machine-readable result under `artifacts/issue151/`.
6. Cleans up its process/temp environment automatically.

In-process GPUI interaction tests run on both Linux and macOS CI. Linux desktop automation is run locally now. A macOS desktop script/check is added where practical, but macOS native interaction is not claimed solely from Linux evidence.

## Required verification

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --tests
cargo xtask guard
python -m lizard -C 50 -L 100 -w src/
cargo coverage
cargo test --test issue151_text_selection_gpui_tests -- --nocapture
scripts/e2e/run_issue151_selection_e2e.sh
```

Run provider-backed E2E where its established contract is applicable. Run macOS CI checks and address platform failures before merge.

## Non-negotiable stop conditions

Revise the architecture rather than adding a workaround if implementation requires any of:

- flattened cross-message transcript;
- alternate flat rendering during selection;
- phantom/unrendered layouts;
- layout sinks into `ChatView`;
- selection arming or deferred first-click replay;
- selection offsets reused across message revisions;
- clipboard text sourced from a different document than the highlighted text.

## Definition of done

Every issue acceptance criterion is exercised by a real GPUI interaction test; Linux desktop automation passes against the compiled app; formatting and links remain unchanged while selected; full verification and CI pass on Linux and macOS; CodeRabbit findings are resolved; and the PR includes evidence and `Fixes #151`.
