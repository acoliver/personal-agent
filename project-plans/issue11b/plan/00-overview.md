# PLAN-20260325-ISSUE11B Overview

## Intent

The project should stop hiding its worst GPUI code behind CI exclusions. This plan removes those exemptions and drives a test-first refactoring campaign to break oversized GPUI files into maintainable units with real behavioral protection.

## CI Structural Thresholds

These are the active thresholds the implementation must satisfy after the exemptions are removed:
- warning above `750` lines per file
- failure above `1000` lines per file
- lizard cyclomatic complexity threshold `-C 50`
- lizard function length threshold `-L 100`

## Core principles

- test-first over churn-first
- behavior over wiring theater
- coherent seams over random file splitting
- maintain public module stability where practical via re-exports
- evidence-driven phase completion
- no replacement god-files

## Delivery strategy

Preferred execution order:
1. Batch A: `chat_view.rs` + `main_panel.rs`
2. Batch B: `profile_editor_view.rs` + `settings_view.rs` + `mcp_add_view.rs`
3. Batch C: `mcp_configure_view.rs` + `model_selector_view.rs` + `api_key_manager_view.rs`
4. `history_view.rs` only if lizard cleanup or shared-contract changes justify touching it

A batch may be implemented as its own PR if that keeps review and rollback manageable.

## What must happen before code movement

For every target file family, Phase 01 must produce:
- a decomposition map grounded in the real file structure
- a behavior contract inventory
- a public API / re-export plan
- a source-text-test remediation plan for `include_str!()` tests referencing that file
- a green-before-move safety-net plan for the first extraction step

These extraction targets are preliminary until Phase 01 produces grounded maps from the actual file structure. If the grounded analysis materially changes the grouping, update the later phase docs before implementation begins.

## Definition of done

- no GPUI structural exemptions remain in CI
- worst giant files are decomposed into coherent modules
- touched files comply with active thresholds
- extracted files do not become replacement god-files
- source-text tests broken by decomposition are deliberately replaced, narrowed, or removed with justification
- `cargo coverage` still passes
- review feedback is only pedantic or review rounds are exhausted
