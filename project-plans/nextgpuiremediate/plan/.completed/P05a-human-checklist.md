# P05a Human Verification Checklist

## Purpose

Collect the required manual end-to-end evidence for `PLAN-20260219-NEXTGPUIREMEDIATE.P05a` so `P05a.md` can be upgraded from `FAIL` to `PASS`.

## Preflight

```bash
cd /Users/acoliver/projects/personal-agent/gpuui
pkill -f personal_agent_gpui || true
cargo build --bin personal_agent_gpui
nohup cargo run --bin personal_agent_gpui >/tmp/personal_agent_gpui.log &
```

Confirm app process:

```bash
osascript -e 'tell application "System Events" to (name of processes) contains "personal_agent_gpui"'
# expected: true
```

## Manual Checks (Required)

Run these checks with direct UI interaction (mouse/keyboard), then record expected vs actual.

### 1) Settings -> Model Selector -> Results load

- Open tray popup.
- Navigate to Settings.
- Click Add Profile (+) to open Model Selector.
- Verify model list/results appear (not blank forever).

Expected:
- Model Selector view opens.
- Models are visible and selectable.

Actual:
- Not yet executed by human operator in this session.

Result: FAIL

Verdict:
- [ ] PASS
- [x] FAIL

Quickmark for unlock helper:
- [ ] PASS
- [x] FAIL (set to PASS when this section is verified)

---

### 2) Return and trigger Profile Editor save path

- From Model Selector, choose a model.
- Verify Profile Editor opens.
- Trigger Save action.
- Verify return/navigation reaction is correct (typically back to Settings).

Expected:
- Profile Editor opens from model selection.
- Save action emits flow and returns to expected view.

Actual:
- Not yet executed by human operator in this session.

Result: FAIL

Verdict:
- [ ] PASS
- [x] FAIL

Quickmark for unlock helper:
- [ ] PASS
- [x] FAIL (set to PASS when this section is verified)

---

### 3) Trigger MCP add/configure path and verify command reactions

- In Settings, click Add MCP (+).
- Complete enough data to advance (`Next`) to MCP Configure.
- Trigger Save in MCP Configure.
- Verify expected navigation/reaction.

Expected:
- Add MCP -> Configure MCP navigation works.
- Save emits expected flow and returns appropriately.

Actual:
- Not yet executed by human operator in this session.

Result: FAIL

Verdict:
- [ ] PASS
- [x] FAIL

Quickmark for unlock helper:
- [ ] PASS
- [x] FAIL (set to PASS when this section is verified)

---

### 4) Send chat message and observe stream updates

- Return to Chat view.
- Send a short message.
- Verify streaming output appears and finalizes.

Expected:
- User message appears.
- Stream chunks/final response appear (or deterministic error surfaced).

Actual:
- Not yet executed by human operator in this session.

Result: FAIL

Verdict:
- [ ] PASS
- [x] FAIL

Quickmark for unlock helper:
- [ ] PASS
- [x] FAIL (set to PASS when this section is verified)

## Fast Completion Instructions

- For each section above:
  1) Replace `Actual: [fill in]` with observed behavior.
  2) Set exactly one checkbox in `Verdict`.
  3) Update `Result:` to either `Result: PASS` or `Result: FAIL`.
- Gate behavior:
  - `P05a-status.sh` / `P05a-unlock-gate.sh` treat a section as PASS if it has either:
    - `Result: PASS`, or
    - `- [x] PASS` in its Verdict block.

## Log Capture (Attach Evidence)

After manual checks:

```bash
tail -n 300 /tmp/personal_agent_gpui.log
```

Paste relevant lines into `P05a.md` under Manual End-to-End Verification.

## Final Gate Rule

Per `dev-docs/COORDINATING.md`, `P05a` can only be marked PASS if **all four manual checks pass** with explicit evidence.

If any one check fails or is unverified, keep `P05a` verdict as FAIL.


## Minimal Human Completion Command Set

If you want the shortest path to unblock P05a, run these commands in order:

```bash
cd /Users/acoliver/projects/personal-agent/gpuui

# 1) launch interactive manual flow (preferred)
project-plans/nextgpuiremediate/plan/.completed/P05a-complete-manual.sh

# 2) if running steps manually, edit checklist with real outcomes for all 4 sections
${EDITOR:-vi} project-plans/nextgpuiremediate/plan/.completed/P05a-human-checklist.md

# 3) lint checklist structure/content
project-plans/nextgpuiremediate/plan/.completed/P05a-checklist-lint.sh

# 4) enforce 4/4 PASS before promotion
project-plans/nextgpuiremediate/plan/.completed/P05a-unlock-gate.sh

# 5) confirm gate status
project-plans/nextgpuiremediate/plan/.completed/P05a-status.sh
```

Notes:
- `P05a-complete-manual.sh` and `P05a-log-capture.sh` are interactive by default.
- The `--allow-noninteractive --wait-seconds N` mode on log capture is for diagnostics only and does not replace manual validation.

Expected unblock condition:
- checklist lint exits `0`
- unlock gate exits `0`
- status shows checklist PASS sections `4 / 4`
