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
deb_name="personal-agent-${release_tag}-x86_64-linux-gnu.deb"
rpm_name="personal-agent-${release_tag}-x86_64-linux-gnu.rpm"
zip_name="personal-agent-${release_tag}-x86_64-linux-gnu.zip"
deb_path="${artifact_dir}/${deb_name}"
rpm_path="${artifact_dir}/${rpm_name}"
zip_path="${artifact_dir}/${zip_name}"
sha_file="${artifact_dir}/SHA256SUMS.txt"

rm -rf "${artifact_dir}"
mkdir -p "${artifact_dir}"

echo "Building release binary..."
cargo build --release --bin personal_agent_gpui

if [[ ! -f "${binary_path}" ]]; then
  echo "expected release binary not found at ${binary_path}" >&2
  exit 1
fi

echo "Verifying binary architecture..."
if ! file "${binary_path}" | grep -q "ELF.*x86-64"; then
  echo "release binary is not x86-64 ELF" >&2
  file "${binary_path}" >&2 || true
  exit 1
fi

echo "Installing cargo-deb..."
if ! command -v cargo-deb >/dev/null 2>&1; then
  cargo install cargo-deb --locked
fi

echo "Installing cargo-generate-rpm..."
if ! command -v cargo-generate-rpm >/dev/null 2>&1; then
  cargo install cargo-generate-rpm --locked
fi

echo "Creating .deb package..."
cargo deb --no-build --output "${deb_path}"

echo "Creating .rpm package..."
cargo generate-rpm -o "${rpm_path}"

echo "Creating .zip archive..."
package_dir="$(mktemp -d)"
trap 'rm -rf "${package_dir}"' EXIT
cp "${binary_path}" "${package_dir}/personal-agent"
(cd "${package_dir}" && zip -r "${zip_path}" personal-agent)

echo "Computing SHA256 checksums..."
{
  sha256sum "${deb_path}" | awk '{print $1 "  " $2}'
  sha256sum "${rpm_path}" | awk '{print $1 "  " $2}'
  sha256sum "${zip_path}" | awk '{print $1 "  " $2}'
} > "${sha_file}"

deb_sha="$(sha256sum "${deb_path}" | awk '{print $1}')"
rpm_sha="$(sha256sum "${rpm_path}" | awk '{print $1}')"
zip_sha="$(sha256sum "${zip_path}" | awk '{print $1}')"

printf "%s" "${deb_name}" > "${artifact_dir}/deb_name.txt"
printf "%s" "${deb_path}" > "${artifact_dir}/deb_path.txt"
printf "%s" "${deb_sha}" > "${artifact_dir}/deb_sha256.txt"

printf "%s" "${rpm_name}" > "${artifact_dir}/rpm_name.txt"
printf "%s" "${rpm_path}" > "${artifact_dir}/rpm_path.txt"
printf "%s" "${rpm_sha}" > "${artifact_dir}/rpm_sha256.txt"

printf "%s" "${zip_name}" > "${artifact_dir}/zip_name.txt"
printf "%s" "${zip_path}" > "${artifact_dir}/zip_path.txt"
printf "%s" "${zip_sha}" > "${artifact_dir}/zip_sha256.txt"

echo "Created release artifacts:"
echo "  ${deb_path}"
echo "  ${rpm_path}"
echo "  ${zip_path}"
echo "  ${sha_file}"
