#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 <release_tag> <asset_name> <asset_sha256>" >&2
  exit 1
fi

release_tag="$1"
asset_name="$2"
asset_sha256="$3"

: "${HOMEBREW_TAP_GITHUB_TOKEN:?HOMEBREW_TAP_GITHUB_TOKEN must be set}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"

if [[ ! "${release_tag}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "release tag must look like vX.Y.Z (received: ${release_tag})" >&2
  exit 1
fi

homebrew_tap_repo="${HOMEBREW_TAP_REPO:-acoliver/homebrew-tap}"
formula_name="${HOMEBREW_FORMULA_NAME:-personal-agent}"
formula_class_name="$(echo "${formula_name}" | sed -E 's/[^a-zA-Z0-9]+/ /g' | awk '{for(i=1;i<=NF;i++){printf toupper(substr($i,1,1)) tolower(substr($i,2))}}')"
version="${release_tag#v}"
release_url="https://github.com/${GITHUB_REPOSITORY}/releases/download/${release_tag}/${asset_name}"

if [[ -z "${formula_class_name}" ]]; then
  echo "unable to derive formula class name from HOMEBREW_FORMULA_NAME=${formula_name}" >&2
  exit 1
fi

work_dir="$(mktemp -d)"
trap 'rm -rf "${work_dir}"' EXIT

tap_dir="${work_dir}/homebrew-tap"
git clone "https://x-access-token:${HOMEBREW_TAP_GITHUB_TOKEN}@github.com/${homebrew_tap_repo}.git" "${tap_dir}"

mkdir -p "${tap_dir}/Formula"
formula_path="${tap_dir}/Formula/${formula_name}.rb"

cat > "${formula_path}" <<EOF
class ${formula_class_name} < Formula
  desc "PersonalAgent macOS menu bar assistant"
  homepage "https://github.com/${GITHUB_REPOSITORY}"
  url "${release_url}"
  version "${version}"
  sha256 "${asset_sha256}"
  license "MIT"

  def install
    bin.install "personal_agent_gpui" => "personal-agent"
  end

  test do
    assert_predicate bin/"personal-agent", :exist?
  end
end
EOF

pushd "${tap_dir}" >/dev/null

if [[ -z "$(git status --porcelain -- "${formula_path}")" ]]; then
  echo "No changes detected in ${formula_path}; skipping push."
  exit 0
fi

git config user.name "${GIT_AUTHOR_NAME:-github-actions[bot]}"
git config user.email "${GIT_AUTHOR_EMAIL:-41898282+github-actions[bot]@users.noreply.github.com}"

git add "${formula_path}"
git commit -m "personal-agent ${version}"
git push origin HEAD

popd >/dev/null

echo "Updated ${homebrew_tap_repo} Formula/${formula_name}.rb for ${release_tag}"
