# Maintainer Handoff Requirements

The refactor is not complete until maintainers can navigate the new structure without rediscovering it from scratch.

## Required handoff artifacts

For each major refactored GPUI family:
- short module map describing the responsibility of each extracted file
- note of the stable public entry point and any re-exports
- location of the strongest behavioral tests for that family
- note of any intentionally retained source-text/static assertions and why they are still legitimate

## Repo docs to update if the layout materially changes

- `src/ui_gpui/README.md`
- `dev-docs/architecture/gpui-architecture.md`

Update only if the module layout or recommended extension points materially changed.