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

version="${release_tag#v}"
bundle_id="com.personalagent.gpui"
app_name="PersonalAgent"
binary_path="target/release/personal_agent_gpui"
artifact_dir="${repo_root}/artifacts/release"
asset_name="personal-agent-${release_tag}-aarch64-apple-darwin.tar.gz"
asset_path="${artifact_dir}/${asset_name}"
sha_file="${artifact_dir}/SHA256SUMS.txt"

rm -rf "${artifact_dir}"
mkdir -p "${artifact_dir}"

# Set macOS deployment target to 13.0 so SMAppService APIs are available at
# runtime. Exported so cargo / rustc / the linker all see the same floor.
# Issue #177: launch-at-login uses SMAppService (macOS 13+ only).
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-13.0}"

echo "Building release binary (MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET})..."
cargo build --release --bin personal_agent_gpui

if [[ ! -f "${binary_path}" ]]; then
  echo "expected release binary not found at ${binary_path}" >&2
  exit 1
fi

if ! lipo -archs "${binary_path}" | grep -q "arm64"; then
  echo "release binary is not arm64" >&2
  lipo -archs "${binary_path}" >&2 || true
  exit 1
fi

# -------------------------------------------------------------------------
# Build .app bundle (Issue #177)
#
# A proper .app bundle is required for:
#   1. LSUIElement=true to take effect (raw binaries have no Info.plist).
#   2. SMAppService.mainApp to register this binary as a login item.
#
# Layout:
#   PersonalAgent.app/
#     Contents/
#       Info.plist
#       MacOS/PersonalAgent          (the signed binary)
#       Resources/AppIcon.icns       (optional, added if present)
# -------------------------------------------------------------------------
package_dir="$(mktemp -d)"
trap 'rm -rf "${package_dir}"' EXIT

app_bundle="${package_dir}/${app_name}.app"
mkdir -p "${app_bundle}/Contents/MacOS"
mkdir -p "${app_bundle}/Contents/Resources"

cp "${binary_path}" "${app_bundle}/Contents/MacOS/${app_name}"
chmod +x "${app_bundle}/Contents/MacOS/${app_name}"

if [[ -f "${repo_root}/assets/AppIcon.icns" ]]; then
  cp "${repo_root}/assets/AppIcon.icns" "${app_bundle}/Contents/Resources/AppIcon.icns"
  icon_file_key="<key>CFBundleIconFile</key>
    <string>AppIcon</string>"
else
  icon_file_key=""
fi

cat > "${app_bundle}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>${app_name}</string>
    <key>CFBundleExecutable</key>
    <string>${app_name}</string>
    <key>CFBundleIdentifier</key>
    <string>${bundle_id}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${app_name}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${version}</string>
    <key>CFBundleVersion</key>
    <string>${version}</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © PersonalAgent contributors. MIT License.</string>
    ${icon_file_key}
</dict>
</plist>
PLIST

# Validate the plist so packaging fails fast if the heredoc is malformed.
plutil -lint "${app_bundle}/Contents/Info.plist"

# Sanity check: LSUIElement key must end up in the bundled plist, otherwise the
# Dock-icon suppression silently regresses on subsequent releases.
if ! /usr/libexec/PlistBuddy -c 'Print :LSUIElement' "${app_bundle}/Contents/Info.plist" \
    | grep -qi '^true$'; then
  echo "Info.plist does not declare LSUIElement=true" >&2
  exit 1
fi

echo "Signing .app bundle with ad-hoc identity..."
codesign --force --deep --sign - "${app_bundle}"
codesign --verify --verbose=2 "${app_bundle}"

echo "Packaging .app bundle into ${asset_name}..."
tar -C "${package_dir}" -czf "${asset_path}" "${app_name}.app"

sha256="$(shasum -a 256 "${asset_path}" | awk '{print $1}')"
printf "%s  %s\n" "${sha256}" "${asset_name}" > "${sha_file}"

printf "%s" "${asset_name}" > "${artifact_dir}/asset_name.txt"
printf "%s" "${asset_path}" > "${artifact_dir}/asset_path.txt"
printf "%s" "${sha256}" > "${artifact_dir}/sha256.txt"

echo "Created release artifact: ${asset_path}"
echo "SHA256: ${sha256}"
