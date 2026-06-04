#!/usr/bin/env bash
set -euo pipefail

fail() {
  printf '::error::%s\n' "$1" >&2
  exit 1
}

repo="${CLI_RELEASE_REPO:-futex-ai/cli-release}"
repo_prefix="https://github.com/${repo}"
repo_url="${repo_prefix}.git"

if [ -z "${CLI_RELEASE_REPO_TOKEN:-}" ]; then
  fail "CLI_RELEASE_REPO_TOKEN is required to fetch ${repo}"
fi

auth_prefix="https://x-access-token:${CLI_RELEASE_REPO_TOKEN}@github.com/${repo}"

git config --global url."${auth_prefix}".insteadOf "${repo_prefix}"
git config --global url."${auth_prefix}.git".insteadOf "${repo_url}"

if [ -n "${GITHUB_ENV:-}" ]; then
  echo "CARGO_NET_GIT_FETCH_WITH_CLI=true" >>"$GITHUB_ENV"
fi

if ! git ls-remote --exit-code "$repo_url" HEAD >/dev/null; then
  fail "CLI_RELEASE_REPO_TOKEN cannot read ${repo}; grant it read-only Contents access to ${repo} and expose the secret to this repository"
fi
