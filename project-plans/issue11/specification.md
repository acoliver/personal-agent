# Feature Specification: Reach 80% Meaningful Coverage

## Purpose

Raise the enforced workspace line coverage gate to a real passing state by adding meaningful behavioral tests that reduce release risk. This issue explicitly forbids structural padding and mock-theater coverage work.

## Architectural Decisions

- **Pattern**: behavioral integration and service-level contract testing
- **Technology Stack**: Rust, cargo, cargo-llvm-cov, tokio async runtime, tempfile-based persistence tests
- **Data Flow**: tests must drive public service/presenter/runtime boundaries and assert user-visible or persistence-visible outcomes
- **Integration Points**: existing service implementations, presenter flows, MCP runtime lifecycle, and persistence/migration behavior

## Constraints

1. Coverage gate is enforced by `xtask/src/main.rs` at `80.0%` line coverage.
2. Tests must satisfy `dev-docs/goodtests.md`.
3. Avoid new low-value `coverage_boost_*` style additions.
4. Prefer black-box or semantically strong gray-box tests using real temp storage, real event flows, and real public APIs.
5. Interaction assertions are only acceptable when the interaction itself is the external contract.
6. The work must iterate until `cargo coverage` passes locally and is expected to pass in CI.

## Current Baseline

Evidence from the latest merged-code coverage gate (`gh run view 23547067341 --job 68551252642 --log`):
- line coverage: `65.06% (14608/22453, missed 7845)`
- regions: `62.92%`
- functions: `67.18%`

## Highest-Value Coverage Areas

Large in-scope source files indicate where meaningful behavioral tests can move the gate most:
- `src/services/profile_impl.rs`
- `src/services/mcp_impl.rs`
- `src/presentation/chat_presenter.rs`
- `src/presentation/settings_presenter.rs`
- `src/services/chat_impl.rs`
- `src/services/conversation_impl.rs`
- `src/mcp/runtime.rs`
- `src/services/profile_migration.rs`

## Existing Test Landscape

The repo already contains strong behavioral tests in some areas, but also coverage-chasing tests. New work should extend the strong style, not the weak style.

Behaviorally promising existing suites include:
- `tests/chat_presenter_coverage_tests.rs`
- `tests/e2e_presenter_chat.rs`
- `tests/history_and_settings_presenter_tests.rs`
- `tests/conversation_message_persistence_tests.rs`
- `tests/mcp_runtime_*`

Suspicious low-value patterns already exist in some `coverage_boost_*` files and should not be copied.

## Integration Points (Mandatory)

### Existing code that must be exercised more deeply
- `src/services/chat_impl.rs`
- `src/presentation/chat_presenter.rs`
- `src/mcp/runtime.rs`
- `src/services/profile_impl.rs`
- `src/services/conversation_impl.rs`

### Existing code that may need small enabling refactors
- `xtask/src/main.rs` only if needed to make local coverage iteration reliable with rustup/Homebrew toolchain coexistence
- helper seams inside tests only where they strengthen behavioral coverage without weakening contracts

### Existing code to avoid extending in the wrong direction
- `tests/coverage_boost_non_gpui_tests.rs`
- `tests/coverage_final_boost_tests.rs`
- other purely structural or mock-heavy test patterns

## Success Criteria

1. `cargo coverage` passes locally on `issue11`.
2. Workspace line coverage is at least `80.00%`.
3. Added tests comply with `dev-docs/goodtests.md`.
4. Full verification passes before PR submission.
5. PR is opened only once local verification indicates it should pass.

## Verification Commands

```bash
cargo test --lib --tests
cargo coverage
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

## Follow-on Rule

If the first implementation batch does not reach 80%, create a follow-on plan under `project-plans/issue11/followups/` and repeat the loop until coverage passes.
