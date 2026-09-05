#!/usr/bin/env bash
set -euo pipefail

retired_paths=(
  "crates/fasti-player"
  "crates/fasti-sync"
  "crates/fasti-connectors"
  "crates/fasti-projections"
  "crates/fasti-core"
  "crates/fasti-activity"
  "crates/fasti-auth"
)

if git ls-files --error-unmatch .fasti-staging >/dev/null 2>&1 || [[ -d .fasti-staging ]]; then
  echo "Temporary source-transfer staging must not be tracked or present" >&2
  exit 1
fi

if find .github/workflows -maxdepth 1 -type f \( -name '*apply*source*.yml' -o -name '*diagnose*source*.yml' \) -print | grep -q .; then
  echo "Review workflows must not apply or diagnose staged source archives" >&2
  exit 1
fi

for path in "${retired_paths[@]}"; do
  while IFS= read -r tracked_file; do
    if [[ -e "$tracked_file" ]]; then
      echo "Retired placeholder remains in the repository inventory: $tracked_file" >&2
      exit 1
    fi
  done < <(git ls-files --cached --others --exclude-standard -- "$path")
done

if [[ ! -f docs/architecture/adr-0005-framework-and-auth-adoption.md ]]; then
  echo "The framework and authentication adoption decision is missing" >&2
  exit 1
fi

docker_from_records() {
  awk '
    toupper($1) == "FROM" {
      image = ""
      alias = ""
      for (field = 2; field <= NF; field++) {
        if ($field ~ /^--/) {
          continue
        }
        if (image == "") {
          image = $field
          continue
        }
        if (toupper($field) == "AS" && field < NF) {
          alias = $(field + 1)
          break
        }
      }
      if (image == "") {
        print "Dockerfile FROM instruction has no image" > "/dev/stderr"
        exit 2
      }
      print image "|" alias
    }
  ' Dockerfile
}

declare -A docker_stage_aliases=()
last_docker_image=""
last_docker_alias=""
while IFS='|' read -r base_image stage_alias; do
  if [[ -z "${docker_stage_aliases[$base_image]+defined}" && "$base_image" != *@sha256:* ]]; then
    echo "External Docker base image must use an immutable digest: $base_image" >&2
    exit 1
  fi
  if [[ -n "$stage_alias" ]]; then
    docker_stage_aliases["$stage_alias"]=1
  fi
  last_docker_image="$base_image"
  last_docker_alias="$stage_alias"
done < <(docker_from_records)

if [[ "$last_docker_image|$last_docker_alias" != "runtime|default" ]]; then
  echo "A plain Docker build must finish at the runtime-equivalent default stage" >&2
  exit 1
fi

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

python3 - <<'PYTHON'
from __future__ import annotations

import os
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any

DEPENDENCY_TABLES = {"dependencies", "dev-dependencies", "build-dependencies"}


def loco_declarations(document: dict[str, Any]) -> list[str]:
    declarations: list[str] = []

    def visit(value: Any, path: tuple[str, ...] = ()) -> None:
        if not isinstance(value, dict):
            return
        for key, child in value.items():
            child_path = (*path, str(key))
            if key in DEPENDENCY_TABLES and isinstance(child, dict):
                for local_name, specification in child.items():
                    package_name = local_name
                    if isinstance(specification, dict):
                        package_name = specification.get("package", local_name)
                    if package_name == "loco-rs":
                        declarations.append(".".join((*child_path, str(local_name))))
            visit(child, child_path)

    visit(document)
    return declarations


def parse_manifest(text: str, name: str) -> dict[str, Any]:
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        raise SystemExit(f"invalid Cargo manifest in framework boundary check ({name}): {error}") from error


cases = {
    "direct": ('[dependencies]\nloco-rs = "1"\n', True),
    "renamed": ('[dependencies]\nweb = { package = "loco-rs", version = "1" }\n', True),
    "table": ('[dependencies.web]\npackage = "loco-rs"\nversion = "1"\n', True),
    "target": ('[target.\'cfg(unix)\'.dev-dependencies]\nweb = { package = "loco-rs", version = "1" }\n', True),
    "negative": ('[dependencies]\naxum = "1"\n', False),
}
for case_name, (source, expected) in cases.items():
    found = bool(loco_declarations(parse_manifest(source, case_name)))
    if found != expected:
        raise SystemExit(f"framework boundary self-test failed: {case_name}")

tracked_manifest_bytes = subprocess.run(
    ["git", "ls-files", "-z", "--", "*Cargo.toml"],
    check=True,
    stdout=subprocess.PIPE,
).stdout
for encoded_path in tracked_manifest_bytes.split(b"\0"):
    if not encoded_path:
        continue
    manifest = Path(os.fsdecode(encoded_path))
    document = parse_manifest(manifest.read_text(encoding="utf-8"), str(manifest))
    declarations = loco_declarations(document)
    if declarations:
        joined = ", ".join(declarations)
        print(
            "Loco is a reference for workflow patterns, not an active Fasti "
            f"runtime dependency: {manifest} ({joined})",
            file=sys.stderr,
        )
        raise SystemExit(1)
PYTHON

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
    *.md | *.toml | *.json | *.yml | *.yaml | *.rs | *.ts | *.mjs | *.svelte | *.html)
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

bash scripts/dev.sh --self-test

echo "PASS: active repository surfaces match the B0/B1 capability boundaries"
