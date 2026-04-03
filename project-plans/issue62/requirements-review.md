# Requirements Review: Issue #62 Markdown Rendering

Reviewed file: `project-plans/issue62/requirements.md`  
Reference spec: `project-plans/issue62/overview.md`  
Code cross-check files:
- `src/ui_gpui/components/message_bubble.rs`
- `src/ui_gpui/views/chat_view/render.rs`
- `src/ui_gpui/views/chat_view/mod.rs`
- `src/ui_gpui/views/chat_view/state.rs`
- `src/ui_gpui/theme.rs`
- `Cargo.toml`

Additional context checked:
- `dev-docs/RUST-RULES.md`
- GitHub Issue #62 body/comments (`gh issue view 62 --repo acoliver/personal-agent --json body,comments`)

---

## Critical Issues

1. **Requirements scope includes spec sections that are not actually normative requirements (risk of over-constraining implementation).**
   - `requirements.md` translates several explanatory/justification/spec-process sections into SHALL requirements (e.g., strict file-touch constraints and some wording around review findings).
   - Example: `REQ-MD-INTEGRATE-060` (“No other production files shall be modified in Phase A”) is much stricter than a typical technical requirement and can block legitimate refactors needed to satisfy behavioral outcomes.
   - Recommendation: demote process/implementation-boundary constraints to “implementation guidance” unless explicitly required by product behavior/architecture contract.

2. **Some requirements duplicate the same invariant in two IDs, causing potential contradiction drift.**
   - `REQ-MD-STREAM-011` and `REQ-MD-SEC-020` both normatively specify the same `assert!` char-boundary requirement, with one claiming “authoritative”.
   - This is traceable to spec cross-reference style, but in a requirements set it increases maintenance risk.
   - Recommendation: keep one normative requirement and convert the other to an explicit traceability alias/non-normative reference.

3. **A few requirements are not fully testable as written due to environment/measurement ambiguity.**
   - Perf requirements (`REQ-MD-PERF-001..004`) depend on baseline frame-time deltas but do not define exact measurement tooling hooks in code (where/how frame time sampled in GPUI test harness).
   - `REQ-MD-PERF-006` (“no unbounded memory growth”) is not bounded to a concrete test window/condition.
   - Recommendation: add measurable protocol details (sampling method, test harness API, duration/iterations, pass/fail math) in requirement text itself or a normative appendix.

4. **EARS syntax quality is mostly good, but a subset are not strict EARS pattern forms.**
   - Non-EARS examples include pure declaratives not using Ubiquitous template syntax consistently or compound conditions that blur pattern types.
   - Examples:
     - `REQ-MD-PARSE-061`, `062`, `063` (structural declarations could still be Ubiquitous but are phrased as mixed design declarations)
     - `REQ-MD-STREAM-037b/037c` are procedural sequence steps rather than clear EARS triggers
     - Some WHERE/WHILE combinations are valid in spirit but overloaded.
   - Recommendation: normalize to strict EARS forms with single trigger/state per requirement.

5. **Potential mismatch with current code reality for model labeling behavior and delegation path assumptions.**
   - Current code today:
     - `render.rs` completed messages render directly in `render_assistant_message` (raw text, click-to-copy)
     - streaming uses `AssistantBubble`
     - `AssistantBubble` currently appends cursor and renders raw string, includes optional “via {model_id}” label
   - Requirements correctly target desired future state, but a few assume exact shape of delegation and labels that may conflict with preserving current baseline visuals unless restated in behavioral terms.
   - Recommendation: ensure requirements focus on externally observable behavior rather than exact method-level sequence where not essential.

---

## Important Suggestions

1. **Completeness is strong but can be simplified by reducing micro-requirements.**
   - Coverage is very high; however, some tables split single behavior into multiple IDs (e.g., 010a/010b/010c, 037a/b/c) that read like implementation checklist steps rather than requirements.
   - Consolidate into one testable behavioral requirement per invariant.

2. **Traceability should explicitly mark non-normative source sections.**
   - `overview.md` contains rationale, review findings, and process notes. Requirements currently trace to these without distinguishing normative vs informative origin.
   - Add a “Normative Source?” column to avoid requirement creep.

3. **Testing requirements alignment with `RUST-RULES.md` is mostly good, but can be improved.**
   - Strengths: significant behavioral `#[gpui::test]` coverage and parser `#[test]` coverage; avoids pure mock theater.
   - Gap: several parser IR tests are structural; ensure they are paired with user-visible behavioral tests (link clicks, copy precedence, rendering outputs) as the confidence center.
   - Add explicit RED-GREEN expectation or “failing test first” note if this doc is used for implementation process conformance.

4. **Clarify Phase A vs Phase B boundaries around dependencies in one place.**
   - Good requirement exists (`REQ-MD-INTEGRATE-070`), but dependency requirements in other sections can be read as immediate unless phase-filtered.
   - Add a global rule: Phase B IDs are inactive unless gate passed.

