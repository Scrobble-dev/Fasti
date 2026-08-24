#!/usr/bin/env bash
# Run a command inside a kernel-enforced low-hardware envelope and report its
# peak memory.
#
# Fasti's performance bar exists so the product works on old or small hardware.
# Measuring a 64 MiB idle budget on a large CI runner proves almost nothing: it
# shows only that the process did not leak the runner's whole memory. A cgroup
# v2 envelope makes the claim real and reproducible, because the kernel, not the
# harness, enforces the limit. A process that exceeds it is killed.
#
# Budgets come from benchmarks/b1/budgets.json. They are not duplicated here.
#
# Usage:
#   scripts/bench-envelope.sh --target idle -- ./target/release/fastid --once
#   scripts/bench-envelope.sh --target heavy -- cargo test --release
#
# Targets map to budgets.json memory_bytes keys: idle, normal, heavy, ceiling.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
budgets="${repo_root}/benchmarks/b1/budgets.json"
target="idle"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) target="$2"; shift 2 ;;
    --) shift; break ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ $# -eq 0 ]]; then
  echo "No command given. Usage: scripts/bench-envelope.sh [--target idle|normal|heavy|ceiling] -- <command>" >&2
  exit 2
fi

if [[ ! -f "$budgets" ]]; then
  echo "Missing budgets file: $budgets" >&2
  exit 1
fi

read -r budget_bytes ceiling_bytes < <(python3 - "$budgets" "$target" <<'PY'
import json
import sys

budgets = json.load(open(sys.argv[1]))["memory_bytes"]
keys = {
    "idle": "idle_target",
    "normal": "normal_target",
    "heavy": "heavy_target",
    "ceiling": "absolute_ceiling",
}
target = sys.argv[2]
if target not in keys:
    raise SystemExit(f"Unknown target {target!r}; expected one of {sorted(keys)}")
print(budgets[keys[target]], budgets["absolute_ceiling"])
PY
)

# The envelope is always the absolute ceiling: that is the hard limit the
# product promises never to cross. The per-target budget is then asserted
# against the measured peak, so a run can be killed by the kernel (ceiling
# breach) or fail the assertion (target breach) and the two are distinguishable.
ceiling_mib=$(( ceiling_bytes / 1048576 ))
budget_mib=$(( budget_bytes / 1048576 ))

# Two routes into a delegated cgroup. A developer session usually has a user
# manager; CI runners usually do not, but do have passwordless sudo. If neither
# works the script FAILS rather than running unconstrained, because an
# unenforced envelope would report a number that means nothing.
scope_properties=(
  -p "MemoryMax=${ceiling_mib}M"
  -p "MemorySwapMax=0"
  -p "CPUQuota=100%"
)

if systemd-run --user --scope --quiet "${scope_properties[@]}" -- true 2>/dev/null; then
  runner=(systemd-run --user --scope --quiet "${scope_properties[@]}" --)
elif sudo -n true 2>/dev/null && sudo systemd-run --scope --quiet "${scope_properties[@]}" -- true 2>/dev/null; then
  runner=(sudo systemd-run --scope --quiet "${scope_properties[@]}" --)
else
  {
    echo "Cannot create a cgroup v2 memory envelope; refusing to report an unenforced measurement."
    echo "  cgroup fs     : $(stat -fc %T /sys/fs/cgroup 2>/dev/null || echo unknown)"
    echo "  systemd-run   : $(command -v systemd-run || echo missing)"
    echo "  user manager  : $(systemd-run --user --scope --quiet -- true 2>&1 || true)"
    echo "  passwordless  : $(sudo -n true 2>&1 && echo yes || echo no)"
  } >&2
  exit 1
fi

peak_file="$(mktemp)"
cleanup() { rm -f "$peak_file"; }
trap cleanup EXIT

# Allocator arenas and async worker pools both scale with core count. Left
# unpinned, a many-core runner inflates the footprint for reasons that have
# nothing to do with the product, and the measurement stops being comparable to
# a small machine.
export MALLOC_ARENA_MAX="${MALLOC_ARENA_MAX:-2}"
export TOKIO_WORKER_THREADS="${TOKIO_WORKER_THREADS:-2}"

# systemd-run performs its own $VAR and ${VAR} expansion on the command line
# before the shell sees it, which silently blanks variables and emits warnings.
# Passing a file instead of an inline string removes that whole class of
# problem.
inner="$(mktemp)"
cleanup_inner() { rm -f "$inner"; }
trap 'cleanup; cleanup_inner' EXIT
cat > "$inner" <<'INNER'
peak_file="$1"; shift
own_cgroup="/sys/fs/cgroup$(awk -F: '{print $3}' /proc/self/cgroup)"
"$@"
status=$?
# memory.peak is the kernel watermark for the whole scope. getrusage ru_maxrss
# misses transient spikes, which is exactly what a ceiling cares about.
cat "$own_cgroup/memory.peak" 2>/dev/null > "$peak_file" || true
exit $status
INNER

set +e
"${runner[@]}" bash "$inner" "$peak_file" "$@"
status=$?
set -e

# Any signal death (128 + signo) inside the envelope means the kernel stopped
# the process. Do not match a single signal number: whether the OOM killer
# lands SIGKILL on the workload or systemd tears the scope down with SIGTERM
# depends on which task the kernel picks, and both mean the same thing here.
if (( status > 128 )); then
  echo "Command was killed inside the ${ceiling_mib} MiB envelope by signal $(( status - 128 ))." >&2
  echo "This is a hard ceiling breach, not a budget overshoot." >&2
  exit 1
fi

peak_bytes="$(cat "$peak_file" 2>/dev/null || true)"
if [[ -z "$peak_bytes" ]]; then
  echo "Envelope produced no memory.peak reading; refusing to claim a measurement." >&2
  exit 1
fi

peak_mib=$(( peak_bytes / 1048576 ))

if (( peak_bytes > budget_bytes )); then
  echo "Peak memory ${peak_mib} MiB exceeds the ${target} budget of ${budget_mib} MiB (envelope ceiling ${ceiling_mib} MiB)" >&2
  exit 1
fi

echo "envelope ${target}: peak ${peak_mib} MiB within ${budget_mib} MiB budget, enforced ceiling ${ceiling_mib} MiB, command exit ${status}"
exit "$status"
