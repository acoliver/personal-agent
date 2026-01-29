# Plan: Service Consolidation Refactor

Plan ID: PLAN-20250125-REFACTOR
Generated: 2025-01-25
Total Phases: 16
Requirements: [REQ-001 through REQ-034 from specification.md]

## Critical Reminders

Before implementing ANY phase, ensure you have:

1. Completed preflight verification (Phase 01)
2. Defined integration contracts for multi-component features
3. Written integration tests BEFORE unit tests
4. Verified all dependencies and types exist as assumed

## Requirements Summary

This plan implements the following requirements from the specification:

### Core Architecture (REQ-001 through REQ-005)
- REQ-001: EventBus for centralized event distribution
- REQ-002: Service trait hierarchy with lifecycle management
- REQ-003: RequestHandler trait for request/response operations
- REQ-004: ObservableService trait for metrics and status
- REQ-005: ServiceError standardization across all services

### Event System (REQ-019)
- REQ-019: EventBus implementation with tokio::sync::broadcast
- REQ-019.1: Centralized event distribution
- REQ-019.2: Typed event hierarchy (UserEvent, ChatEvent, McpEvent, SystemEvent)
- REQ-019.3: Event subscription and unsubscription
- REQ-019.4: Error handling in event handlers

### Service Layer (REQ-020 through REQ-024)
- REQ-020: Service module with business logic services
- REQ-020.1: ConversationService for conversation state
- REQ-020.2: ChatService for LLM interactions
- REQ-020.3: McpService for MCP connections
- REQ-020.4: ProfileService for user preferences
- REQ-020.5: SecretsService for secure credential storage
- REQ-021: Service lifecycle management
- REQ-022: Service health checks and metrics
- REQ-023: Service error handling and recovery
- REQ-024: Service initialization and shutdown

### Presenter Layer (REQ-025 through REQ-029)
- REQ-025: Presentation layer for UI coordination
- REQ-025.1: ChatPresenter for chat operations
- REQ-025.2: McpPresenter for MCP configuration
- REQ-025.3: SettingsPresenter for application settings
- REQ-025.4: ErrorPresenter for error display
- REQ-026: ViewCommand enum for UI updates
- REQ-027: Presenter-service integration
- REQ-028: Presenter event handling
- REQ-029: Presenter error handling

### Cross-Cutting Concerns (REQ-001 through REQ-005)
- REQ-001: Service trait hierarchy with lifecycle management
- REQ-002: Arc<Mutex<T>> for shared mutable state
- REQ-003: Arc<T> for read-only operations
- REQ-004: Configuration validation at startup
- REQ-005: Structured logging with service-level spans

### Integration and Migration (REQ-030 through REQ-034)
- REQ-030: UI layer integration with presenters
- REQ-031: Event-driven UI updates
- REQ-032: Backwards compatibility during migration
- REQ-033: Data migration and compatibility
- REQ-034: Deprecation and cleanup of legacy code

## Phase Overview

| Phase | Title | Status | Primary Requirements |
|-------|-------|--------|---------------------|
| 01 | Preflight Verification | Pending | Dependency verification, type checks |
| 01a | Preflight Verification Checklist | Pending | Verification of preflight checks |
| 02 | Analysis | Completed | Domain model, existing code analysis |
| 02a | Analysis Verification | Pending | Verification of analysis completeness |
| 03 | Pseudocode Development | Completed | Implementation pseudocode for all phases |
| 03a | Pseudocode Verification | Pending | Verification of pseudocode completeness |
| 04 | EventBus Stub | Pending | REQ-001, REQ-002, REQ-003 |
| 04a | EventBus Stub Verification | Pending | Verification of EventBus stub structure |
| 05 | EventBus TDD | Pending | REQ-001, REQ-002, REQ-003 |
| 05a | EventBus TDD Verification | Pending | Verification of EventBus test coverage |
| 06 | EventBus Implementation | Pending | REQ-001, REQ-002, REQ-003 |
| 06a | EventBus Implementation Verification | Pending | Verification of EventBus implementation |
| 07 | Service Layer Stub | Pending | REQ-001, REQ-004, REQ-005 |
| 07a | Service Layer Stub Verification | Pending | Verification of service stub structure |
| 08 | Service Layer TDD | Pending | REQ-001, REQ-004, REQ-005 |
| 08a | Service Layer TDD Verification | Pending | Verification of service test coverage |
| 09 | Service Layer Implementation | Pending | REQ-001, REQ-004, REQ-005 |
| 09a | Service Layer Implementation Verification | Pending | Verification of service implementation |
| 10 | Presenter Layer Stub | Pending | REQ-001, REQ-003, REQ-025 |
| 10a | Presenter Layer Stub Verification | Pending | Verification of presenter stub structure |
| 11 | Presenter Layer TDD | Pending | REQ-001, REQ-003, REQ-025 |
| 11a | Presenter Layer TDD Verification | Pending | Verification of presenter test coverage |
| 12 | Presenter Layer Implementation | Pending | REQ-001, REQ-003, REQ-025 |
| 12a | Presenter Layer Implementation Verification | Pending | Verification of presenter implementation |
| 13 | UI Integration | Pending | REQ-025, REQ-026, REQ-027, REQ-028 |
| 13a | UI Integration Verification | Pending | Verification of UI integration |
| 14 | Data Migration | Pending | REQ-026, REQ-027 |
| 14a | Data Migration Verification | Pending | Verification of data migration |
| 15 | Deprecation and Cleanup | Pending | All requirements |
| 15a | Deprecation Verification | Pending | Verification of cleanup completion |
| 16 | End-to-End Testing | Pending | All requirements |
| 16a | E2E Verification | Pending | Final verification of all requirements |

