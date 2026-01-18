# MCP Init Reliability + Package Arguments Design

## Problem Statement

The ByteDance filesystem MCP exits immediately when required CLI args (such as `--allowed-directories`) are missing. PersonalAgent currently launches some stdio MCPs without required args, which leads to:

- MCP processes exiting quickly with errors on stderr.
- `initialize()` hanging because the stdio transport waits on a response that never arrives.
- MCP startup blocking other MCPs during init (until timeout), producing a confusing UX.

## Goals

- Surface required `packageArguments` from the registry and collect values in the configure UI.
- Validate required args before spawn to avoid avoidable launch failures.
- Ensure stdio transport detects process exit and unblocks in-flight requests.
- Keep MCP init non-blocking: failures should mark `Error` and continue.
- Preserve security: never interpolate user args into a shell string.

## Non-Goals

- Full configSchema rendering for Smithery MCPs (future work).
- Automated secret handling for package arguments (see Open Questions).
- Auto-disable policy decisions (defer to product discussion).

## Design Overview

### 1) Registry Parsing + Persistence

- Parse `packageArguments` from the official registry response.
- Store metadata in `McpConfig.package_args` (already in types).
- Persist user-provided values in `McpConfig.config["package_args"]` as a JSON map of string values.

Example config JSON:

```json
{
  "package_args": {
    "allowed-directories": "/Users/alice/Documents,/Users/alice/Downloads"
  }
}
```

### 2) Configure UI

- When a registry result includes `package_args`, show a field per arg.
- Required args must be non-empty on save.
- Display descriptions and defaults (if provided by registry).
- Input format for multi-values: comma-separated string (interpreted as repeated flags).

### 3) Command Construction

- Build command as `Vec<String>` (no shell interpolation).
- For each configured arg value:
  - **Named args**: append `--{name}` and the value.
  - **Positional args**: append the value as-is.
- Split comma-separated values into multiple entries. Empty segments are ignored.

### 4) Pre-Spawn Validation

- If any required `package_args` are missing values, fail fast:
  - set MCP status to `Error` with a helpful message.
  - skip spawning and continue init of other MCPs.

### 5) Stdio Transport Exit Handling (SerdesAI)

- When the stdio reader hits EOF:
  - mark transport disconnected.
  - clear all pending requests (dropping senders) so awaiting calls return immediately.
- Pending request failures should include stderr buffer if present.

### 6) Runtime Safeguards

- Existing init timeout remains (`MCP_INIT_TIMEOUT`).
- On init/list_tools error or timeout:
  - close the client to terminate child.
  - set MCP status to `Error`.
- Continue initializing remaining MCPs.

## Security & UX Notes

- **Shell injection**: avoid building a single command string; always use `Command::new` + `args`.
- **Secrets**: package args are not marked secret in registry. Do not log their values. If registry adds secret flags for args, we should route them through secrets storage.
- **Error visibility**: surface stderr in logs/UI where possible.

## Test Plan (TDD)

1. **Stdio transport exit**
   - Spawn a process that exits after reading one line.
   - Assert that `request()` returns an error without hanging (use `timeout`).
2. **Required package args validation**
   - Config with required args and no values should fail pre-spawn.
3. **Command builder args**
   - Named args produce `--flag value` pairs.
   - Comma-separated values expand to repeated flags.

## Open Questions

- Should package args allow structured values (JSON) beyond strings?
- Should required arg values be stored in secrets storage (if flagged as secret)?
- Should failed MCPs auto-disable after N failures?
- Do we need an explicit UI prompt when config is missing (instead of just error)?
