# Execution Tracker: ISSUE-168

## Status Summary

- Total phases: 3 (A–C)
- Completed: 0
- In progress: 0
- Remaining: 3
- Current phase: A (not started)

## Phase Dependencies

- Phase A → Phase B → Phase C
- Phase C requires Phase B command schema updates (`conversation_id` on tool-approval commands).

## Phase Status

| Phase | Description | Status | Verified | Evidence |
|---|---|---|---|---|
| A | Store-owned per-conversation streaming state + projection | PENDING | - | - |
| B | Approval conversation ownership plumbing | PENDING | - | - |
| C | ChatView per-conversation approval storage/rendering | PENDING | - | - |

## Verification Checklist

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test --lib --tests`
- [ ] `python -m lizard -C 50 -L 100 -w src/`

## CI / PR Loop Checklist

- [ ] Branch pushed: `issue168`
- [ ] PR created with title containing `(Fixes #168)`
- [ ] PR body includes summary + verification evidence
- [ ] `gh pr checks <PR_NUM> --watch --interval 300` completed with green checks
- [ ] CodeRabbit comments triaged, replied, and resolved when addressed/invalid
- [ ] Remediation reruns of full verification completed

## Notes / Remediation Log

- typescriptreviewer critique integrated into plan:
  - keep streaming map internal to `AppStoreInner`
  - project selected streaming snapshot for publish
  - add ownership-tagged tool approval request/resolve events
  - explicitly cover conversation deletion cleanup and switch-back projection tests