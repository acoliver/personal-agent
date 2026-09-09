# Plan: MCP Add/Configure screens accept and persist API keys (issue #244)

Plan ID: PLAN-20260908-ISSUE244
Generated: 2026-09-08
Total Phases: 3
Requirements: ISSUE-244-INPUT, ISSUE-244-DROPDOWN, ISSUE-244-PASTE-ADD, ISSUE-244-PERSIST, ISSUE-244-HTTP-HEADER, ISSUE-244-CAN-SAVE

## Critical Reminders

1. Test-first: every phase below lists the failing tests to write before the implementation.
2. Mimic existing patterns exactly: `McpAddView` (IME/canvas/registry-dropdown/active-field)
   and `ApiKeyManagerView` (cmd-v paste, `sanitized_clipboard_text`, backspace/tab).
3. Fail fast: no fallback layers; secret-store failures abort the save with `ShowError`.
4. Plaintext API keys never enter `McpConfig` serialization; they go to the OS keychain via
   `SecretsManager::store_api_key_named` and travel to the presenter in the event payload.

## Phase 0.5: Preflight Verification (completed during analysis)

| Assumption | Verified at | Status |
|------------|-------------|--------|
| `McpConfigureView` renders static fields, no `EntityInputHandler` | `src/ui_gpui/views/mcp_configure_view/render.rs` | OK |
| `render_auth_method_section` has no `on_mouse_down`/menu | same file | OK |
| `McpConfigurePresenter::on_configure_mcp` sends `env: None` always | `src/presentation/mcp_configure_presenter.rs` | OK |
| `emit_save_mcp_config` hardcodes `auth_type: None`, drops values | `src/ui_gpui/views/mcp_configure_view/mod.rs` | OK |
| `McpAddView::handle_key_down` returns before handling cmd-v | `src/ui_gpui/views/mcp_add_view/mod.rs` | OK |
| Real HTTP header consumption is `McpRuntime::create_http_client` (`src/mcp/runtime.rs`), fed by `build_env_for_config` via keychain; `toolset::build_headers_for_config` only serves `create_toolset_from_config` | `src/mcp/runtime.rs`, `src/mcp/toolset.rs`, `src/mcp/manager.rs` | OK |
| Keychain naming: service `personal-agent`, key `mcp:{id}:{var_name}` (`mcp:{id}` = "default") | `src/services/secure_store.rs` `mcp_keys` | OK |
| `EnvVarConfig` has only `name`, `required` (needs `is_secret`) | `src/mcp/types.rs` | OK |
| Single-env-var runtime path loads `mcp:{id}` (default) while the spec stores named keys | `src/mcp/toolset.rs` `build_env_for_config` | MISMATCH — fix by loading named always |
| Test seam: `secure_store::use_mock_backend()` in-memory keychain used by `tests/mcp_toolset_tests.rs`, `tests/mcp_secrets_tests.rs` | tests | OK |
| Clipboard round-trips in `TestAppContext` (`cx.write_to_clipboard` → `read_from_clipboard`) | `src/ui_gpui/views/chat_view/message_selection_tests.rs` | OK |

Blocking issues: none.

## Phase 01: Failing tests (RED)

### ISSUE-244-INPUT: configure-screen text entry

**Behavior**:
- GIVEN: an `McpConfigureView` with `active_field = ApiKey`
- WHEN: cmd-v is pressed with `"sk-test-123\r\n"` on the clipboard
- THEN: `data.api_key == "sk-test-123"` (sanitized), mask toggle still display-only
- WHEN: backspace is pressed
- THEN: the last character is popped
- WHEN: tab is pressed repeatedly
- THEN: the active field cycles ApiKey → KeyfilePath → ApiKey (None starts at ApiKey)
- WHEN: IME `replace_text_in_range` inserts text
- THEN: text lands in the active field

Tests: `src/ui_gpui/views/mcp_configure_view/mod.rs` tests module
(`paste_populates_active_field_and_sanitizes`, `backspace_pops_last_character`,
`tab_cycles_active_fields`, `ime_replace_lands_in_active_field`).

