#!/usr/bin/env bash
set -euo pipefail

retired_paths=(
  "apps/desktop"
  "apps/web"
  "packages/ui"
  "crates/fasti-player"
  "crates/fasti-sync"
  "crates/fasti-connectors"
  "crates/fasti-projections"
  "crates/fasti-core"
  "crates/fasti-activity"
  "crates/fasti-auth"
)

for path in "${retired_paths[@]}"; do
  while IFS= read -r tracked_file; do
    if [[ -e "$tracked_file" ]]; then
      echo "Retired placeholder remains in the repository inventory: $tracked_file" >&2
      exit 1
    fi
  done < <(git ls-files --cached --others --exclude-standard -- "$path")
done

manifest_dependency_names() {
  local manifest="$1"

  # Read dependency keys rather than grepping the whole manifest so package
  # descriptions and comments cannot produce false boundary failures. This
  # covers normal, development, build, target-specific, and renamed entries.
  awk '
    function trim(value) {
      sub(/^[[:space:]]+/, "", value)
      sub(/[[:space:]]+$/, "", value)
      return value
    }

    function unquote(value) {
      value = trim(value)
      if (value ~ /^"[^"]+"$/ || value ~ /^\047[^\047]+\047$/) {
        return substr(value, 2, length(value) - 2)
      }
      return value
    }

    function package_override(line, value) {
      if (match(line, /package[[:space:]]*=[[:space:]]*["\047][A-Za-z0-9_-]+["\047]/)) {
        value = substr(line, RSTART, RLENGTH)
        sub(/^package[[:space:]]*=[[:space:]]*["\047]/, "", value)
        sub(/["\047]$/, "", value)
        print value
      }
    }

    /^[[:space:]]*\[/ {
      section = $0
      sub(/[[:space:]]*#.*/, "", section)
      section = trim(section)
      direct = section ~ /^\[(target\..+\.)?(dev-|build-)?dependencies\]$/
      nested = section ~ /^\[(target\..+\.)?(dev-|build-)?dependencies\.[A-Za-z0-9_-]+\]$/

      if (nested) {
        dependency = section
        sub(/^.*dependencies\./, "", dependency)
        sub(/\]$/, "", dependency)
        print unquote(dependency)
      }
      next
    }

    direct && /^[[:space:]]*["\047]?[A-Za-z0-9_-]+["\047]?[[:space:]]*=/ {
      dependency = $0
      sub(/=.*/, "", dependency)
      print unquote(dependency)
      package_override($0)
      next
    }

    nested {
      package_override($0)
    }
  ' "$manifest" | sort -u
}

assert_no_boundary_dependencies() {
  local crate="$1"
  shift
  local dependencies
  local dependency
  local forbidden

  dependencies="$(manifest_dependency_names "crates/$crate/Cargo.toml")"
  for dependency in $dependencies; do
    for forbidden in "$@"; do
      if [[ "$dependency" == "$forbidden" ]]; then
        echo "$crate must not depend on boundary-external package: $dependency" >&2
        exit 1
      fi
    done
  done
}

adapter_schema_runtime_dependencies=(
  "axum"
  "clap"
  "fasti-api"
  "fasti-cli"
  "fasti-contracts"
  "fasti-store"
  "rusqlite"
  "schemars"
  "serde-saphyr"
  "tauri"
  "tokio"
  "tower"
  "tracing"
  "tracing-subscriber"
  "utoipa"
  "utoipa-axum"
)

assert_no_boundary_dependencies \
  "fasti-domain" \
  "fasti-application" \
  "${adapter_schema_runtime_dependencies[@]}"

assert_no_boundary_dependencies \
  "fasti-application" \
  "${adapter_schema_runtime_dependencies[@]}"

while IFS= read -r knowledge_file; do
  if ! grep -Fq "**Status:** Governed design draft; not implemented" "$knowledge_file"; then
    echo "Knowledge surface lacks an explicit future-draft boundary: $knowledge_file" >&2
    exit 1
  fi
done < <(find knowledge -type f -name '*.md' -print)

if grep -RFn '/settings/' knowledge; then
  echo "Knowledge drafts must not direct users to unavailable product routes" >&2
  exit 1
fi

active_claim_files=()
while IFS= read -r claim_file; do
  [[ -f "$claim_file" ]] || continue
  case "$claim_file" in
    brand/* | contracts/* | fixtures/* | knowledge/* | scripts/* | tests/*) continue ;;
    *.md | *.toml | *.json | *.yml | *.yaml | *.rs | *.ts | *.mjs)
      active_claim_files+=("$claim_file")
      ;;
  esac
done < <(git ls-files --cached --others --exclude-standard)

forbidden_claims=(
  "chronicle and player"
  "playback built in"
  "embedded playback"
  "fasti-player"
  "fasti-sync"
  "fasti-connectors"
  "fasti-projections"
)

for claim in "${forbidden_claims[@]}"; do
  if grep -Fin "$claim" "${active_claim_files[@]}"; then
    echo "Unsupported product claim remains: $claim" >&2
    exit 1
  fi
done

if grep -RFn '.route("/api/v1/events"' crates/fasti-api/src; then
  echo "The event route must remain absent until durable persistence exists" >&2
  exit 1
fi

if grep -RFn "submitEvent" packages/sdk/src; then
  echo "The SDK must not claim an unavailable event-submission capability" >&2
  exit 1
fi

grep -Fiq "discussion" CONTRIBUTING.md
grep -Fq "Developer Certificate of Origin" CONTRIBUTING.md
grep -Fq "AGPL-3.0-or-later" README.md

while IFS=: read -r workflow line_number instruction; do
  action_ref="${instruction#*uses:}"
  action_ref="${action_ref%%#*}"
  action_ref="${action_ref#"${action_ref%%[![:space:]]*}"}"
  action_ref="${action_ref%"${action_ref##*[![:space:]]}"}"
  action_ref="${action_ref#\"}"
  action_ref="${action_ref%\"}"
  action_ref="${action_ref#\'}"
  action_ref="${action_ref%\'}"

  [[ "$action_ref" == ./* ]] && continue

  if [[ ! "$action_ref" =~ ^[^@[:space:]]+@[0-9a-f]{40}$ ]]; then
    echo "External workflow action must use an immutable commit: $workflow:$line_number: $action_ref" >&2
    exit 1
  fi
done < <(
  grep -RHnE '^[[:space:]]*(-[[:space:]]*)?uses:[[:space:]]*' .github/workflows
)

echo "PASS: active repository surfaces match the B0/B1 capability boundaries"
