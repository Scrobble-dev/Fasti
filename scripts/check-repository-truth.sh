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
)

for path in "${retired_paths[@]}"; do
  while IFS= read -r tracked_file; do
    if [[ -e "$tracked_file" ]]; then
      echo "Retired placeholder remains in the repository inventory: $tracked_file" >&2
      exit 1
    fi
  done < <(git ls-files --cached --others --exclude-standard -- "$path")
done

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

echo "PASS: active repository surfaces match the B0 capability boundary"
