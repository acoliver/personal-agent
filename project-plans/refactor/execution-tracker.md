# Execution Tracker: Event-Driven Architecture Refactoring

## Plan Information

**Plan ID**: PLAN-20250125-REFACTOR
**Feature**: 5-Layer Event-Driven Architecture Refactoring
**Generated**: 2025-01-25
**Total Phases**: 16

---

## Execution Status

| Phase | ID | Title | Status | Started | Completed | Verified | Semantic? | Notes |
|-------|-----|-------|--------|---------|-----------|----------|-----------|-------|
| 0 | P0 | Preflight Verification | [ ] | - | - | - | N/A | Verify dependencies, types, modules |
| 1 | P1 | Event System Stub | [ ] | - | - | - | [ ] | Create EventBus skeleton |
| 2 | P2 | Event System TDD | [ ] | - | - | - | [ ] | Tests for EventBus |
| 3 | P3 | Event System Implementation | [ ] | - | - | - | [ ] | Implement EventBus |
| 4 | P4 | Service Layer Stub | [ ] | - | - | - | [ ] | Service trait stubs |
| 5 | P5 | Service Layer TDD | [ ] | - | - | - | [ ] | Service tests |
| 6 | P6 | Service Layer Implementation | [ ] | - | - | - | [ ] | Implement services |
| 7 | P7 | Presenter Layer Stub | [ ] | - | - | - | [ ] | Presenter stubs |
| 8 | P8 | Presenter Layer TDD | [ ] | - | - | - | [ ] | Presenter tests |
| 9 | P9 | Presenter Layer Implementation | [ ] | - | - | - | [ ] | Implement presenters |
| 10 | P10 | UI Integration | [ ] | - | - | - | [ ] | Refactor views to emit events |
| 11 | P11 | Integration Testing | [ ] | - | - | - | [ ] | End-to-end tests |
| 12 | P12 | Error Handling Integration | [ ] | - | - | - | [ ] | Error presenter |
| 13 | P13 | Migration & Data | [ ] | - | - | - | [ ] | Migrate conversations |
| 14 | P14 | Deprecation & Cleanup | [ ] | - | - | - | [ ] | Remove old code |
| 15 | P15 | Performance Verification | [ ] | - | - | - | [ ] | Verify no regression |
| 16 | P16 | Documentation & Examples | [ ] | - | - | - | [ ] | Update docs |

---

## Completion Markers

### Global Completion Criteria

- [ ] All phases have @plan markers in code
- [ ] All requirements have @requirement markers
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings
- [ ] No phases skipped (P0 → P1 → P2 → ... → P16)

### Component Completion Status

#### Event System (src/events/)
- [ ] EventBus implemented with tokio::sync::broadcast
- [ ] All event types defined (AppEvent hierarchy)
- [ ] Global emit() and subscribe() functions
- [ ] Event logging with tracing
- [ ] Tests for multi-subscriber scenarios

#### Service Layer (src/services/)
- [ ] ConversationService trait + impl
- [ ] ProfileService trait + impl
- [ ] ChatService trait + impl
- [ ] McpService trait + impl
- [ ] McpRegistryService trait + impl
- [ ] SecretsService trait + impl
- [ ] ModelsRegistryService trait + impl
- [ ] AppSettingsService trait + impl

#### Presenter Layer (src/presentation/)
- [ ] ChatPresenter
- [ ] SettingsPresenter
- [ ] HistoryPresenter
- [ ] ProfileEditorPresenter
- [ ] McpAddPresenter
- [ ] McpConfigurePresenter
- [ ] ModelSelectorPresenter
- [ ] ErrorPresenter

#### UI Layer (src/ui/)
- [ ] ChatView refactored (<500 lines)
- [ ] SettingsView refactored (<500 lines)
- [ ] HistoryView refactored
- [ ] ProfileEditorView refactored
- [ ] McpAddView refactored
- [ ] McpConfigureView refactored
- [ ] ModelSelectorView refactored

---

## Phase-by-Phase Tracking

### Phase 0: Preflight Verification

**Goal**: Verify all assumptions before implementation

**Checks**:
- [ ] All dependencies verified (tokio, serdes-ai, thiserror, etc.)
- [ ] All types match expectations (Conversation, Profile, McpConfig, etc.)
- [ ] All module paths are valid
- [ ] Test infrastructure ready
- [ ] Current codebase builds

