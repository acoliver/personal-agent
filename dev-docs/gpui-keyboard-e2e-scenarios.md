# GPUI Keyboard-First E2E Scenario Suite (Real UI, No Backend Bypass)

This document defines executable-style, keyboard-first scenario contracts for `personal_agent_gpui`.

It is the canonical source for follow-on automation implementation and acceptance evidence.

---

## Scope

This suite is intentionally focused on the currently blocked product outcomes:

1. Profile can be created from real UI and appears in Settings profile list.
2. User can select/switch active profile from main Chat screen.
3. Chat model display/behavior is profile-driven (not hardcoded).
4. A real conversation of **no less than 5 user messages** proves context carryover using:
   - Profile name: `Synthetic Kimi 2.5 nvp4`
   - Model id: `moonshotai/kimi-k2-instruct-0905:nscale-nvp4`

---

## Non-Goals / Prohibited Shortcuts

- No `PA_AUTOMATE_KIMI_PROFILE` / backend seeding as final validation.
- No direct writes to profile/conversation files to satisfy acceptance.
- No presenter-only injection as substitute for UI interaction.

Real GPUI interaction path must be exercised end-to-end.

---

## Test Environment Contract

- Binary under test: `personal_agent_gpui`
- OS: macOS with Accessibility permissions granted for automation runner.
- Data roots used by app runtime:
  - `~/.llxprt/profiles`
  - `~/.llxprt/conversations`
  - `~/.llxprt/app_settings.json`
- Synthetic key path expected in scenario: `~/.keys/.synthetic_key`
  - If `~/.keys/.synethetic_key` is referenced anywhere, treat it as typo and fail scenario precheck.

---

## Current Keyboard Behavior Reference (as implemented)

These bindings are relied on by the scenario steps:

- Global/MainPanel:
  - `Ctrl+S` → navigate Settings
  - `Ctrl+H` → navigate History
  - `Cmd+W` → back
- Settings view:
  - `+` (Shift+=) → Model Selector
  - `e` → edit selected profile
  - `m` → add MCP
  - `Esc` / `Cmd+W` → Chat
- Model Selector view:
  - type chars/backspace → search query
  - `Esc` / `Cmd+W` → Settings
  - model row selection currently mouse-only in UI; keyboard automation may send click only where keyboard path is absent
- Profile Editor view:
  - field editing through active-field focus + keyboard input
  - `Cmd+S` → Save profile
  - `Esc` / `Cmd+W` → Settings
- Chat view:
  - typing + `Enter` sends message
  - `Cmd+,` → Settings
  - `Cmd+N` → New conversation
  - `Cmd+T` → Toggle thinking

---

## Scenario File Conventions

Each scenario below is written in:

- **Given**: preconditions
- **When**: keyboard-first operations
- **Then**: observable UI and artifact assertions
- **Evidence**: required logs/files/assert outputs to retain

---

## SCN-001: Create Synthetic Kimi profile from real UI and verify list presence

### Given

- App launched as `personal_agent_gpui`.
- No pre-existing profile named `Synthetic Kimi 2.5 nvp4`.
- Key file exists at `~/.keys/.synthetic_key`.

### When

1. Navigate to Settings (`Ctrl+S` or `Cmd+,` from Chat).
2. Trigger Add Profile (`+` in Settings).
3. In Model Selector, search for `moonshotai/kimi-k2-instruct-0905:nscale-nvp4`.
4. Select that model entry (keyboard-first; minimal click allowed only if row selection has no keyboard handler).
5. Transition to Profile Editor.
6. Ensure fields are set to:
   - Name: `Synthetic Kimi 2.5 nvp4`
   - Provider/API type consistent with selected model (OpenAI-compatible path where applicable)
   - Model ID: `moonshotai/kimi-k2-instruct-0905:nscale-nvp4`
   - Auth method: Keyfile
   - Keyfile path: `~/.keys/.synthetic_key`
   - Show thinking: enabled (as scenario default)
7. Save with `Cmd+S`.
8. Return to Settings list and refresh if needed.

### Then

- Settings profile list contains an entry named `Synthetic Kimi 2.5 nvp4`.
- Entry is selectable and remains present after leaving/re-entering Settings.
- No error notification is shown for save.

### Evidence

