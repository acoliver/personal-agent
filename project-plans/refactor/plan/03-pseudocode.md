# Phase 03: Pseudocode Phase

## Phase ID

`PLAN-20250125-REFACTOR.P03`

## Prerequisites

- Required: Phase 02a (Analysis Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P02A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/analysis-verification-report.md`
- Preflight verification: Phases 01, 01a, 02, 02a completed

## Purpose

Document the pseudocode phase that has already been completed. This phase:

1. Created detailed, numbered pseudocode for all implementation phases
2. Mapped pseudocode to requirements from specification
3. Provided step-by-step implementation guidance
4. Included verification steps for each phase

**Note:** This phase is already complete. This file documents what was done.

## Requirements Implemented

This phase implements pseudocode requirements (no code requirements):

- **PC-001**: Numbered pseudocode created for all implementation phases
- **PC-002**: Pseudocode mapped to requirements (REQ-XXX)
- **PC-003**: Implementation steps detailed and sequential
- **PC-004**: Verification steps included for each phase
- **PC-005**: Pseudocode is detailed enough to implement directly

## Implementation Tasks

### Files Already Created

- `project-plans/refactor/analysis/pseudocode/01-utility-types.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P03`
  - Pseudocode for utility types and service traits
  - Implements: REQ-001 through REQ-005

- `project-plans/refactor/analysis/pseudocode/02-service-registry.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P03`
  - Pseudocode for ServiceRegistry foundation
  - Implements: REQ-002, REQ-010, REQ-017

- `project-plans/refactor/analysis/pseudocode/03-llm-service.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P03`
  - Pseudocode for LlmService implementation
  - Implements: REQ-007, REQ-012, REQ-018

- `project-plans/refactor/analysis/pseudocode/04-registry-service.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P03`
  - Pseudocode for RegistryService enhancement
  - Implements: REQ-009, REQ-013, REQ-014

- `project-plans/refactor/analysis/pseudocode/05-mcp-service.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P03`
  - Pseudocode for McpService consolidation
  - Implements: REQ-006, REQ-011, REQ-016

- `project-plans/refactor/analysis/pseudocode/06-agent-service.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P03`
  - Pseudocode for AgentService completion
  - Implements: REQ-008 (pending SerdesAI)

- `project-plans/refactor/analysis/pseudocode/07-ui-migration.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P03`
  - Pseudocode for UI layer migration
  - Implements: REQ-025, REQ-026, REQ-027, REQ-028

- `project-plans/refactor/analysis/pseudocode/08-integration.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P03`
  - Pseudocode for integration and testing
  - Implements: Integration of all services

### Pseudocode Structure

Each pseudocode file follows this structure:

```markdown
# Pseudocode NN: [Title]

## Phase Reference
Phase: [NN]
Plan ID: PLAN-20250125-REFACTOR.P03

## Requirements Implemented
- REQ-XXX: [Requirement title]

## Implementation Steps

### Step 1: [Title]
[Detailed pseudocode with line numbers]

### Step 2: [Title]
[Detailed pseudocode with line numbers]

[...]

## Verification Steps
1. [Verification step]
2. [Verification step]
[...]
```

## Pseudocode Created

### 01-utility-types.md

**Purpose:** Define utility types and service traits used across all services

**Key Components:**
- Service trait (init, health_check, shutdown)
- RequestHandler trait (handle request/response)
- ObservableService trait (metrics, status)
- ServiceError enum (standardized errors)
- ServiceMetrics struct (metrics collection)

**Requirements:** REQ-001, REQ-003, REQ-004, REQ-005

### 02-service-registry.md

**Purpose:** Create ServiceRegistry to unify service access

**Key Components:**
- ServiceRegistry struct
- Service initialization (init_all)
- Service accessors (mcp(), llm(), agent())
- Health check coordination
- Shutdown coordination

**Requirements:** REQ-002, REQ-010, REQ-017

### 03-llm-service.md

**Purpose:** Extract LLM functionality from client.rs into LlmService

**Key Components:**
- LlmService struct
- Model profile management
- API key management
- Streaming and non-streaming requests
- Provider detection
- Request queuing and throttling

**Requirements:** REQ-007, REQ-012, REQ-018

### 04-registry-service.md

**Purpose:** Enhance registry module with service interface

**Key Components:**
- RegistryService struct
- Cache management with TTL
- Provider information
- Health check (freshness)
- Metrics (cache hit rate)

**Requirements:** REQ-009, REQ-013, REQ-014

### 05-mcp-service.md

**Purpose:** Consolidate MCP service from manager.rs and service.rs

**Key Components:**
- McpService struct
- Lifecycle management (spawn, stop, cleanup)
- Tool routing and aggregation
- Health monitoring
- Metrics collection

**Requirements:** REQ-006, REQ-011, REQ-016

### 06-agent-service.md

**Purpose:** Complete AgentService with SerdesAI integration

**Key Components:**
- AgentService struct
- SerdesAI Agent wrapper
- MCP toolset integration
- Conversation management
- Tool execution loop

**Requirements:** REQ-008 (pending SerdesAI PR #5)

### 07-ui-migration.md

**Purpose:** Migrate UI layer to use service interfaces

**Key Components:**
- main.rs: Initialize ServiceRegistry
- chat_view.rs: Use AgentService
- mcp_add_view.rs: Use McpService
- mcp_configure_view.rs: Use McpService
- Gradual migration strategy

**Requirements:** REQ-025, REQ-026, REQ-027, REQ-028

### 08-integration.md

**Purpose:** Integration testing and end-to-end verification

**Key Components:**
- Integration test suite
- End-to-end tests
- Performance benchmarks
- Documentation updates
- Cleanup of deprecated code

**Requirements:** All requirements (integration verification)

## Verification Commands

### Structural Verification

```bash
# Check all pseudocode files exist
ls -1 project-plans/refactor/analysis/pseudocode/*.md
# Expected: 8 files (01-utility-types.md through 08-integration.md)

# Check plan markers
grep -r "@plan:PLAN-20250125-REFACTOR.P03" project-plans/refactor/analysis/pseudocode/ | wc -l
# Expected: 8 occurrences (one per file)

# Check requirement references
grep -r "REQ-[0-9]" project-plans/refactor/analysis/pseudocode/ | wc -l
# Expected: 30+ requirement references

# Check pseudocode structure
grep -r "## Implementation Steps" project-plans/refactor/analysis/pseudocode/ | wc -l
# Expected: 8 occurrences (one per file)

# Check verification steps
grep -r "## Verification Steps" project-plans/refactor/analysis/pseudocode/ | wc -l
# Expected: 8 occurrences (one per file)
```

### Content Verification

```bash
# Count implementation steps across all pseudocode
grep -r "### Step [0-9]" project-plans/refactor/analysis/pseudocode/ | wc -l
# Expected: 50+ implementation steps

# Check for line numbers (for traceability)
grep -r "lines [0-9]-[0-9]" project-plans/refactor/analysis/pseudocode/ | wc -l
# Expected: 20+ line number references

# Verify pseudocode has code examples
grep -r '```rust' project-plans/refactor/analysis/pseudocode/ | wc -l
# Expected: 30+ code blocks
```

### Manual Verification Checklist

Read each pseudocode file and verify:

#### General

- [ ] Pseudocode is numbered and sequential
- [ ] Each step maps to specific requirements (REQ-XXX)
- [ ] Pseudocode is detailed enough to implement
- [ ] Verification steps are included
- [ ] Plan markers are present

#### Per-File Verification

**01-utility-types.md**
- [ ] Service trait defined
- [ ] RequestHandler trait defined
- [ ] ObservableService trait defined
- [ ] ServiceError enum defined
- [ ] ServiceMetrics struct defined

**02-service-registry.md**
- [ ] ServiceRegistry struct defined
- [ ] Initialization logic defined
- [ ] Service accessors defined
- [ ] Health check defined
- [ ] Shutdown logic defined

**03-llm-service.md**
- [ ] LlmService struct defined
- [ ] Extraction from client.rs documented
- [ ] Model management defined
- [ ] Request handling defined
- [ ] Provider logic defined

**04-registry-service.md**
- [ ] RegistryService struct defined
- [ ] Cache management defined
- [ ] TTL logic defined
- [ ] Health check defined
- [ ] Metrics defined

**05-mcp-service.md**
- [ ] McpService struct defined
- [ ] Consolidation from manager.rs + service.rs documented
- [ ] Lifecycle management defined
- [ ] Tool routing defined
- [ ] Health monitoring defined

**06-agent-service.md**
- [ ] AgentService struct defined
- [ ] SerdesAI integration documented
- [ ] Toolset integration defined
- [ ] Conversation management defined
- [ ] Note about SerdesAI PR #5

**07-ui-migration.md**
- [ ] Migration strategy defined
- [ ] main.rs changes documented
- [ ] UI module changes documented
- [ ] Backwards compatibility documented
- [ ] Gradual migration approach

**08-integration.md**
- [ ] Integration tests defined
- [ ] End-to-end tests defined
- [ ] Performance benchmarks defined
- [ ] Documentation updates defined
- [ ] Cleanup steps defined

## Success Criteria

- All pseudocode files created and documented
- Pseudocode is detailed and implementable
- Each pseudocode maps to requirements
- Verification steps included
- Plan markers present
- Line numbers for traceability

## Failure Recovery

If this phase fails (pseudocode incomplete):

1. Identify missing pseudocode components
2. Complete missing pseudocode
3. Re-run Phase 03a verification
4. Cannot proceed to Phase 04 until pseudocode complete

## Phase Completion Marker

Already created: `project-plans/refactor/plan/.completed/P03.md`

Contents:

```markdown
Phase: P03
Completed: [Date when pseudocode was completed]
Files Created:
  - analysis/pseudocode/01-utility-types.md
  - analysis/pseudocode/02-service-registry.md
  - analysis/pseudocode/03-llm-service.md
  - analysis/pseudocode/04-registry-service.md
  - analysis/pseudocode/05-mcp-service.md
  - analysis/pseudocode/06-agent-service.md
  - analysis/pseudocode/07-ui-migration.md
  - analysis/pseudocode/08-integration.md
Files Modified: None
Tests Added: None (pseudocode phase)
Verification: Pseudocode complete and detailed
Total Implementation Steps: [N]
Total Requirements Referenced: [N]
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 03a: Pseudocode Verification
2. Verify pseudocode completeness and correctness
3. Then proceed to implementation phases (04+)

## Important Notes

This phase documents work that has already been completed. The pseudocode phase was done before creating the plan files. This documentation ensures traceability and verification of the pseudocode work.

## References

- `project-plans/refactor/analysis/pseudocode/` - All pseudocode files
- `project-plans/refactor/specification.md` - Requirements specification
- `project-plans/refactor/analysis/domain-model.md` - Domain model
- `project-plans/refactor/analysis/target-architecture.md` - Target architecture
