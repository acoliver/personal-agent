# Phase 05: Final Verification

## Phase ID

`PLAN-20260325-ISSUE11B.P05`

## Objective

Prove that the refactoring removed the need for GPUI structural exemptions and left the codebase in a better state.

## Verification Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --tests
cargo coverage
python3 -m venv .venv-lizard
. .venv-lizard/bin/activate
python -m pip install --upgrade pip
python -m pip install lizard
python -m lizard -C 50 -L 100 -w src/ --exclude "src/main_gpui.rs" --exclude "src/bin/*" --exclude "src/services/chat.rs" --exclude "src/llm/client_agent.rs"
find src -name '*.rs' -print0 | xargs -0 wc -l | sort -n
```

## Scope note on remaining non-GPUI lizard excludes

Remaining excludes in the structural command (`src/main_gpui.rs`, `src/bin/*`, `src/services/chat.rs`, `src/llm/client_agent.rs`) are pre-existing non-GPUI exclusions outside this plan’s scope. This plan removes the GPUI-specific exclusions and does not add new ones.

## Additional hard constraint

No newly created file may be added to the coverage ignore regex in `xtask/src/main.rs`. Extracted GPUI modules must remain inside honest coverage accounting.

## Required final evidence

- `cargo coverage` result is recorded and any coverage delta versus baseline is explained honestly
- lizard output shows no touched-function violations
- file-length output shows no files above `1000` lines and no unexplained warnings above `750`
- `views/mod.rs` exports and downstream imports are stable or intentionally updated
- all source-text tests affected by moved code were replaced, narrowed, or removed with explicit rationale
- no extracted file became a replacement god-file
- maintainer handoff notes document the new module layout

## Success Criteria

- structural checks pass without GPUI view/component exemptions
- tests remain good tests under `dev-docs/goodtests.md`
- refactored files are materially smaller and more maintainable
- honest coverage remains accounted for and no extracted GPUI file was added to coverage-ignore handling
- follow-on coverage gaps are explicit if the workspace is still below the gate
- review loop has converged to pedantic-only issues or exhausted five rounds
