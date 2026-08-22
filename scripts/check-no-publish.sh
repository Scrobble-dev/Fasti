#!/usr/bin/env bash
set -euo pipefail

workflow_dir="${1:-.github/workflows}"
script_dir="${2:-scripts}"

if [[ ! -d "$workflow_dir" ]]; then
  echo "Workflow directory not found: $workflow_dir" >&2
  exit 2
fi

forbidden='(^[[:space:]]*permissions:[[:space:]]*write-all([[:space:]]|$)|^[[:space:]]*[A-Za-z0-9_-]+:[[:space:]]*write([[:space:]]|$)|docker/login-action|release-action|upload-release-action|action-gh-release|attest-build-provenance|push-to-registry:[[:space:]]*true|push:[[:space:]]*true|(^|[[:space:]])(docker|podman|nerdctl|oras)[[:space:]]+push([[:space:]]|$)|docker[[:space:]]+buildx[[:space:]]+build.*--push([=[:space:]]|$)|^[[:space:]]*--push([=[:space:]]|$)|(npm|pnpm|yarn|cargo)[^[:alnum:]_#\n]+publish([^[:alnum:]_]|$)|(^|[[:space:]])(npm|pnpm|yarn|bun)[[:space:]]+run[[:space:]]+publish([[:space:]]|$)|(^|[[:space:]])gh[[:space:]]+release([[:space:]]|$)|(^|[[:space:]])gh[[:space:]]+api[^#\n]*/releases([[:space:]]|$))'
failed=0

while IFS= read -r -d '' workflow; do
  matches="$(grep -En "$forbidden" "$workflow" || true)"
  if [[ -n "$matches" ]]; then
    echo "Public publishing is disabled before the B8 release gate: $workflow" >&2
    echo "$matches" >&2
    failed=1
  fi
done < <(find "$workflow_dir" -type f \( -name '*.yml' -o -name '*.yaml' \) -print0)

if [[ -d "$script_dir" ]]; then
  while IFS= read -r -d '' script; do
    case "$(basename "$script")" in
      check-no-publish.sh | test-no-publish-policy.sh) continue ;;
    esac

    matches="$(grep -En "$forbidden" "$script" || true)"
    if [[ -n "$matches" ]]; then
      echo "Public publishing command is disabled before the B8 release gate: $script" >&2
      echo "$matches" >&2
      failed=1
    fi
  done < <(find "$script_dir" -type f -print0)
fi

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

echo "PASS: workflows contain no public publishing path"
