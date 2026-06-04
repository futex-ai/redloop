#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
Usage:
  scripts/cli-release.sh resolve-workspace-tag <tag> <manifest-path>
  scripts/cli-release.sh verify-package-manifests <manifest-path>
  scripts/cli-release.sh normalize-workspace-dependency-versions <manifest-path>
  scripts/cli-release.sh write-release-pr-config <source-config> <output-config>
  scripts/cli-release.sh resolve-binary-package <binary> <binary-config-json>
  scripts/cli-release.sh package <binary> <version> <target> <binary-path> <dist-dir>

Environment:
  CLI_RELEASE_VERSION_PACKAGE  Package used to read the workspace version.
                               Defaults to cli-release-server.
USAGE
}

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 64
}

require_jq() {
  if ! command -v jq >/dev/null 2>&1; then
    fail "jq is required"
  fi
}

checksum_sha256() {
  file_path="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file_path" | awk '{print $1}'
    return
  fi

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file_path" | awk '{print $1}'
    return
  fi

  fail "sha256sum or shasum is required"
}

version_package() {
  printf '%s\n' "${CLI_RELEASE_VERSION_PACKAGE:-cli-release-server}"
}

workspace_version() {
  manifest_path="$1"
  package_name="$(version_package)"

  require_jq

  cargo metadata \
    --no-deps \
    --format-version=1 \
    --manifest-path "$manifest_path" \
    | jq -r --arg package "$package_name" '.packages[] | select(.name == $package) | .version'
}

validate_name() {
  value="$1"
  label="$2"

  if [ -z "$value" ]; then
    fail "$label is required"
  fi

  case "$value" in
    *[!A-Za-z0-9._-]*)
      fail "$label contains unsupported characters"
      ;;
  esac
}

validate_version() {
  version="$1"

  validate_name "$version" "version"
  case "$version" in
    v*)
      fail "version must not include a leading v"
      ;;
  esac
}

resolve_workspace_tag() {
  if [ "$#" -ne 2 ]; then
    usage
    exit 64
  fi

  tag="${1#refs/tags/}"
  manifest_path="$2"

  case "$tag" in
    v*)
      version="${tag#v}"
      ;;
    *)
      fail "tag '$tag' does not match workspace release tags"
      ;;
  esac

  validate_version "$version"

  current_version="$(workspace_version "$manifest_path")"
  if [ -z "$current_version" ]; then
    fail "could not resolve package '$(version_package)' in $manifest_path"
  fi
  if [ "$current_version" != "$version" ]; then
    fail "tag version '$version' does not match workspace version '$current_version'"
  fi

  printf 'version=%s\n' "$version"
  printf 'tag=v%s\n' "$version"
  printf 'release_title=CLI Release v%s\n' "$version"
}

verify_package_manifests() {
  if [ "$#" -ne 1 ]; then
    usage
    exit 64
  fi

  manifest_path="$1"

  require_jq

  invalid_dependencies="$(
    cargo metadata \
      --no-deps \
      --format-version=1 \
      --manifest-path "$manifest_path" \
      | jq -r '
        .packages as $packages
        | $packages[]
        | . as $package
        | .dependencies[]
        | select(.path != null)
        | . as $dependency
        | ([ $packages[] | select(.name == $dependency.name) | .version ][0]) as $dependency_version
        | if $dependency_version == null then
            select($dependency.req == "*" or $dependency.req == "")
            | "\($package.name) -> \($dependency.name) requires \($dependency.req); expected a version requirement"
          else
            ("^" + $dependency_version) as $expected
            | select($dependency.req != $expected and $dependency.req != $dependency_version)
            | "\($package.name) -> \($dependency.name) requires \($dependency.req); expected \($expected)"
          end
      '
  )"

  if [ -n "$invalid_dependencies" ]; then
    printf 'error: path dependencies used by release packaging need matching version requirements:\n' >&2
    printf '%s\n' "$invalid_dependencies" >&2
    exit 64
  fi

  printf 'package_manifests=ok\n'
}

