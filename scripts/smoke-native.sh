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

# Pick a way into an isolated network namespace. Unprivileged user namespaces
# work on a developer box; GitHub Actions runners restrict them, so fall back to
# sudo, which runners have passwordless. If neither works the script FAILS
# rather than skipping: a gate that quietly does nothing is worse than no gate.
if unshare --user --map-root-user --net -- true 2>/dev/null; then
  isolate=(unshare --user --map-root-user --net --)
  privileged=0
elif sudo -n true 2>/dev/null && sudo unshare --net -- true 2>/dev/null; then
  isolate=(sudo unshare --net --)
  privileged=1
else
  # Report what was actually tried, so the next failure explains itself instead
  # of needing a local repro. Ubuntu 24.04 sets
  # kernel.apparmor_restrict_unprivileged_userns=1, which is why the
  # unprivileged route works on many developer machines and not on CI runners.
  {
    echo "Cannot create a network namespace by either route; native offline proof cannot run here."
    echo "  unprivileged userns : $(unshare --user --map-root-user --net -- true 2>&1 || true)"
    echo "  passwordless sudo   : $(sudo -n true 2>&1 && echo available || echo unavailable)"
    echo "  apparmor restriction: $(sysctl -n kernel.apparmor_restrict_unprivileged_userns 2>/dev/null || echo unknown)"
  } >&2
  exit 1
fi

idle_limit_mib="${FASTI_IDLE_MEMORY_LIMIT_MIB:-64}"
work_dir="$(mktemp -d)"
# Under the sudo route the daemon writes as root, so plain rm can fail.
cleanup() { rm -rf "$work_dir" 2>/dev/null || sudo rm -rf "$work_dir" 2>/dev/null || true; }
trap cleanup EXIT

# Everything below runs inside a namespace with no interface except loopback.
# `ip link set lo up` is required: a fresh netns has lo present but DOWN, so a
# daemon binding 127.0.0.1 would fail for the wrong reason.
"${isolate[@]}" bash -euo pipefail -c '
work_dir="$1"
daemon="$2"
cli="$3"
idle_limit_mib="$4"

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

# 4. Prove the shipped loopback daemon can perform durable one-time bootstrap.
# Secrets stay inside this process and are never written to logs or URLs. The
# bootstrap secret proves the caller can read a file the daemon'"'"'s OS user
# owns -- the same local-filesystem trust boundary the data root lock assumes.
python3 - "${work_dir}/data/bootstrap.secret" <<'"'"'PY'"'"'
import http.client
import json
import re
import sys

bootstrap_secret = open(sys.argv[1]).read().strip()

def post(path, payload, bearer=None):
    connection = http.client.HTTPConnection("127.0.0.1", 8420, timeout=5)
    headers = {"content-type": "application/json"}
    if bearer is not None:
        headers["authorization"] = f"Bearer {bearer}"
    connection.request("POST", path, body=json.dumps(payload), headers=headers)
    response = connection.getresponse()
    body = json.loads(response.read())
    connection.close()
    return response.status, body

status, initialized = post("/api/v1/node/initialization", {}, bearer=bootstrap_secret)
if status != 200 or not re.fullmatch(r"[0-9a-f]{64}", initialized.get("initialization_proof", "")):
    raise SystemExit("Durable node initialization failed")

status, enrolled = post(
    "/api/v1/client-enrollments",
    {"initialization_proof": initialized["initialization_proof"]},
)
if (
    status != 200
    or enrolled.get("credential_scheme") != "Bearer"
    or not re.fullmatch(r"[0-9a-f]{64}", enrolled.get("credential", ""))
):
    raise SystemExit("Durable first-client enrollment failed")

status, problem = post("/api/v1/node/initialization", {}, bearer=bootstrap_secret)
if status != 409 or problem.get("code") != "already_initialized":
    raise SystemExit("One-time node initialization did not close after enrollment")
PY

# Idle memory. scripts/smoke-oci.sh already holds the container path to this
# budget; the native path was unbounded, so the readiness gate could pass with a
# daemon over the 64 MiB idle target. Measured from /proc rather than enforced
# with a cgroup, matching the OCI smoke, which also measures. An enforced limit
# would change what the test proves: it would show the daemon SURVIVES a cap,
# not what it actually uses.
rss_kib="$(awk "/^VmRSS:/ {print \$2}" "/proc/${daemon_pid}/status")"
if [[ -z "$rss_kib" ]]; then
  echo "Could not read idle memory for the native daemon" >&2
  exit 1
fi
# Compare in KiB. Truncating to whole MiB first would let 65537 KiB read as
# 64 MiB and pass a 64 MiB budget, so the gate would be loose by up to 1023 KiB.
rss_mib=$(( rss_kib / 1024 ))
if (( rss_kib > idle_limit_mib * 1024 )); then
  echo "Native daemon idle memory ${rss_mib} MiB exceeds the ${idle_limit_mib} MiB budget" >&2
  exit 1
fi

echo "native offline smoke: CLI guard, durable bootstrap, daemon health, and ${rss_mib} MiB idle memory all pass with no network"
' _ "$work_dir" "$(realpath "$daemon")" "$(realpath "$cli")" "$idle_limit_mib"
