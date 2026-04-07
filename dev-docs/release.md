# Release Flow (macOS arm64 + Linux x86_64)

This repository publishes `personal-agent` releases from git tags to GitHub Releases. The workflow produces artifacts for both macOS (Apple Silicon) and Linux (x86_64).

## Scope

### macOS
- Target architecture: `aarch64-apple-darwin` (Apple Silicon)
- Distribution channels: GitHub Releases + Homebrew tap (`acoliver/homebrew-tap`)
- Signing: ad-hoc (`codesign --sign -`)
- No notarization

### Linux
- Target architecture: `x86_64-linux-gnu`
- Distribution channel: GitHub Releases only
- Formats: `.deb`, `.rpm`, `.zip`
- No additional secrets required (uploads use `GITHUB_TOKEN`)

## Workflow entry point

- `.github/workflows/release.yml`
- Triggers:
  - Push tag matching `v*`
  - Manual dispatch with `release_tag` input

## Required repository secrets

Set in `acoliver/personal-agent`:

- `HOMEBREW_TAP_GITHUB_TOKEN`
  - Fine-grained PAT with write access to `acoliver/homebrew-tap`
  - Minimum required permission: repository contents (read/write)
  - Only required for macOS/Homebrew releases

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

3. **macOS job** builds and signs `target/release/personal_agent_gpui`.
4. **Linux job** builds and packages the binary for Linux.
5. Workflow creates GitHub Release `vX.Y.Z` with artifacts:
   - `personal-agent-vX.Y.Z-aarch64-apple-darwin.tar.gz` (macOS)
   - `personal-agent-vX.Y.Z-x86_64-linux-gnu.deb` (Debian/Ubuntu)
   - `personal-agent-vX.Y.Z-x86_64-linux-gnu.rpm` (RHEL/Fedora)
   - `personal-agent-vX.Y.Z-x86_64-linux-gnu.zip` (generic Linux)
   - `SHA256SUMS.txt` (checksums for all artifacts)
6. Workflow updates `Formula/personal-agent.rb` in `acoliver/homebrew-tap`.

## Install commands for users

### macOS (Homebrew)

```bash
brew tap acoliver/tap
brew install personal-agent
```

### Linux (.deb - Debian/Ubuntu)

```bash
# Download from GitHub Releases
curl -LO https://github.com/acoliver/personal-agent/releases/download/vX.Y.Z/personal-agent-vX.Y.Z-x86_64-linux-gnu.deb
sudo dpkg -i personal-agent-vX.Y.Z-x86_64-linux-gnu.deb
```

### Linux (.rpm - RHEL/Fedora)

```bash
# Download from GitHub Releases
curl -LO https://github.com/acoliver/personal-agent/releases/download/vX.Y.Z/personal-agent-vX.Y.Z-x86_64-linux-gnu.rpm
sudo rpm -i personal-agent-vX.Y.Z-x86_64-linux-gnu.rpm
```

### Linux (.zip - generic)

```bash
# Download from GitHub Releases
curl -LO https://github.com/acoliver/personal-agent/releases/download/vX.Y.Z/personal-agent-vX.Y.Z-x86_64-linux-gnu.zip
unzip personal-agent-vX.Y.Z-x86_64-linux-gnu.zip
sudo mv personal-agent /usr/local/bin/
```

## Local dry-run helpers

### macOS

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

### Linux

- Build/package locally:

  ```bash
  scripts/release/package_linux_x86_64.sh v0.1.0
  ```

## Notes

- Ad-hoc signing is sufficient for this Homebrew-first flow but is not Apple-trusted developer signing.
- If Developer ID signing is added later, only the signing step and secrets need to change; release/tap mechanics can remain the same.
- Linux artifacts are uploaded directly to GitHub Releases; no package registry is used since GitHub Packages does not support `.deb`/`.rpm`/`.zip` formats.
- Both macOS and Linux release jobs run in parallel. The macOS job typically creates the GitHub Release first; the Linux job will wait and upload artifacts to the existing release.
