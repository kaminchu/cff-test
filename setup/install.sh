#!/usr/bin/env bash

set -euo pipefail

repository="kaminchu/cff-test"
api_url="${GITHUB_API_URL:-https://api.github.com}"
server_url="${GITHUB_SERVER_URL:-https://github.com}"

curl_args=(--fail --location --silent --show-error)
api_curl_args=("${curl_args[@]}")
if [[ -n "${GH_TOKEN:-}" ]]; then
  api_curl_args+=(--header "Authorization: Bearer ${GH_TOKEN}")
fi

release_tags() {
  curl "${api_curl_args[@]}" \
    --header "Accept: application/vnd.github+json" \
    "${api_url}/repos/${repository}/releases?per_page=100" \
    | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p'
}

resolve_version() {
  if [[ -n "${CFF_TEST_VERSION:-}" ]]; then
    printf '%s\n' "${CFF_TEST_VERSION}"
    return
  fi

  if [[ "${CFF_TEST_ACTION_REF:-}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9._-]+)?$ ]]; then
    printf '%s\n' "${CFF_TEST_ACTION_REF}"
  elif [[ "${CFF_TEST_ACTION_REF:-}" =~ ^v[0-9]+(\.[0-9]+)?$ ]]; then
    local prefix="${CFF_TEST_ACTION_REF}."
    local tag
    while IFS= read -r tag; do
      if [[ "${tag}" == "${prefix}"* ]]; then
        printf '%s\n' "${tag}"
        return
      fi
    done < <(release_tags)
    echo "No cff-test release found for ${CFF_TEST_ACTION_REF}." >&2
    return 1
  else
    release_tags | sed -n '1p'
  fi
}

case "${RUNNER_OS:-}" in
  Linux)
    case "${RUNNER_ARCH:-}" in
      X86) target="i686-unknown-linux-gnu" ;;
      X64) target="x86_64-unknown-linux-gnu" ;;
      ARM64) target="aarch64-unknown-linux-gnu" ;;
      *) echo "Unsupported Linux architecture: ${RUNNER_ARCH:-unknown}" >&2; exit 1 ;;
    esac
    ;;
  macOS)
    case "${RUNNER_ARCH:-}" in
      X64) target="x86_64-apple-darwin" ;;
      ARM64) target="aarch64-apple-darwin" ;;
      *) echo "Unsupported macOS architecture: ${RUNNER_ARCH:-unknown}" >&2; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported runner OS: ${RUNNER_OS:-unknown}" >&2
    exit 1
    ;;
esac

version="$(resolve_version)"
if [[ ! "${version}" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]; then
  echo "Invalid cff-test release tag: ${version}" >&2
  exit 1
fi

install_dir="${RUNNER_TEMP}/cff-test-${version}"
asset="cff-test-${version}-${target}"
mkdir -p "${install_dir}"

curl "${curl_args[@]}" \
  --output "${install_dir}/cff-test" \
  "${server_url}/${repository}/releases/download/${version}/${asset}"
chmod +x "${install_dir}/cff-test"
"${install_dir}/cff-test" --version
printf '%s\n' "${install_dir}" >> "${GITHUB_PATH}"
