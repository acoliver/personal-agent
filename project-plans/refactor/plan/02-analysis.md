# Phase 02: Analysis Phase

## Phase ID

`PLAN-20250125-REFACTOR.P02`

## Prerequisites

- Required: Phase 01a (Preflight Verification Checklist) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P01A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/preflight-report.md`
  - `project-plans/refactor/preflight-verification-checklist.md`
- Preflight verification: Phases 01 and 01a completed successfully

## Purpose

Document the analysis phase that has already been completed. This phase:

1. Created the domain model for the refactoring
2. Analyzed existing code structure and dependencies
3. Identified service boundaries and responsibilities
4. Defined the target architecture
5. Created numbered pseudocode for implementation

**Note:** This phase is already complete. This file documents what was done.

## Requirements Implemented

This phase implements analysis requirements (no code requirements):

- **AN-001**: Domain model created with clear service boundaries
- **AN-002**: Existing code analyzed for extraction points
- **AN-003**: Target architecture defined with service layer hierarchy
- **AN-004**: Service traits and interfaces designed
- **AN-005**: Numbered pseudocode created for all implementation phases

## Implementation Tasks

### Files Already Created

- `project-plans/refactor/analysis/domain-model.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P02`
  - Domain entities and their relationships
  - Service boundaries and responsibilities
  - Data flow diagrams

- `project-plans/refactor/analysis/existing-code-analysis.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P02`
  - Current code structure analysis
  - Dependencies between modules
  - Extraction points identified

- `project-plans/refactor/analysis/target-architecture.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P02`
  - Service layer hierarchy
  - Service trait definitions
  - Integration patterns

- `project-plans/refactor/analysis/pseudocode/`
  - Directory containing numbered pseudocode for all phases
  - `01-utility-types.md` - Utility types and traits
  - `02-service-registry.md` - Service registry foundation
  - `03-llm-service.md` - LLM service implementation
  - `04-registry-service.md` - Registry service enhancement
  - `05-mcp-service.md` - MCP service consolidation
  - `06-agent-service.md` - Agent service completion
  - `07-ui-migration.md` - UI layer migration
  - `08-integration.md` - Integration and testing

### Analysis Completed

#### Domain Model Created

The domain model defines:

- **Service Layer Hierarchy**: Clear separation between application, service facade, core service, and runtime support layers
- **Service Traits**: Common interfaces for all services (Service, RequestHandler, ObservableService)
- **Service Registry**: Unified access point for all services
- **Error Handling**: Standardized ServiceError type
- **Configuration**: Service configuration patterns
- **Lifecycle**: Init, health check, shutdown patterns

#### Existing Code Analyzed

Key findings from existing code:

- **MCP Service**: Split between `mcp/service.rs` (singleton, tool routing) and `mcp/manager.rs` (lifecycle)
- **LLM Client**: Mixed concerns in `llm/client.rs` (API bridge, message handling, provider logic)
- **Agent Module**: Incomplete (awaiting SerdesAI PR #5)
- **Registry**: Good pattern to follow (cache with HTTP fetching)
- **Global Runtime**: Well-implemented in `agent/runtime.rs`

#### Target Architecture Defined

Four-layer architecture:

1. **Application Layer**: UI, main.rs, orchestration
2. **Service Facade Layer**: ServiceRegistry, high-level operations
3. **Core Service Layer**: McpService, LlmService, AgentService, RegistryService
4. **Runtime Support Layer**: GlobalRuntime, SecretsManager, ConfigStore, HttpClient

#### Pseudocode Created

Numbered pseudocode for all implementation phases:

1. Utility types and service traits
2. Service registry foundation
3. LLM service extraction
4. Registry service enhancement
5. MCP service consolidation
6. Agent service completion
7. UI layer migration
8. Integration and testing

## Verification Commands

### Structural Verification

```bash
# Check analysis files exist
ls -la project-plans/refactor/analysis/
# Expected: domain-model.md, existing-code-analysis.md, target-architecture.md

# Check pseudocode directory exists
ls -la project-plans/refactor/analysis/pseudocode/
# Expected: Multiple .md files with numbered names

# Check plan markers
grep -r "@plan:PLAN-20250125-REFACTOR.P02" project-plans/refactor/analysis/
# Expected: Multiple occurrences in analysis files

# Check domain model completeness
grep -E "Service|Entity|Repository" project-plans/refactor/analysis/domain-model.md | wc -l
# Expected: 10+ domain concepts defined

# Check pseudocode completeness
ls project-plans/refactor/analysis/pseudocode/*.md | wc -l
# Expected: 8+ pseudocode files (one per implementation phase)
```

### Semantic Verification (Manual)

- [ ] Domain model clearly defines service boundaries
- [ ] Existing code analysis identifies all extraction points
- [ ] Target architecture is clear and implementable
- [ ] Service traits are well-designed (init, health, shutdown)
- [ ] Pseudocode is detailed enough to implement from
- [ ] All pseudocode is numbered for traceability
- [ ] Pseudocode references requirements from specification

## Success Criteria

- Analysis files created and documented
- Domain model complete and clear
- Existing code analyzed thoroughly
- Target architecture defined
- Numbered pseudocode created for all phases
- All analysis files have plan markers

## Failure Recovery

If this phase fails (analysis incomplete):

1. Identify missing analysis components
2. Complete missing analysis
3. Re-run Phase 02a verification
4. Cannot proceed to Phase 03a until analysis complete

## Phase Completion Marker

Already created: `project-plans/refactor/plan/.completed/P02.md`

Contents:

```markdown
Phase: P02
Completed: [Date when analysis was completed]
Files Created:
  - analysis/domain-model.md
  - analysis/existing-code-analysis.md
  - analysis/target-architecture.md
  - analysis/pseudocode/ (multiple files)
Files Modified: None
Tests Added: None (analysis phase)
Verification: Analysis complete and documented
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 02a: Analysis Verification
2. Verify analysis completeness and correctness
3. Then proceed to Phase 03a: Pseudocode Verification
4. Then proceed to implementation phases (04+)

## Important Notes

This phase documents work that has already been completed. The analysis phase was done before creating the plan files. This documentation ensures traceability and verification of the analysis work.

## References

- `project-plans/refactor/analysis/domain-model.md` - Full domain model
- `project-plans/refactor/analysis/existing-code-analysis.md` - Code analysis results
- `project-plans/refactor/analysis/target-architecture.md` - Target architecture
- `project-plans/refactor/analysis/pseudocode/` - Numbered pseudocode
- `project-plans/refactor/specification.md` - Requirements specification
