# Issue #32 Remediation Plan: Tool Approval Policy + MCP Identifier Semantics (Exact)

## Objective

Remediate the implementation plan for issue #32 so it exactly preserves issue semantics and aligns with identifiers and wiring that actually exist in this repository, while keeping evaluation deterministic and pure.

This plan defines:

- Exact decision outcomes: `Allow | Deny | AskUser`
- Exact policy model fields and Rust types required by issue #32
- MCP identity resolution using existing runtime structures (`McpTool`, `mcp_id`, `McpConnection`)
- Deterministic `auto_approve_reads` behavior with unchanged evaluate signature
- Concrete integration points in `McpToolExecutor` / `client_agent` flow
- Persistence and session lifecycle behavior using `AppSettingsService`
- Test-first implementation sequence with explicit precedence and fallback tests

## Scope and Non-Goals

### In Scope

- Policy decision engine for tool execution approval decisions
- Persistent allow/deny configuration + session allowlist behavior
- MCP tool identifier normalization and policy matching
- Integration boundary behavior when decision is `AskUser`
- Deterministic tests for matching, precedence, and fallback behaviors

### Out of Scope

- Renaming issue semantics or decision variants
- Introducing nonexistent runtime fields (for example, `McpToolCall.server`)
- Reworking tool execution architecture outside required wiring points
- Non-deterministic shell heuristics or broad shell parsing expansion

## Required Semantics (Must Remain Exact)

- Decision outcomes are exactly:
  - `Allow`
  - `Deny`
  - `AskUser`
- `evaluate` remains:
  - `evaluate(&self, tool_identifier: &str) -> ToolApprovalDecision`
- Issue #32 matching behavior and precedence remain intact.
- Any “prompt” terminology is replaced by `AskUser` as the sole interactive fallback state.

## Real Runtime Identifier Mapping (No Nonexistent Fields)

There is no `McpToolCall.server` field in the current codebase contract used by this plan.

Use codebase-real identifiers:

- Runtime tool metadata: `McpTool { name, mcp_id }`
- MCP connection lookup: `HashMap<Uuid, McpConnection>` where `McpConnection.config.name` is the server name

### MCP server identity resolution for policy matching

Given an MCP invocation, resolve canonical policy identifier from runtime objects:

1. Start from tool metadata (`McpTool`):
   - `tool_name = mcp_tool.name`
   - `mcp_uuid = mcp_tool.mcp_id`
2. Resolve connection:
   - `connections.get(&mcp_uuid)` -> `McpConnection`
3. Read server identity:
   - `server_name = mcp_connection.config.name`
4. Build canonical MCP identifier for policy evaluation:
   - `mcp.{server_name}.{tool_name}` (primary canonical form)

If lookup fails (no connection for `mcp_id`), fall back to deterministic unresolved form:
- `mcp.unknown.{tool_name}`

This keeps behavior deterministic and avoids inventing unavailable fields.

## Concrete ToolApprovalPolicy Data Model (Issue #32 Required Shape)

Define a concrete policy model with exact required fields and types:

- `always_allow: Vec<String>`
- `always_deny: Vec<String>`
- `session_allowlist: Vec<String>`
- `auto_approve_reads: bool`

Suggested Rust shape (for implementation reference only):

- `#[derive(Debug, Clone, Serialize, Deserialize, Default)]`
- struct `ToolApprovalPolicy` with the four fields above

Decision type:

- enum `ToolApprovalDecision { Allow, Deny, AskUser }`

No additional required fields are introduced in the persisted schema for issue #32 scope.

## Matching Rules and Precedence (Deterministic)

### Identifier normalization

All incoming identifiers are normalized before matching:

- trim whitespace
- lowercase for policy comparisons
- collapse internal accidental repeated separators only if existing codebase already does so; otherwise keep literal (do not over-normalize)

### Prefix semantics and intentional false-positive behavior

Policy entries are treated as prefix match tokens to preserve existing issue #32 semantics, including known intentional prefix false-positive behavior.

That means:

- an entry `mcp.github.read` matches `mcp.github.read_file`
- this can intentionally also match broader prefixes when configured that way
- tests explicitly lock this behavior to prevent accidental “tightening”

### Precedence (must be exact)

Evaluation order:

1. `always_deny` match => `Deny` (deny wins over everything)
2. `session_allowlist` match => `Allow`
3. `always_allow` match => `Allow`
4. `auto_approve_reads` recognized-read match => `Allow`
5. fallback => `AskUser`

This ensures explicit deny beats allow/yolo-style approvals.

## auto_approve_reads Clarification (Unchanged evaluate Signature)

`evaluate(&self, tool_identifier: &str)` remains unchanged.

Add deterministic internal helper(s), for example:

- `fn is_recognized_read_identifier(tool_identifier: &str) -> bool`

Behavior:

- If `auto_approve_reads == false`: helper result ignored
- If `auto_approve_reads == true`:
  - recognized read identifier => `Allow` (unless denied earlier by precedence)
  - unrecognized identifier => no auto-allow; continue to fallback (`AskUser` unless earlier allow/deny matched)

Recognized read set is deterministic and explicit (no runtime network lookups, no fuzzy inference).

## Shell Handling (Bounded/Deterministic)

Shell handling remains bounded and deterministic:

