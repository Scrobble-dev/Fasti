#!/usr/bin/env bash
# Self-test for scripts/smoke-oci.sh's runtime selection and its embedded
# OCI-memory-sample parser.
#
# The full smoke gate needs a real container runtime (Docker or Podman) and
# is exercised end-to-end in CI and by `cargo xtask test deep`. This
# self-test isolates the parts of smoke-oci.sh that do not depend on a
# running container: the docker/podman runtime guard, and the inline Python
# parser that turns a `docker stats`/`podman stats` memory sample into a
# byte count.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script="${here}/smoke-oci.sh"
work_dir="$(mktemp -d)"
failures=0
cleanup() {
  rm -rf "$work_dir"
}
trap cleanup EXIT

check() {
  local name="$1" expected="$2"; shift 2
  local status=0
  "$@" >"${work_dir}/out" 2>"${work_dir}/err" || status=$?
  if [[ "$status" -eq "$expected" ]]; then
    printf '  ok    %-56s exit %s\n' "$name" "$status"
  else
    printf '  FAIL  %-56s exit %s, wanted %s\n' "$name" "$status" "$expected"
    sed 's/^/        /' "${work_dir}/err" | tail -3
    failures=$((failures + 1))
  fi
}

check_stderr_contains() {
  local name="$1" needle="$2"
  if grep -Fq "$needle" "${work_dir}/err"; then
    printf '  ok    %-56s stderr contains expected text\n' "$name"
  else
    printf '  FAIL  %-56s stderr missing: %s\n' "$name" "$needle"
    failures=$((failures + 1))
  fi
}

echo "smoke-oci.sh self-test"

# The runtime guard must run before anything touches a container engine, so
# it is exercisable without Docker or Podman installed.
check "rejects an unsupported OCI runtime" 1 \
  bash "$script" fasti:b0 "" buildah
check_stderr_contains "rejects an unsupported OCI runtime" \
  "OCI runtime must be docker or podman"

check "rejects a case-mismatched runtime name" 1 \
  bash "$script" fasti:b0 "" Docker
check_stderr_contains "rejects a case-mismatched runtime name" \
  "OCI runtime must be docker or podman"

# The default `oci_runtime="${3:-docker}"` substitution treats an empty
# third argument the same as an omitted one (bash's `:-` triggers on both
# unset and empty parameters), so it falls through to "docker" rather than
# tripping the guard.
check "empty runtime override falls back to the docker default" 0 \
  bash -c 'oci_runtime="${1:-docker}"; [[ "$oci_runtime" == "docker" ]]' -- ""

# The idle-memory sample parser is embedded in smoke-oci.sh as an inline
# Python heredoc so it can turn `docker stats`/`podman stats` output like
# "45.3MiB" into a byte count. Extract that exact heredoc body (rather than
# reimplementing it) so this test tracks the shipped script instead of a
# hand-copied duplicate.
parser_file="${work_dir}/memory_parser.py"
awk '
  /"\$memory_sample"/ { capture=1; next }
  capture && /^PY$/ { capture=0; next }
  capture { print }
' "$script" >"$parser_file"

if [[ ! -s "$parser_file" ]]; then
  echo "FAIL  could not locate the embedded OCI memory-sample parser in $script" >&2
  failures=$((failures + 1))
fi

assert_memory_bytes() {
  local name="$1" sample="$2" expected="$3"
  local actual
  actual="$(python3 "$parser_file" "$sample")"
  if [[ "$actual" == "$expected" ]]; then
    printf '  ok    %-56s %s -> %s bytes\n' "$name" "$sample" "$actual"
  else
    printf '  FAIL  %-56s %s -> %s bytes, wanted %s\n' "$name" "$sample" "$actual" "$expected"
    failures=$((failures + 1))
  fi
}

assert_memory_bytes "parses whole MiB samples (docker format)" "40MiB" "41943040"
assert_memory_bytes "parses fractional MiB samples (podman format)" "45.3MiB" "47500493"
assert_memory_bytes "parses whole GiB samples" "1GiB" "1073741824"
assert_memory_bytes "parses raw byte samples" "512B" "512"
assert_memory_bytes "parses KiB samples" "2048KiB" "2097152"

if python3 "$parser_file" "not-a-memory-value" >"${work_dir}/out" 2>"${work_dir}/err"; then
  printf '  FAIL  %-56s expected a parse failure\n' "rejects an unrecognized memory format"
  failures=$((failures + 1))
else
  check_stderr_contains "rejects an unrecognized memory format" \
    "Unrecognized OCI runtime memory value"
fi

if (( failures > 0 )); then
  echo "smoke-oci.sh self-test: ${failures} check(s) failed" >&2
  exit 1
fi

echo "smoke-oci.sh self-test: runtime guard and memory parser behave as shipped"