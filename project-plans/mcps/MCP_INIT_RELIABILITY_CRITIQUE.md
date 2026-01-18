# MCP Init Reliability Design - Subagent Critique Summary

Source: rustreviewer subagent critique of MCP init/package args design.

## Key Risks

### Security
- **Shell injection risk** if args are interpolated into a single command string. Always pass args as a vector to `Command::new`.
- **Sensitive args** may be stored in plaintext if they live in `config`. If registry marks args as secret in the future, route them through secrets storage.

### Runtime Robustness
- **Zombie processes** if init times out but child remains running. Ensure `close()` is called on failure/timeouts.
- **Stderr buffer** should be bounded to avoid unbounded memory growth.
- **EOF handling** should clear pending requests so `.request()` returns promptly.

### UX
- If MCP auto-disables after repeated failures, user should be notified and able to re-enable.
- Required args should surface in the configure screen with descriptions/defaults.

## Suggested Improvements

- Validate stored arg values on load against registry metadata.
- Consider per-MCP timeouts or retry/backoff policies (optional).
- Add test cases for:
  - stderr capture on failure
  - timeout cleanup of child process
  - handling args with spaces or commas

## Test Ideas

- `request()` returns error after child exit and does not hang.
- `build_command` expands comma-separated values to repeated flags.
- Missing required args fail fast before spawn.
