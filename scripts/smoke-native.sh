#!/usr/bin/env bash
# Native network-denied smoke.
#
# scripts/smoke-oci.sh proves the container path works with `--network none`.
# Nothing proved the same for the native binaries, and B8a requires native and
# OCI distribution readiness, not one of the two. A daemon that quietly needs
# the network would look healthy under Docker's isolation and fail on a real
# offline host.
#
# Usage: scripts/smoke-native.sh [path-to-target-dir]
set -euo pipefail

target_dir="${1:-target/release}"
daemon="${target_dir}/fastid"
cli="${target_dir}/fasti"

for binary in "$daemon" "$cli"; do
  if [[ ! -x "$binary" ]]; then
    echo "Missing native binary $binary; build with: cargo build --locked --release --bin fastid --bin fasti" >&2
    exit 1
  fi
done

if ! unshare --user --map-root-user --net -- true 2>/dev/null; then
  echo "This host cannot create a user+network namespace; native offline proof cannot run here" >&2
  exit 1
fi

work_dir="$(mktemp -d)"
cleanup() { rm -rf "$work_dir"; }
trap cleanup EXIT

# Everything below runs inside a namespace with no interface except loopback.
# `ip link set lo up` is required: a fresh netns has lo present but DOWN, so a
# daemon binding 127.0.0.1 would fail for the wrong reason.
unshare --user --map-root-user --net -- bash -euo pipefail -c '
work_dir="$1"
daemon="$2"
cli="$3"

ip link set lo up

# 1. Prove the isolation is real before trusting anything that follows.
#    Without this, a namespace that silently failed to isolate would make every
#    later assertion pass for the wrong reason.
if timeout 5 getent hosts github.com >/dev/null 2>&1; then
  echo "Network namespace did not isolate: DNS still resolves" >&2
  exit 1
fi
if timeout 5 bash -c "exec 3<>/dev/tcp/1.1.1.1/443" 2>/dev/null; then
  echo "Network namespace did not isolate: external TCP still connects" >&2
  exit 1
fi

# 2. The guarded CLI must fail explicitly and quietly, offline, exactly as it
#    does in the container path.
cli_stdout="${work_dir}/cli.out"
cli_stderr="${work_dir}/cli.err"
if "$cli" verify >"$cli_stdout" 2>"$cli_stderr"; then
  echo "Unavailable verify command reported success" >&2
  exit 1
fi
if [[ -s "$cli_stdout" ]] \
  || ! grep -Fq "capability_id=portability.workspace.verify" "$cli_stderr" \
  || ! grep -Fq "owned by B3" "$cli_stderr"; then
  echo "Unavailable verify command did not fail explicitly and quietly offline" >&2
  cat "$cli_stderr" >&2
  exit 1
fi

# 3. The daemon must start and serve health over loopback with no external
#    network at all.
export FASTI_LISTEN=127.0.0.1:8420
export FASTI_DATA_ROOT="${work_dir}/data"
"$daemon" >"${work_dir}/daemon.log" 2>&1 &
daemon_pid=$!
trap "kill ${daemon_pid} 2>/dev/null || true" EXIT

health=""
for _ in $(seq 1 30); do
  if health="$(timeout 2 bash -c "
      exec 3<>/dev/tcp/127.0.0.1/8420 || exit 1
      printf \"GET /api/v1/health HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n\" >&3
      cat <&3
    " 2>/dev/null)"; then
    if [[ -n "$health" ]]; then
      break
    fi
  fi
  sleep 1
done

if [[ -z "$health" ]]; then
  cat "${work_dir}/daemon.log" >&2
  echo "Native daemon did not become healthy with the network denied" >&2
  exit 1
fi

body="${health##*$'"'"'\r\n\r\n'"'"'}"
python3 - "$body" <<'"'"'PY'"'"'
import json
import sys

payload = json.loads(sys.argv[1])
if payload.get("status") != "healthy" or not payload.get("version"):
    raise SystemExit(f"Unexpected network-denied health payload: {payload!r}")
PY

echo "native offline smoke: CLI guard and daemon health both pass with no network"
' _ "$work_dir" "$(realpath "$daemon")" "$(realpath "$cli")"