### ISSUE-244-DROPDOWN: working AUTH METHOD dropdown

**Behavior**:
- GIVEN: the configure view
- WHEN: `toggle_auth_dropdown` runs → `show_auth_dropdown == true`, active field cleared
- WHEN: escape is pressed while open → dropdown closes and NO navigation is requested
- WHEN: a method is selected → `data.auth_method` changes, dropdown closes
- WHEN: method is API Key → `can_save` false until `api_key` non-empty; Key File → requires path

Tests: `auth_dropdown_state_transitions`, `selecting_auth_method_updates_can_save`.

### ISSUE-244-PERSIST: save payload carries auth + env + secrets

**Behavior**:
- GIVEN: draft with `auth_method = ApiKey`, `env_var_name = "EXA_API_KEY"`, env
  `[(EXA_API_KEY, "")]`, typed `api_key = "secret"`
- WHEN: `emit_save_mcp_config` runs
- THEN: event carries `auth_type: ApiKey`, `env_vars: [(EXA_API_KEY, required: true,
  is_secret: true)]`, `secrets: [(EXA_API_KEY, secret)]`
- GIVEN: ApiKey auth with NO env pairs → `env_vars` derived from `env_var_name`
- GIVEN: Keyfile auth with path → `keyfile_path: Some(path)`, `secrets` empty

Tests: `save_payload_carries_auth_env_and_secrets`,
`save_payload_derives_env_var_when_missing`, `save_payload_carries_keyfile_path`.

### ISSUE-244-PASTE-ADD: Add-screen paste

**Behavior**:
- GIVEN: `McpAddView` with active field ManualEntry / SearchQuery
- WHEN: cmd-v with `"npx foo\r\n"` on the clipboard
- THEN: field value is sanitized `"npx foo"`; no active field → no-op; SearchQuery paste
  clears the selected result and emits a registry search

Tests: `paste_appends_to_active_fields_sanitized_and_ignores_no_field`.

### ISSUE-244-PERSIST (presenter): draft env + keychain secrets

**Behavior**:
- GIVEN: app config on disk contains MCP `X` with `env_vars: [{name: EXA_API_KEY, required: true}]`
  and the service can resolve `X`
- WHEN: `ConfigureMcp { id: X }` is published
- THEN: draft carries `env: Some([(EXA_API_KEY, "")])` and `env_var_name = "EXA_API_KEY"`
- GIVEN: mock keychain backend active
- WHEN: `SaveMcpConfig` with `secrets: [(EXA_API_KEY, sk-123)]` is published
- THEN: `mcp_keys::get_named(saved_id, "EXA_API_KEY")` returns the value and the saved
  config JSON contains no plaintext secret

Tests: `tests/remaining_presenter_coverage_tests.rs`
(`mcp_configure_draft_loads_env_vars_from_app_config`,
`mcp_configure_save_stores_keychain_secrets`).

### ISSUE-244-HTTP-HEADER: toolset header builder

**Behavior**:
- GIVEN: Http transport + ApiKey auth + single env var with keychain secret
- WHEN: `build_headers_for_config(config, secrets)` runs
- THEN: headers contain `x-api-key: <secret>`; missing secret is an error (fail fast)

Tests: extended `tests/mcp_toolset_headers_tests.rs` (`build_headers_emits_x_api_key_for_http_api_key_auth`).

### Existing tests to update for the new event/type shapes

- `UserEvent::SaveMcpConfig` gains `secrets: Vec<(String, String)>` → update constructions in
  `tests/gpui_wiring_event_flow_tests.rs` (4), `tests/remaining_presenter_coverage_tests.rs` (3),
  and the configure-view tests (3).
- `EnvVarConfig` gains `is_secret: bool` (`#[serde(default)]`) → update constructions in
  `tests/mcp_toolset_tests.rs`, `tests/mcp_runtime_tests.rs`, `tests/mcp_runtime_flow_tests.rs`,
  `tests/coverage_boost_non_gpui_tests.rs`, `tests/mcp_registry_mapping_tests.rs`.
