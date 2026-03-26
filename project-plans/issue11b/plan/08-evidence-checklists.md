# Phase Evidence Checklists

Each implementation phase must attach concrete evidence before it can be marked complete.

## Required evidence for every implementation batch

### 1. Safety-net evidence before extraction
- list of behavioral safety-net tests for the batch
- passing output proving those tests already protect the current behavior before structural movement
- brief note mapping each safety-net test to a contract from `06-behavior-contract-inventory.md`

### 2. Optional red→green evidence for newly added missing coverage
- only required if the batch adds genuinely new behavioral coverage rather than pure extraction protection
- failing output for the new missing-behavior test
- passing output after the minimal implementation or seam addition

### 3. Green-after-move evidence
- passing output for the same safety-net tests after extraction
- short note describing the minimal extraction that preserved the behavior

### 4. Structural evidence
- before/after line counts for touched files
- before/after lizard output for touched files/functions
- note proving no newly created file exceeded `750` lines without justification
- note proving no touched function exceeds `-L 100` or `-C 50`
- note proving no extracted file contains an unjustified mix of more than two of: command handling, render implementation, data model/state management

### 5. API/static-test evidence
- list of public re-exports preserved or changed
- list of `include_str!()` assertions touched and their recorded disposition
- for each surviving static assertion that was updated rather than replaced/removed, a brief justification of the stable contract it still proves
- note showing any inline tests moved, preserved, or externalized deliberately

### 6. Coverage evidence
- `cargo coverage` result for the batch or nearest stable checkpoint
- explanation for any coverage delta
- note showing no newly extracted substantial module was left effectively untested

## Required evidence for stabilization checkpoints
- duplicate helpers removed
- transitional adapters removed or justified
- dead code removed
- module ownership/layout note updated
- no extracted file contains an unjustified mix of more than two of: command handling, render implementation, data model/state management
