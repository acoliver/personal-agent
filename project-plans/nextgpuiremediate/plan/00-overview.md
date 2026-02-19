# Plan: GPUI Final-Mile Wiring Remediation

Plan ID: PLAN-20260219-NEXTGPUIREMEDIATE
Generated: 2026-02-19
Total Phases: 17
Requirements: REQ-WIRE-001, REQ-WIRE-002, REQ-WIRE-003, REQ-WIRE-004, REQ-WIRE-005, REQ-WIRE-006, REQ-INT-001, REQ-INT-002

## Critical Reminders

Before implementing ANY phase, ensure you have:

1. Completed preflight verification (Phase 0.5)
2. Defined integration contracts for multi-component features
3. Written integration tests BEFORE unit tests
4. Verified all dependencies and types exist as assumed

## Scope (Active Runtime Only)

This plan applies only to active GPUI runtime paths:

- `src/main_gpui.rs`
- `src/ui_gpui/**/*`
- `src/presentation/**/*`
- `src/events/**/*`
- related services used by those paths

Legacy AppKit UI (`src/ui/**/*`) is out of scope.

## Core Defects Driving This Plan

1. Split event intake in `main_gpui.rs` (global EventBus vs local broadcast pathways)
2. Incomplete presenter output forwarding into bridge command stream
3. MainPanel drops most `ViewCommand` variants
4. UserEvent mismatch hotspots (`SaveProfileEditor`, `SaveMcp`, `McpAddNext`)
5. Placeholder presenters and service caveats causing fake interactivity

## Mandatory Sequence

`P0.5 -> P01 -> P01a -> P02 -> P02a -> P03 -> P03a -> P04 -> P04a -> P05 -> P05a -> P06 -> P06a -> P07 -> P07a -> P08 -> P08a`

No skipped phases permitted.

## Integration-First Execution Rule

Implementation is invalid unless integration phases (06/07/08 + verification) complete and pass.

## Pseudocode Mapping

- `analysis/pseudocode/component-001-event-pipeline.md`
- `analysis/pseudocode/component-002-main-panel-routing.md`
- `analysis/pseudocode/component-003-profile-flow.md`
- `analysis/pseudocode/component-004-conversation-flow.md`
- `analysis/pseudocode/component-005-mcp-flow.md`
- `analysis/pseudocode/component-006-settings-flow.md`

Phases 05–08 must cite pseudocode line ranges explicitly.