- `build_headers_for_config`/`build_env_for_config` signature/semantics → update
  `tests/mcp_toolset_tests.rs`, `tests/mcp_toolset_headers_tests.rs`, `tests/mcp_toolset_create_tests.rs`.

RED gate: `cargo test --lib --tests` fails with missing-field/missing-method errors and the
new assertion failures; no production code changed yet.

## Phase 02: Implementation (GREEN)

### Files to Modify

- `src/events/types.rs` — `SaveMcpConfig { id, config, secrets: Vec<(String, String)> }`.
- `src/mcp/types.rs` — `EnvVarConfig.is_secret: bool` with `#[serde(default)]`.
- `src/mcp/registry.rs` — map `is_secret: v.is_secret` from registry metadata.
- `src/mcp/toolset.rs` —
  `build_env_for_config`: ApiKey/Keyfile arms load `load_api_key_named(id, var.name)` for every
  var (single-var "default" special case removed; store and load sides now agree);
  `build_headers_for_config(config, secrets) -> McpResult<HashMap<String, String>>` emits
  `x-api-key` when transport is Http and auth is ApiKey (single env var), propagating keychain
  errors; `create_toolset_from_config` propagates.
- `src/presentation/mcp_configure_presenter.rs` —
  `on_configure_mcp`: look up the persisted `McpConfig` (override path or default path) and map
  `env_vars` into the draft (`env: Some([(name, "")])`, `env_var_name` = first name or
  "API_KEY"); absent metadata keeps the existing `env: None` contract;
  `on_save_config`: store each secret via `SecretsManager::store_api_key_named` before writing
  config; any failure → `ShowError("Save Failed")` and no config write.
- `src/ui_gpui/views/mcp_configure_view/mod.rs` — `ActiveField` enum, view fields
  (`active_field`, `show_auth_dropdown`, `ime_marked_byte_count`), paste/backspace/tab/escape
  handling with `(&mut self, event, window, cx)` signature, `sanitized_clipboard_text`,
  `select_auth_method`, `emit_save_mcp_config` (auth_type mapping, env vars with `is_secret`,
  derived env var for ApiKey, keyfile_path, secrets payload), draft-load state reset.
- `src/ui_gpui/views/mcp_configure_view/ime.rs` (new) — `EntityInputHandler` for
  `McpConfigureView`, structurally copied from `mcp_add_view/ime.rs`.
- `src/ui_gpui/views/mcp_configure_view/render.rs` — invisible canvas +
  `window.handle_input(ElementInputHandler::new(...))`, clickable API key / keyfile fields
  (accent border + caret when active), auth dropdown trigger (▼/▲) and backdrop+overlay menu,
  key-down listener passes window/cx.
- `src/ui_gpui/views/mcp_add_view/mod.rs` — cmd-v paste before the platform/control early
  return; sanitized append to the active field; search re-emitted for SearchQuery paste.

### Not changed (deliberate)

- `src/mcp/runtime.rs::create_http_client` — the real HTTP connection path already turns
  keychain-loaded env vars into headers (key-like names → `Authorization: Bearer`, others →
  `X-<VAR>`); extending `build_headers_for_config` per spec covers the toolset contract. The
  runtime behavior is documented in the final report.

## Phase 03: Verification

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --tests
```

All output logged under `tmp/verify244/`.

### Semantic checklist

- [ ] Configure screen: fields accept typing (IME) and paste; mask toggle display-only
- [ ] AUTH METHOD opens/selects/closes; escape closes the menu before navigating away
- [ ] Add screen Manual Entry + Search accept paste; sanitized
- [ ] Save: keychain holds `mcp:{id}:{var}`; config JSON holds names + is_secret only
- [ ] `can_save` still gates ApiKey on non-empty key, Keyfile on non-empty path
- [ ] Existing integration tests updated, full suite green
