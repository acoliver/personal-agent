# Stabilization and Pruning Checkpoints

Each implementation batch must include a stabilization checkpoint before the batch is declared complete.

## Stabilization tasks

- remove temporary forwarding helpers that only existed during extraction
- collapse duplicate mapping helpers created during incremental moves
- remove dead render helpers and obsolete state adapters
- verify the extracted module ownership still makes sense
- confirm no module became a dumping ground named only by convenience

## Cohesion heuristic

An extracted file should not contain an unjustified mix of more than two of these concerns:
- command handling
- render implementation
- data model / state management

If a file still mixes more than two concerns, the phase evidence must explain why that is the least-bad structure.

## Batch-specific checkpoints

### P02a
- `chat_view` render/command/input helpers are settled into clear modules
- `main_panel` command routing and child forwarding are not split across duplicated helpers
- `route_view_command` has been treated as an explicit decomposition target, not left buried incidentally
- inline tests formerly in `main_panel.rs` are deliberately preserved, moved, or externalized

These P02a/P03a/P04a checkpoints are the stabilization phases referenced by the execution tracker; they are defined here rather than in separate per-batch documents.

### P03a
- settings/editor family shared logic is only extracted when at least two real call sites justify it
- no premature shared abstraction was introduced just to avoid repetition once

### P04a
- remaining cleanup leaves no lingering replacement god-files or temporary bridges
