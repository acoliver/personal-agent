#!/usr/bin/env bash
set -euo pipefail

if [[ $# -gt 1 ]]; then
  echo "usage: $0 [vX.Y.Z]" >&2
  exit 1
fi

release_tag="${1:-${GITHUB_REF_NAME:-}}"
if [[ -z "${release_tag}" ]]; then
  echo "missing release tag; pass vX.Y.Z or set GITHUB_REF_NAME" >&2
  exit 1
fi

if [[ "${release_tag}" != v* ]]; then
  echo "release tag must start with v (received: ${release_tag})" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

binary_path="target/release/personal_agent_gpui"
artifact_dir="${repo_root}/artifacts/release"
asset_name="personal-agent-${release_tag}-aarch64-apple-darwin.tar.gz"
asset_path="${artifact_dir}/${asset_name}"
sha_file="${artifact_dir}/SHA256SUMS.txt"

rm -rf "${artifact_dir}"
mkdir -p "${artifact_dir}"

echo "Building release binary..."
cargo build --release --bin personal_agent_gpui

if [[ ! -f "${binary_path}" ]]; then
  echo "expected release binary not found at ${binary_path}" >&2
  exit 1
fi

echo "Signing binary with ad-hoc identity..."
codesign --force --sign - "${binary_path}"
codesign --verify --verbose=2 "${binary_path}"

if ! file "${binary_path}" | grep -q "arm64"; then
  echo "release binary is not arm64" >&2
  file "${binary_path}" >&2
  exit 1
fi

package_dir="$(mktemp -d)"
trap 'rm -rf "${package_dir}"' EXIT
cp "${binary_path}" "${package_dir}/personal_agent_gpui"
tar -C "${package_dir}" -czf "${asset_path}" personal_agent_gpui

sha256="$(shasum -a 256 "${asset_path}" | awk '{print $1}')"
printf "%s  %s\n" "${sha256}" "${asset_name}" > "${sha_file}"

printf "%s" "${asset_name}" > "${artifact_dir}/asset_name.txt"
printf "%s" "${asset_path}" > "${artifact_dir}/asset_path.txt"
printf "%s" "${sha256}" > "${artifact_dir}/sha256.txt"

echo "Created release artifact: ${asset_path}"
echo "SHA256: ${sha256}"