5. **URL and HTML security requirements are strong and accurate.**
   - They correctly align with issue discussion and harden earlier weaker guidance.
   - Minor improvement: add explicit requirement for preserving link text display even when URL rejected (already present in `REQ-MD-SEC-003`, good).

---

## Minor Notes

1. **Terminology consistency:** use either “Phase A/B” everywhere or “Phase 1/2” everywhere (currently mixed in issue context vs requirements).
2. **Model label phrasing:** current code shows “via {model_id}” in `AssistantBubble` and plain model text in `render_assistant_message`; requirement should define exact desired user-visible label format once.
3. **Some priority labels (“Could/Should”) may be better moved to an implementation backlog** if this document is used as a strict acceptance gate.
4. **Requirements doc length/fragmentation:** consider grouping by acceptance test suite rather than parser event granularity for maintainability.

---

## EARS Format Correctness Summary

- **Overall:** Mostly EARS-compliant.
- **Strong patterns:** most event-driven (`WHEN`), unwanted (`IF...THEN`), state-driven (`WHILE`), optional (`WHERE`) forms are present and usable.
- **Issues found:**
  - Some requirements are procedural sequences rather than standalone EARS statements.
  - Some ubiquitous statements are declarative design constraints but not written in canonical “The [system] shall ...” form consistently.
  - A few combine multiple behaviors/triggers in one sentence, reducing testability.

Verdict on EARS quality: **Good but needs normalization pass for strict conformance.**

---

## Accuracy vs Current Codebase (cross-check)

Observed current state from code:
- No markdown renderer module currently in place.
- Assistant completed path: raw text in `render_assistant_message()` with bubble-level copy.
- Streaming path: `AssistantBubble`, still raw text + cursor.
- No mdstream fields in `ChatView` (`mod.rs`) currently.
- `Cargo.toml` currently has neither `pulldown-cmark`, `mdstream`, nor `url` dependencies for this feature.
- Theme exposes required accessors (`bg_base`, `bg_dark`, `bg_darker`, `text_primary`, `text_muted`, `accent`, `border`), so color-token requirements are realistic.

Conclusion: requirements generally describe target state accurately relative to current baseline and planned delta.

---

## Completeness Checklist (Spec Section Coverage)

| Spec Section | Coverage in requirements.md | Notes |
|---|---|---|
| Top-Level Preconditions | Adequate | Architecture + click strategy + phase gating represented |
| 1. Purpose & Problem | Adequate | Delegation/visual parity requirements present |
| 2. Functional Requirements | Strong | Block/inline/fallback/click behavior covered |
| 3. Technical Architecture | Strong | Two-phase IR and ownership boundaries covered |
| 4. GPUI API Usage & Verification | Partial | Behavior covered; procedural verification steps mostly not translated (acceptable) |
| 5. Component Design | Strong | parse/render/public API/module export covered |
| 6. Streaming (Conditional Draft) | Strong | Extensive Phase B requirement coverage |
| 7. Theme Integration | Strong | Token mapping and no hardcoded colors captured |
| 8. Security | Strong | URL allowlist + HTML stripping + UTF-8 assertion captured |
| 9. Error Handling | Partial-Strong | empty input, panic policy, malformed handling mostly covered |
| 10. Integration Points | Strong | store/presenter no-change constraints included |
| 11. Testing Strategy | Strong | unit + gpui + streaming lifecycle suites mapped |
| 12. Incremental Rollout | Adequate | phase isolation and telemetry included |
| 13. Out of Scope | Adequate | represented via absence + optional/could requirements |
| 14/15 CodeRabbit findings/traceability | Strong | many findings reflected explicitly |
| 16 Dependencies | Strong | `url` and phase B mdstream dependency requirements present |
| 17 Files Touched | Strong (possibly over-strong) | captured as hard constraints |
| 18 Risk Assessment | Partial | mitigations implied via tests/security/perf, not always explicit |
| 19 Performance Criteria | Strong | thresholds and protocol requirements included |
| 20/21 checklist/slices | Partial | transformed into detailed reqs; process steps mostly implicit |

Overall completeness vs spec: **High**.

---

## Alignment with `dev-docs/RUST-RULES.md`

- **Aligned:**
  - Strong testing emphasis (unit + gpui behavioral tests)
  - Real boundary behavior checks (link click, copy behavior, stream lifecycle)
  - Avoids mock-theater focus
- **Potential improvement:**
  - Explicitly prioritize behavioral outcomes over structural assertions in test wording
  - Add requirement that new behavior is integrated through real call paths (already implied by integration section, could be explicit)

Net: **Substantially aligned** with project testing philosophy.

---

## Verdict

**Needs revision before implementation planning finalization.**

The document is impressively comprehensive and mostly accurate, but should be revised to:
1. tighten strict EARS consistency,
2. remove/soften over-constraining process-like “shall” statements,
3. de-duplicate overlapping invariants,
4. sharpen testability for perf/memory requirements.

After that cleanup pass, it will be ready for implementation planning.