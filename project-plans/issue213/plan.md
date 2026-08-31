# Issue #213 — Chat stream killed by 2-minute total HTTP timeout

Streaming chat requests apply reqwest's per-request `.timeout(Duration::from_mins(2))`.
In reqwest 0.12 that is a *total* deadline that keeps ticking while the SSE body
streams, so any turn generating longer than 120s (thinking models on large contexts)
is killed by the client mid-generation, then the error is displayed as the misleading
`error decoding response body`.

## Goal

Replace the 2-minute total deadline on streaming requests with idle-read semantics:
the stream may run as long as chunks keep arriving; it only fails when *no bytes
arrive* for a larger default window (5 minutes).

## Acceptance criteria

- [ ] `NormalizingSseModel::request_stream` no longer applies a per-request total
      `.timeout()` to the streaming POST.
- [ ] The wrapper's HTTP client is built with reqwest `read_timeout` (idle) using a
      shared default constant of 5 minutes; the constant is exported for call sites.
- [ ] Idle enforcement is owned by the wrapper (constructor applies it from config),
      so production wiring cannot forget it.
- [ ] A stream whose chunks keep arriving survives past any total deadline
      (regression test: `ModelSettings.timeout` set to a short value must NOT kill an
      actively streaming response).
- [ ] A stream that goes silent beyond the idle window fails with an error.
- [ ] Quirks default headers (e.g. User-Agent overrides) still reach the streaming
      client (openai-quirks path passes the header map through config).
- [ ] Non-streaming behavior unchanged (inner model keeps its 2-minute
      `ExtendedModelConfig` timeout for title generation etc.).
- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test --lib --tests` all pass.

## Design

### `src/llm/normalizing_model.rs`

- `NormalizingSseModelConfig`: drop `client: Client`; add
  `default_headers: reqwest::header::HeaderMap` and `idle_read_timeout: Duration`.
- `NormalizingSseModel::new` returns `Result<Self, LlmError>` and builds its own
  client: `Client::builder().default_headers(headers).read_timeout(idle).build()`.
- Add `pub(crate) const DEFAULT_STREAM_IDLE_READ_TIMEOUT: Duration =
  Duration::from_mins(5);`
- `request_stream`: remove the `settings.timeout.unwrap_or(default_timeout)` logic and
  the `.timeout(...)` call. reqwest `read_timeout` covers the connect + headers phase
  and resets on every successful body read (verified in vendored reqwest 0.12.28
  `async_impl/client.rs` / `async_impl/body.rs`).
- Remove the now-dead `default_timeout` field and its Debug entry.

### `src/llm/client.rs`

- `build_model`: pass `default_headers: HeaderMap::new()` and
  `idle_read_timeout: DEFAULT_STREAM_IDLE_READ_TIMEOUT` into the wrapper config; map
  client-build failure to `LlmError::InvalidConfig` like existing sites.
- `build_openai_model_with_quirks`: inner model keeps the quirks-header client; the
  wrapper receives `quirks_header_map().unwrap_or_default()` + the idle constant.

### Tests (same file, `#[cfg(test)]`)

- `stream_survives_total_deadline_while_chunks_keep_arriving` — the red test. Serves
  SSE from a local `tiny_http` server (chunks every 150ms, total ~1.2s), passes
  `ModelSettings { timeout: Some(400ms) }`, idle set to 5s. Old code aborts at 400ms;
  new code must stream to completion.
- `stream_fails_when_silent_beyond_idle_read_timeout` — server sends one chunk then
  stalls 1s with idle 200ms; the stream must yield an error.
- `default_idle_read_timeout_is_five_minutes` — pins the larger default.

## Out of scope (tracked in the issue)

- Persisting partial output on stream failure.
- Retrying retryable stream errors (timeout / 429 / 5xx).
- Tool-result blob capping before history replay.