normalize_workspace_dependency_versions() {
  if [ "$#" -ne 1 ]; then
    usage
    exit 64
  fi

  manifest_path="$1"

  require_jq

  tmp_versions="$(mktemp)"
  tmp_manifest="$(mktemp)"
  cleanup() {
    rm -f "$tmp_manifest" "$tmp_versions"
  }
  trap cleanup EXIT

  cargo metadata \
    --no-deps \
    --format-version=1 \
    --manifest-path "$manifest_path" \
    | jq -r '.packages[] | select(.source == null) | [.name, .version] | @tsv' >"$tmp_versions"

  awk -v package_versions_path="$tmp_versions" '
    BEGIN {
      while ((getline package_row < package_versions_path) > 0) {
        split(package_row, package_fields, "\t")
        if (package_fields[1] != "" && package_fields[2] != "") {
          versions[package_fields[1]] = package_fields[2]
        }
      }
      close(package_versions_path)
      in_workspace_dependencies = 0
    }

    /^\[workspace\.dependencies\][[:space:]]*$/ {
      in_workspace_dependencies = 1
      print
      next
    }

    /^\[/ {
      in_workspace_dependencies = 0
    }

    {
      dependency_name = $0
      sub(/^[[:space:]]*/, "", dependency_name)
      sub(/[[:space:]]*=.*/, "", dependency_name)

      if (in_workspace_dependencies && dependency_name in versions && $0 ~ /^[[:space:]]*[[:alnum:]_-]+[[:space:]]*=/ && $0 ~ /\{[^}]*path[[:space:]]*=/ && $0 !~ /\{[^}]*version[[:space:]]*=/) {
        sub(/\{[[:space:]]*/, "{ version = \"" versions[dependency_name] "\", ")
      }

      print
    }
  ' "$manifest_path" >"$tmp_manifest"

  mv "$tmp_manifest" "$manifest_path"
  trap - EXIT

  printf 'workspace_dependency_versions=ok\n'
}

write_release_pr_config() {
  if [ "$#" -ne 2 ]; then
    usage
    exit 64
  fi

  source_config="$1"
  output_config="$2"
  tmp_config="$(mktemp)"

  cleanup() {
    rm -f "$tmp_config"
  }
  trap cleanup EXIT

  if ! awk '
    BEGIN {
      in_workspace = 0
      git_only_replaced = 0
      release_seen = 0
    }

    /^\[workspace\][[:space:]]*$/ {
      in_workspace = 1
      print
      next
    }

    /^\[/ {
      if (in_workspace && release_seen != 1) {
        print "release = true"
        release_seen = 1
      }
      in_workspace = 0
    }

    in_workspace && /^[[:space:]]*git_only[[:space:]]*=/ {
      print "git_only = false"
      git_only_replaced = 1
      next
    }

    in_workspace && /^[[:space:]]*release[[:space:]]*=/ {
      print "release = true"
      release_seen = 1
      next
    }

    {
      print
    }

    END {
      if (in_workspace && release_seen != 1) {
        print "release = true"
      }
      if (git_only_replaced != 1) {
        exit 1
      }
    }
  ' "$source_config" >"$tmp_config"; then
    fail "could not write release-pr config"
  fi

  mv "$tmp_config" "$output_config"
  trap - EXIT

  printf 'release_pr_config=%s\n' "$output_config"
}

resolve_binary_package() {
  if [ "$#" -ne 2 ]; then
    usage
    exit 64
  fi

  requested_binary="$1"
  config_path="$2"

  require_jq
  validate_name "$requested_binary" "binary"

  package="$(
    jq -r --arg binary "$requested_binary" '
      .binaries[]
      | select(.name == $binary)
      | .package
    ' "$config_path"
  )"

  if [ -z "$package" ]; then
    fail "unsupported binary '$requested_binary'"
  fi

  printf 'binary=%s\n' "$requested_binary"
  printf 'package=%s\n' "$package"
}

package_cli() {
  if [ "$#" -ne 5 ]; then
    usage
    exit 64
  fi

  binary="$1"
  version="$2"
  target="$3"
  binary_path="$4"
  dist_dir="$5"

  validate_name "$binary" "binary"
  validate_version "$version"
  validate_name "$target" "target"

  if [ ! -f "$binary_path" ]; then
    fail "binary path '$binary_path' does not exist"
  fi

  artifact_name="${binary}-${version}-${target}"
  archive_name="${artifact_name}.tar.gz"
  staging_dir="$(mktemp -d)"

  cleanup() {
    rm -rf "$staging_dir"
  }
  trap cleanup EXIT

  mkdir -p "$staging_dir/$artifact_name" "$dist_dir"
  cp "$binary_path" "$staging_dir/$artifact_name/$binary"
  chmod 755 "$staging_dir/$artifact_name/$binary"

  tar -C "$staging_dir" -czf "$dist_dir/$archive_name" "$artifact_name"
  checksum_sha256 "$dist_dir/$archive_name" >"$dist_dir/$archive_name.sha256"

  printf 'asset_name=%s\n' "$archive_name"
  printf 'asset_path=%s\n' "$dist_dir/$archive_name"
  printf 'checksum_name=%s\n' "$archive_name.sha256"
  printf 'checksum_path=%s\n' "$dist_dir/$archive_name.sha256"
}

if [ "$#" -lt 1 ]; then
  usage
  exit 64
fi

command="$1"
shift

case "$command" in
  resolve-workspace-tag)
    resolve_workspace_tag "$@"
    ;;
  verify-package-manifests)
    verify_package_manifests "$@"
    ;;
  normalize-workspace-dependency-versions)
    normalize_workspace_dependency_versions "$@"
    ;;
  write-release-pr-config)
    write_release_pr_config "$@"
    ;;
  resolve-binary-package)
    resolve_binary_package "$@"
    ;;
  package)
    package_cli "$@"
    ;;
  *)
    usage
    exit 64
    ;;
esac