## Architecture Overview

This refactor introduces a **3-layer event-driven architecture**:

### Layer 1: Event System (Phases 04-06a)
- **EventBus**: Centralized event distribution using tokio::sync::broadcast
- **Event Types**: UserEvent, ChatEvent, McpEvent, SystemEvent
- **Implementation**: Stub → TDD → Implementation pattern

### Layer 2: Service Layer (Phases 07-09a)
- **ConversationService**: Manages conversation state and message history
- **ChatService**: Handles LLM interactions and streaming responses
- **McpService**: Manages MCP server connections and tools
- **ProfileService**: Handles user preferences and settings
- **SecretsService**: Secure credential storage
- **Implementation**: Stub → TDD → Implementation pattern

### Layer 3: Presenter Layer (Phases 10-12a)
- **ChatPresenter**: Coordinates chat operations between UI and services
- **McpPresenter**: Handles MCP configuration UI interactions
- **SettingsPresenter**: Manages application settings
- **ErrorPresenter**: Centralized error display
- **ViewCommand**: Commands for UI updates
- **Implementation**: Stub → TDD → Implementation pattern

### Integration & Testing (Phases 13-16a)
- **Phase 13**: UI integration with presenters
- **Phase 14**: Data migration and compatibility
- **Phase 15**: Deprecation and cleanup
- **Phase 16**: End-to-end testing and verification

## Development Strategy

Each major component (EventBus, Services, Presenters) follows a **3-phase pattern**:

1. **Stub Phase**: Create structure with `unimplemented!()` methods
2. **TDD Phase**: Write comprehensive tests first
3. **Implementation Phase**: Implement to pass tests

This ensures:
- Clear structure before implementation
- Test-driven development discipline
- Compilation at every phase
- Incremental verification

## Success Criteria

- All 16 phases completed in sequence (no skipped phases)
- All verification commands pass (structural + semantic)
- All requirements have @requirement markers in code
- All phases have @plan markers in code
- cargo build succeeds with no warnings
- cargo test passes with 80%+ coverage
- cargo clippy passes with no warnings
- No deferred implementation (no unimplemented!(), todo!(), etc.)

## Architecture Verification

The 3-layer architecture ensures:
- **Separation of Concerns**: UI (Presenters) ↔ Business Logic (Services) ↔ Events (EventBus)
- **Testability**: Each layer can be tested independently
- **Maintainability**: Clear boundaries between components
- **Scalability**: Event-driven design supports future growth

## Verification Strategy

Each phase includes:
1. **Structural Verification**: Files exist, markers present, code compiles
2. **Semantic Verification**: Behavior actually works, tests would fail without implementation
3. **Integration Verification**: Components call each other correctly
4. **Lifecycle Verification**: Initialization, shutdown, error handling work

## Execution Tracking

See `project-plans/refactor/execution-tracker.md` for detailed phase status tracking.

## Risk Mitigation

**Known Risks:**
- UI integration complexity (mitigated by 3-phase pattern: Stub → TDD → Implementation)
- Event system performance (mitigated by tokio::sync::broadcast and testing)
- Service state management (mitigated by comprehensive test coverage)
- Backwards compatibility (maintained during transition, legacy cleanup in Phase 15)

**Mitigation Strategies:**
- 3-phase development pattern ensures structure before implementation
- Incremental integration with backwards compatibility
- Feature flags for rollback capability
- Continuous integration testing at each phase
- Comprehensive test coverage before each phase completion
- Verification phases (a suffix) after each major phase

## References

- `project-plans/refactor/specification.md` - Full requirements specification
- `project-plans/refactor/analysis/` - Analysis phase outputs
- `project-plans/refactor/analysis/pseudocode/` - Pseudocode for all phases
- `dev-docs/PLAN-TEMPLATE.md` - Plan template and guidelines
- `dev-docs/architecture/` - Target architecture patterns