- Automation transcript with step-by-step key events.
- UI snapshot (accessibility tree/text) showing profile list row.
- Optional file evidence (non-authoritative alone): new profile JSON under `~/.llxprt/profiles`.

---

## SCN-002: Select/switch active profile from Chat screen

### Given

- At least two profiles exist:
  - `Synthetic Kimi 2.5 nvp4`
  - one non-Kimi profile (e.g. existing default)
- User is on Chat screen.

### When

1. Open Chat profile/model selector from title bar (keyboard path required by product; if absent, this scenario currently expected to fail and gate implementation).
2. Switch active profile to non-Kimi profile.
3. Verify model label updates.
4. Switch active profile back to `Synthetic Kimi 2.5 nvp4`.
5. Verify model label updates to Kimi model.

### Then

- Profile switch can be performed directly from Chat without navigating to Settings.
- Active model label in Chat reflects selected profile (not static/hardcoded).
- Subsequent assistant messages are tagged/rendered using active profile model identity.

### Evidence

- Accessibility snapshots before/after each switch.
- Command/event log showing profile selection event path.
- Message render evidence showing model label changed across switches.

---

## SCN-003: 5+ message context carryover conversation on Synthetic Kimi profile

### Given

- Active chat profile is `Synthetic Kimi 2.5 nvp4`.
- Model label shows Kimi model identity in Chat title area.
- Conversation starts clean (`Cmd+N` if needed).

### When

Send these **five user messages minimum** in one conversation (verbatim or semantically equivalent):

1. `Remember this codeword for later: ORBIT-731.`
2. `Summarize the codeword format in one sentence.`
3. `Now give me two bullet points that use that codeword naturally.`
4. `What was the exact codeword I asked you to remember?`
5. `Answer again with only the codeword and nothing else.`

### Then

- Assistant responses complete for each turn.
- Turn 4 and 5 correctly reference `ORBIT-731`, demonstrating context retention.
- Conversation includes at least five user turns and corresponding assistant outputs.

### Evidence

- Transcript with turn index, role, content.
- Stored conversation artifact showing all turns under one conversation id.
- UI snapshot showing final response with exact codeword.

---

## SCN-004: Regression guard — hardcoded model label is not used

### Given

- Active profile is `Synthetic Kimi 2.5 nvp4`.

### When

1. Open Chat.
2. Start new conversation.
3. Send one message.

### Then

- Chat title/model label is **not** `claude-sonnet-4` unless the selected profile actually uses that model.
- Assistant message model tag aligns with selected profile model.

### Evidence

- Snapshot of title bar model label.
- Snapshot of assistant message model tag.

---

## SCN-005: Profile lifecycle sync in Settings list

### Given

- A profile exists and is visible in Settings.

### When

1. Edit selected profile name in Profile Editor, save.
2. Return to Settings.
3. Delete selected profile.
4. Change default profile when multiple profiles remain.

### Then

- Settings list reflects update/delete/default changes without stale rows.
- Selection state remains valid after each operation.

### Evidence

- Before/after snapshots of Settings profile list.
- Command trace showing handling of `ProfileUpdated`, `ProfileDeleted`, `DefaultProfileChanged`.

---

## Required Automation Output Format

For each scenario run, automation must emit:

```text
SCENARIO: SCN-00X
STATUS: PASS|FAIL
STEPS_RUN: <count>
ASSERTIONS_PASSED: <count>
ASSERTIONS_FAILED: <count>
ARTIFACTS:
  - <path1>
  - <path2>
NOTES:
  - <short diagnostic>
```

---

## Acceptance Gate for This Workstream

Final acceptance is satisfied only when:

1. `SCN-001` PASS
2. `SCN-002` PASS
3. `SCN-003` PASS (with >= 5 user messages)
4. `SCN-004` PASS
5. Artifacts retained and reviewable

If any of the above fails, workstream remains incomplete.

---

## Implementation Notes for Follow-on Tasks

- Existing `tests/ui_automation_tests.rs` is menubar/popover-oriented and must be superseded for GPUI binary coverage.
- Prefer deterministic keyboard scripts with bounded waits and explicit assertion checkpoints over ad hoc sleeps.
- Keep scenario IDs stable; scripts should map 1:1 to scenario sections above.
