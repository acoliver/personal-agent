# Phase 05a: Presenter/Store Runtime Integration Verification

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P05a`

## Prerequisites

- Required: Phase 05 completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P05.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P05.md`

## Verification Commands

```bash
cargo check --all-targets -q
cargo test --test presenter_selection_and_settings_tests --test chat_view_conversation_switch_regression_tests --test seven_bugs_regression_tests --test chat_startup_scrollback_layout_regression_tests --test gpui_wiring_command_routing_tests --test gpui_wiring_event_flow_tests --test gpui_integration_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P05" src/ui_gpui src/presentation src/main_gpui.rs src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
grep -R -n "@requirement[: ]REQ-ARCH-003.2\|@requirement[: ]REQ-ARCH-003.3\|@requirement[: ]REQ-ARCH-003.4\|@requirement[: ]REQ-ARCH-003.6" src/ui_gpui src/presentation src/main_gpui.rs src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/01-app-store.md:\|@pseudocode[: ]analysis/pseudocode/02-selection-loading-protocol.md:\|@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" src/ui_gpui src/presentation src/main_gpui.rs src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"

grep -R -n "ConversationLoadFailed\|selection_generation\|SelectConversation\|drain_commands\|try_send\|current_snapshot\|revision" src/ui_gpui src/presentation src/main_gpui.rs src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
grep -R -n "ShowThinking\|HideThinking\|AppendThinking\|AppendStream\|FinalizeStream\|StreamCancelled\|StreamError\|ShowToolCall\|UpdateToolCall" src/ui_gpui src/presentation --include="*.rs"
grep -rn "todo!\|unimplemented!" src/main_gpui.rs src/ui_gpui src/presentation src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
grep -rn -E "(// TODO|// FIXME|// HACK|placeholder|not yet)" src/main_gpui.rs src/ui_gpui src/presentation src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
```

## Structural Verification Checklist

- [ ] Store reduction path exists for runtime presenter updates
- [ ] Selection loading protocol exists in code
- [ ] Chat/history views consume updated snapshot state
- [ ] Required `@plan`, `@requirement`, and `@pseudocode` markers are present in touched production items
- [ ] `selection_generation` is present on both `ConversationActivated` and `ConversationMessagesLoaded`
- [ ] `ConversationLoadFailed` exists as a named `ViewCommand` variant and is wired through affected sites
- [ ] exact proof artifact homes are named in evidence for: always-live ingress / single minting site, no-publication-on-ignored-inputs, same-id reselection semantics, and bounded `ConversationCleared` restoration
- [ ] a named deterministic proof artifact is attached for sole production drainer behavior
- [ ] a named deterministic proof artifact is attached for sole ordinary-runtime minting-site behavior, even if the final helper/function name differs from this plan's shorthand
- [ ] a named deterministic proof artifact is attached for same-id reselection semantics (`Loading`/`Ready` no-op, `Error` retry)
- [ ] a named deterministic proof artifact is attached for ignored/no-op/stale commands not bumping revision or publishing
- [ ] selection user-event transport carries the minted token and is wired through the existing bridge/forwarder path

## Semantic Verification Checklist

- [ ] Manual selection shows loading/ready/error semantics rather than false empty state
- [ ] the first authoritative runtime selection transition occurs at the GPUI runtime boundary before async load work begins
- [ ] matching runtime `ConversationActivated` payloads are treated as idempotent echo rather than a second logical transition/publication, except for the one bounded title-provenance upgrade from `LiteralFallback("Untitled Conversation")` to `HistoryBacked`
- [ ] `ConversationActivated` no longer causes ordinary transcript clear
- [ ] `ConversationMessagesLoaded` performs bulk replacement correctly for matching generation only
- [ ] stale/off-target success and failure payloads are ignored
- [ ] same-id reselection while `Loading`/`Ready` is a no-op and same-id reselection from `Error` retries with a new generation
- [ ] named deterministic negative-control proof would fail if any production path could still dispatch selection async load work without first passing through the single GPUI-owned selection-intent boundary and ordinary-runtime minting site, including same-id reselection while `Loading`/`Ready`
- [ ] currently active GPUI streaming/thinking behavior remains intact, including `Uuid::nil()` targeting semantics already exercised by `ChatPresenter` and `ChatView`
- [ ] `FinalizeStream` direct-finalize durable transcript materialization is proven by named deterministic behavior evidence showing accepted finalize writes exactly one durable assistant payload, duplicate finalize after cleared buffers is a no-op, stale/off-target finalize is a no-op, and `Uuid::nil()` resolves before acceptance
- [ ] any treatment of `ShowToolCall` / `UpdateToolCall` is explicit and does not silently invent new GPUI behavior
- [ ] bounded `ConversationCleared` behavior is proven by named deterministic behavior evidence showing the mounted clear-handling path (currently `src/ui_gpui/views/chat_view.rs::ChatView::handle_command(ViewCommand::ConversationCleared)`, or an evidence-mapped repo-idiomatic equivalent) restores from authoritative `current_snapshot()` within the same synchronous mounted update transaction, before control returns to the event loop, and without store revision change
- [ ] ignored/no-op/stale runtime commands are proven by named deterministic behavior evidence not to bump revision or publish
- [ ] named deterministic ingress proof would fail if a second production drainer or reentrant reduction path were reintroduced
- [ ] verification treats the `ViewCommand` payload-shape change as a serialized contract migration, not a local reducer tweak

## Success Criteria

- Verification evidence can trace select intent -> GPUI-owned selection boundary -> ordinary-runtime minting site -> enriched `UserEvent::SelectConversation { id, selection_generation }` -> presenter async load -> store mutation -> snapshot render