**Output**: `project-plans/refactor/.completed/P0.md`

---

### Phase 1: Event System Stub

**Goal**: Create EventBus skeleton that compiles

**Files**:
- `src/events/mod.rs`
- `src/events/bus.rs`
- `src/events/types.rs`

**Markers**:
- `@plan:PLAN-20250125-REFACTOR.P01`
- `@requirement:EV-001` (EventBus exists)

**Output**: `project-plans/refactor/.completed/P1.md`

---

### Phase 2: Event System TDD

**Goal**: Write comprehensive tests for EventBus

**Files**:
- `src/events/bus.rs` (tests module)

**Markers**:
- `@plan:PLAN-20250125-REFACTOR.P02`
- `@requirement:EV-T1` through `EV-T8`

**Output**: `project-plans/refactor/.completed/P2.md`

---

### Phase 3: Event System Implementation

**Goal**: Implement EventBus to pass all tests

**Files**:
- `src/events/bus.rs`

**Markers**:
- `@plan:PLAN-20250125-REFACTOR.P03`
- `@pseudocode:events/bus.md lines X-Y`

**Output**: `project-plans/refactor/.completed/P3.md`

---

### Phase 4: Service Layer Stub

**Goal**: Create service trait stubs

**Files**:
- `src/services/mod.rs`
- `src/services/conversation.rs`
- `src/services/profile.rs`
- `src/services/chat.rs`
- `src/services/mcp.rs`
- `src/services/mcp_registry.rs`
- `src/services/secrets.rs`
- `src/services/models_registry.rs`
- `src/services/app_settings.rs`

**Markers**:
- `@plan:PLAN-20250125-REFACTOR.P04`

**Output**: `project-plans/refactor/.completed/P4.md`

---

### Phase 5: Service Layer TDD

**Goal**: Write service tests

**Files**:
- `tests/services_test.rs`

**Markers**:
- `@plan:PLAN-20250125-REFACTOR.P05`

**Output**: `project-plans/refactor/.completed/P5.md`

---

### Phase 6: Service Layer Implementation

**Goal**: Implement services

**Markers**:
- `@plan:PLAN-20250125-REFACTOR.P06`
- `@pseudocode:services/*.md`

**Output**: `project-plans/refactor/.completed/P6.md`

---

### Phase 7-9: Presenter Layer

**Phases**: 07 (stub), 08 (tdd), 09 (impl)

**Output**: `project-plans/refactor/.completed/P7.md`, `P8.md`, `P9.md`

---

### Phase 10: UI Integration

**Goal**: Refactor views to emit UserEvents

**Files Modified**:
- `src/ui/chat_view.rs` (reduce to <500 lines)
- `src/ui/settings_view.rs` (reduce to <500 lines)
- Other view files

**Markers**:
- `@plan:PLAN-20250125-REFACTOR.P10`

**Output**: `project-plans/refactor/.completed/P10.md`

---

### Phase 11-16: Remaining Phases

Detailed tracking in individual phase files.

---

## Notes Section

### 2025-01-25
- Created refactoring plan structure
- This is a comprehensive refactoring affecting the entire application
- Must maintain backward compatibility with existing data
- Testing strategy is critical due to complexity

### Migration Notes

**Conversation Schema Changes**:
- Remove `profile_id` from Conversation
- Add `model_id`, `cancelled`, `tool_calls` to Message
- Migrate existing conversations on first run

**Config Changes**:
- Add `active_conversation_id` to config
- Migrate existing config.json

---

## Verification Summary

### Build Verification

```bash
# Run after each implementation phase
cargo build --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

### Test Verification

```bash
# Run all tests
cargo test --all-targets

# Integration tests
cargo test --test integration_test
```

---

## Plan Evaluation

Before starting implementation, this plan was evaluated for:

- [x] Integration analysis completed
- [x] All pseudocode files include numbered lines
- [x] Implementation phases reference pseudocode
- [x] No reverse testing patterns
- [x] All files will be UPDATED, not duplicated
- [x] Behavioral contract verification included

**Evaluation Result**: PASSED
**Builds in Isolation**: NO (integrates with existing code)
**Has User Access**: YES (through existing UI)
**Integration Points**: Identified in specification
