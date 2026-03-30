# Homebrew Release Flow (arm64, ad-hoc signed)

This repository publishes `personal-agent` releases from git tags and updates the Homebrew tap automatically.

## Scope

- Target architecture: `aarch64-apple-darwin` (Apple Silicon)
- Distribution channel: Homebrew tap only (`acoliver/homebrew-tap`)
- Signing: ad-hoc (`codesign --sign -`)
- No notarization

## Workflow entry point

- `.github/workflows/release.yml`
- Triggers:
  - Push tag matching `v*`
  - Manual dispatch with `release_tag` input

## Required repository secret

Set in `acoliver/personal-agent`:

- `HOMEBREW_TAP_GITHUB_TOKEN`
  - Fine-grained PAT with write access to `acoliver/homebrew-tap`
  - Minimum required permission: repository contents (read/write)

## Optional repository variables

Defaults are already wired in the workflow and scripts.

- `HOMEBREW_TAP_REPO` (default: `acoliver/homebrew-tap`)
- `HOMEBREW_FORMULA_NAME` (default: `personal-agent`)

## Release process

1. Bump version in `Cargo.toml` if needed.
2. Push a tag:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

3. Workflow builds and signs `target/release/personal_agent_gpui`.
4. Workflow packages artifact as:
   - `personal-agent-vX.Y.Z-aarch64-apple-darwin.tar.gz`
   - `SHA256SUMS.txt`
5. Workflow creates or updates GitHub Release `vX.Y.Z` with artifacts.
6. Workflow updates `Formula/personal-agent.rb` in `acoliver/homebrew-tap`.

## Install command for users

```bash
brew tap acoliver/homebrew-tap
brew install personal-agent
```

## Local dry-run helpers

- Build/package/sign locally:

  ```bash
  scripts/release/package_macos_arm64.sh v0.1.0
  ```

- Update tap formula manually (expects env vars):

  ```bash
  export HOMEBREW_TAP_GITHUB_TOKEN=...
  export GITHUB_REPOSITORY=acoliver/personal-agent
  scripts/release/update_homebrew_tap.sh v0.1.0 personal-agent-v0.1.0-aarch64-apple-darwin.tar.gz <sha256>
  ```

## Notes

- Ad-hoc signing is sufficient for this Homebrew-first flow but is not Apple-trusted developer signing.
- If Developer ID signing is added later, only the signing step and secrets need to change; release/tap mechanics can remain the same.