- No broad shell command introspection beyond explicit identifier matching rules in policy engine.
- If shell-specific classification is currently unwired in this repo path, keep it acknowledged as unwired and default to normal precedence flow (typically `AskUser` unless explicit allow/deny entry matches).

Do not add non-deterministic parsing or heuristic expansions.

## Integration Wiring in This Repo

### Primary wiring point

Integrate evaluation at the MCP tool execution boundary in the existing `McpToolExecutor` / `client_agent` path (where tool execution requests are translated into actual calls).

### Boundary behavior on decision outcomes

At execution boundary:

- `Allow` => execute tool immediately
- `Deny` => short-circuit execution and return denied outcome/error path per existing agent error conventions
- `AskUser` => do not execute automatically; invoke existing user-approval interaction boundary (or return approval-required signal to caller) and branch by user choice:
  - Always => mutate persistent allowlist (`always_allow`) then allow execution
  - Session => mutate in-memory session allowlist then allow execution for current conversation
  - Deny => do not execute

This keeps `evaluate` pure (no side effects in evaluation itself).

## Purity, Policy Access, and Persistence Separation

Keep `evaluate` pure:

- reads policy state from in-memory struct only
- no settings I/O or prompt I/O inside evaluate

Side effects happen only in integration/service layer:

- load/save policy via `AppSettingsService`
- update session allowlist in session-scoped memory holder
- user interaction handling at executor/agent boundary

## Session Allowlist Lifecycle

`session_allowlist` is in-memory only for active conversation scope.

Lifecycle requirements:

- initialized empty on new conversation
- cleared when conversation changes/reset event occurs
- trigger source must use existing “current conversation” state/events in repo (same event/state transitions currently used to establish active conversation context)

No persistence of session allowlist across conversation boundaries.

## Persistence Schema and Keys (Concrete)

Use `AppSettingsService` `get_setting` / `set_setting` with serde JSON serialization.

### Setting key

- `tool_approval_policy`

### Stored JSON object schema

{
  "always_allow": ["..."],
  "always_deny": ["..."],
  "auto_approve_reads": true
}

Notes:

- `session_allowlist` is intentionally excluded from persisted JSON.
- On load failure/absent setting: default policy (`always_allow=[]`, `always_deny=[]`, `auto_approve_reads=false`).

### Session storage location

- In-memory field owned by runtime/session context manager associated with current conversation.
- Not written via `set_setting`.

## Concrete Module/File Placement (Aligned to Repo Structure)

Place changes within existing policy/execution/service layers used by MCP tool execution path, concretely:

- Tool approval model/evaluator in the same module area where tool execution policy logic currently lives (or nearest shared policy module used by `client_agent`).
- Integration callsite in `McpToolExecutor` execution path (invoked before actual MCP tool dispatch).
- Settings adapter usage in existing application settings service layer (`AppSettingsService`) rather than inside evaluator.
- Session lifecycle hook in existing conversation context/state manager (current conversation change/reset event handler).

No new top-level architecture or unrelated module tree is introduced.

## Test-First Implementation Sequence (Required)

1. Add/adjust unit tests for evaluator behavior before implementation changes.
2. Add integration-level tests around `McpToolExecutor` boundary for `AskUser` and mutation paths.
3. Implement model + evaluator updates to satisfy unit tests.
4. Implement wiring/persistence/session lifecycle updates to satisfy integration tests.
5. Run full verification suite for repo conventions.

## Explicit Required Tests

### Evaluator precedence tests

- `always_deny` match returns `Deny` even when same identifier also matches:
  - `session_allowlist`
  - `always_allow`
  - recognized-read auto-approve
- `session_allowlist` beats `always_allow` ordering neutrality (both allow; decision remains `Allow`)
- no matches + no read auto-approve => `AskUser`

### auto_approve_reads tests

- recognized read identifier with flag on => `Allow`
- recognized read identifier with flag off => `AskUser` (unless explicit allow/deny)
- unrecognized identifier with flag on => `AskUser` (unless explicit allow/deny)

### AskUser fallback and integration tests

- `evaluate -> AskUser` causes executor boundary to request approval / return approval-required signal without executing tool
- user chooses Always => persistent `always_allow` updated via `set_setting`, subsequent evaluation allows
- user chooses Session => in-memory session allowlist updated, current conversation allows
- user chooses Deny => execution blocked

### MCP identifier resolution tests

- valid `mcp_id` lookup resolves `mcp.{server_name}.{tool_name}`
- missing connection lookup resolves deterministic fallback `mcp.unknown.{tool_name}`

### Prefix behavior lock tests

- intentional prefix false-positive behavior is preserved by test (documented with explicit case)
- deny-prefix still overrides allow-prefix in conflict

### Session lifecycle tests

- session allowlist present during active conversation
- cleared on new conversation trigger from existing conversation state/event path

### Persistence schema tests

- JSON stored/read via `tool_approval_policy` key with expected fields only
- missing/invalid persisted value falls back to default policy safely

## Verification Checklist

- `Allow | Deny | AskUser` only
- `evaluate(&self, tool_identifier: &str)` unchanged
- no references to nonexistent MCP fields
- deny precedence verified
- read auto-approve deterministic and scoped
- AskUser integration path verified
- Always/Session mutations correctly routed (persistent vs in-memory)
- session lifecycle reset on conversation change event
- settings JSON schema and key implemented exactly
- shell handling remains bounded/deterministic and acknowledged if unwired