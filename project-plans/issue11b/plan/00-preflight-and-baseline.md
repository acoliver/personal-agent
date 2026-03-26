# Phase 00: Preflight and Baseline Exposure

## Phase ID

`PLAN-20260325-ISSUE11B.P00`

## Objective

Expose the GPUI structural debt in CI and record the concrete baseline that the refactoring effort must eliminate.

## Requirements

### REQ-STRUCT-001
GPUI views/components must not be excluded from project structural checks merely because they are large.

### REQ-STRUCT-002
The project must record which files and functions violate the current thresholds so refactoring can proceed deliberately.

### REQ-BASELINE-001
The plan must capture not only file length but also function-level lizard debt, source-text test dependencies, and public API surfaces that will be affected.

## Tasks

1. Remove GPUI view/component excludes from `.github/workflows/pr-quality-and-e2e.yml`.
2. Validate workflow syntax.
3. Record the worst GPUI file sizes and target ordering.
4. Record current GPUI lizard hotspots.
5. Confirm current GPUI components are or are not structural offenders.
6. Inventory `include_str!()` tests referencing the GPUI view files.
7. Record exported surfaces in `src/ui_gpui/views/mod.rs` that will need re-export stability or deliberate consumer updates.
8. Record that the excludes were a policy/workaround choice, not a GPUI framework requirement.

## Verification

```bash
ruby -e "require 'yaml'; YAML.load_file('.github/workflows/pr-quality-and-e2e.yml')"
find src/ui_gpui/views src/ui_gpui/components -name '*.rs' -print0 | xargs -0 wc -l | sort -n | tail -n 30
python3 -m venv .venv-lizard
. .venv-lizard/bin/activate
python -m pip install --upgrade pip
python -m pip install lizard
python -m lizard -C 50 -L 100 -w src/ui_gpui/views src/ui_gpui/components
rg 'include_str!\("../src/ui_gpui/views/' tests
```

## Required evidence

- workflow diff showing GPUI exemptions removed
- YAML validation output
- ranked file-length offender table
- ranked function-level lizard offender table
- note showing components are baseline-safe or naming any component offenders
- list of `include_str!()` tests per target file
- `views/mod.rs` export inventory

## Success Criteria

- GPUI excludes are removed from the workflow
- the baseline offender list is recorded
- function-level lizard hotspots are recorded
- source-text test dependencies are recorded
- public API/re-export impact is recorded
