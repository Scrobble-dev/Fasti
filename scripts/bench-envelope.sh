#!/usr/bin/env bash
# Run a command inside a kernel-enforced low-hardware envelope. The canonical
# idle profile also samples steady memory and CPU for the governed window.
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
#   scripts/bench-envelope.sh --target idle \
#     --profile canonical-idle \
#     --receipt target/fasti-evidence/envelope/x86_64/receipt.json \
#     --artifact target/release/fastid --build-profile release \
#     --artifact-budget-receipt target/fasti-artifact-stage/x86_64/receipt.json \
#     -- bash scripts/bench-daemon-idle.sh target/release/fastid
#
# Targets map to budgets.json memory_bytes keys: idle, normal, heavy, ceiling.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
budgets="${repo_root}/benchmarks/b1/budgets.json"
target="idle"
receipt=""
artifact=""
build_profile=""
profile="startup-smoke"
artifact_budget_receipt=""
max_sample_lateness_ns=500000000

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target|--receipt|--artifact|--build-profile|--profile|--artifact-budget-receipt)
      if [[ $# -lt 2 ]]; then
        echo "$1 requires a value" >&2
        exit 2
      fi
      case "$1" in
        --target) target="$2" ;;
        --receipt) receipt="$2" ;;
        --artifact) artifact="$2" ;;
        --build-profile) build_profile="$2" ;;
        --profile) profile="$2" ;;
        --artifact-budget-receipt) artifact_budget_receipt="$2" ;;
      esac
      shift 2
      ;;
    --) shift; break ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ $# -eq 0 ]]; then
  echo "No command given. Usage: scripts/bench-envelope.sh [--target idle|normal|heavy|ceiling] -- <command>" >&2
  exit 2
fi

case "$profile" in
  startup-smoke) ;;
  canonical-idle)
    if [[ "$target" != "idle" ]]; then
      echo "The canonical-idle profile requires --target idle" >&2
      exit 2
    fi
    ;;
  *) echo "Unknown profile: $profile; expected startup-smoke or canonical-idle" >&2; exit 2 ;;
esac

if [[ ! -f "$budgets" ]]; then
  echo "Missing budgets file: $budgets" >&2
  exit 1
fi

if [[ -n "$receipt" ]]; then
  if [[ -z "$artifact" || "$build_profile" != "release" ]]; then
    echo "--receipt requires --artifact <release binary> and --build-profile release" >&2
    exit 2
  fi
  if [[ ! -f "$artifact" || -L "$artifact" ]]; then
    echo "Receipt artifact must be a regular non-symlink file: $artifact" >&2
    exit 1
  fi
  artifact_absolute="$(realpath "$artifact")"
  case "$artifact_absolute" in
    "$repo_root"/*) ;;
    *) echo "Receipt artifact must remain inside $repo_root" >&2; exit 1 ;;
  esac
  artifact_relative="${artifact_absolute#"${repo_root}/"}"
  artifact_sha256_before="$(sha256sum "$artifact_absolute" | awk '{print $1}')"
  artifact_size="$(stat -c %s "$artifact_absolute")"
  if [[ "$profile" == "canonical-idle" && -z "$artifact_budget_receipt" ]]; then
    echo "Canonical idle receipts require --artifact-budget-receipt" >&2
    exit 2
  fi
  artifact_budget_receipt_absolute=""
  if [[ -n "$artifact_budget_receipt" ]]; then
    if [[ ! -f "$artifact_budget_receipt" || -L "$artifact_budget_receipt" ]]; then
      echo "Artifact budget receipt must be a regular non-symlink file: $artifact_budget_receipt" >&2
      exit 1
    fi
    artifact_budget_receipt_absolute="$(realpath "$artifact_budget_receipt")"
    case "$artifact_budget_receipt_absolute" in
      "$repo_root"/*) ;;
      *) echo "Artifact budget receipt must remain inside $repo_root" >&2; exit 1 ;;
    esac
  fi
  if [[ -n "${FASTI_ENVELOPE_TEST_WARMUP_SECONDS:-}" || -n "${FASTI_ENVELOPE_TEST_MEASUREMENT_SECONDS:-}" ]]; then
    echo "Test timing overrides cannot produce a receipt" >&2
    exit 2
  fi
fi

read -r budget_bytes ceiling_bytes warmup_seconds measurement_seconds sample_interval_ms cpu_average_limit_bp cpu_p95_limit_bp < <(python3 - "$budgets" "$target" <<'PY'
from decimal import Decimal
import json
import sys

document = json.load(open(sys.argv[1]))
budgets = document["memory_bytes"]
keys = {
    "idle": "idle_target",
    "normal": "normal_target",
    "heavy": "heavy_target",
    "ceiling": "absolute_ceiling",
}
target = sys.argv[2]
if target not in keys:
    raise SystemExit(f"Unknown target {target!r}; expected one of {sorted(keys)}")
timing = document["timing_seconds"]
cpu = document["idle_cpu_percent_one_core"]
print(
    budgets[keys[target]], budgets["absolute_ceiling"],
    timing["idle_warmup"], timing["idle_measurement"], timing["sample_interval_ms"],
    int(Decimal(str(cpu["average"])) * 100), int(Decimal(str(cpu["p95"])) * 100),
)
PY
)

if [[ "$profile" == "canonical-idle" && -z "$receipt" ]]; then
  warmup_seconds="${FASTI_ENVELOPE_TEST_WARMUP_SECONDS:-$warmup_seconds}"
  measurement_seconds="${FASTI_ENVELOPE_TEST_MEASUREMENT_SECONDS:-$measurement_seconds}"
fi

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

# Prefer the user scope only when it can also create the required network namespace.
if systemd-run --user --scope --quiet "${scope_properties[@]}" -- true 2>/dev/null &&
  { [[ "$profile" != "canonical-idle" ]] || unshare --user --map-root-user --net -- true 2>/dev/null; }
then
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

temporary_root="$(mktemp -d)"
peak_file="${temporary_root}/peak"
events_file="${temporary_root}/events"
limits_file="${temporary_root}/limits"
cgroup_file="${temporary_root}/cgroup"
samples_file="${temporary_root}/samples"
touch "$peak_file" "$events_file" "$limits_file" "$cgroup_file" "$samples_file"
cleanup() { rm -rf "$temporary_root"; }
trap cleanup EXIT

# Allocator arenas and async worker pools both scale with core count. Left
# unpinned, a many-core runner inflates the footprint for reasons that have
# nothing to do with the product, and the measurement stops being comparable to
# a small machine.
export MALLOC_ARENA_MAX=2
export TOKIO_WORKER_THREADS=2

# systemd-run performs its own $VAR and ${VAR} expansion on the command line
# before the shell sees it, which silently blanks variables and emits warnings.
# Passing a file instead of an inline string removes that whole class of
# problem.
inner="${temporary_root}/inner"
cat > "$inner" <<'INNER'
peak_file="$1"; shift
events_file="$1"; shift
limits_file="$1"; shift
cgroup_file="$1"; shift
profile="$1"; shift
own_cgroup="/sys/fs/cgroup$(awk -F: '{print $3}' /proc/self/cgroup)"
printf '%s\n' "$own_cgroup" > "$cgroup_file"
if [[ "$profile" == "canonical-idle" ]]; then
  # A sudo-created scope is already root; remapping root can be denied by the host.
  if (( EUID == 0 )); then
    isolate=(unshare --net)
  else
    isolate=(unshare --user --map-root-user --net)
  fi
  "${isolate[@]}" bash -c '
    ip link set lo up
    if [[ -n "$(ip route show)" ]]; then
      echo "Canonical idle namespace unexpectedly has an IP route." >&2
      exit 1
    fi
    exec "$@"
  ' bash "$@"
else
  "$@"
fi
status=$?
# memory.peak is the kernel watermark for the whole scope. getrusage ru_maxrss
# misses transient spikes, which is exactly what a ceiling cares about.
cat "$own_cgroup/memory.peak" 2>/dev/null > "$peak_file" || true
cat "$own_cgroup/memory.events" 2>/dev/null > "$events_file" || true
{
  printf 'memory_max=%s\n' "$(cat "$own_cgroup/memory.max" 2>/dev/null)"
  printf 'memory_swap_max=%s\n' "$(cat "$own_cgroup/memory.swap.max" 2>/dev/null)"
  printf 'cpu_max=%s\n' "$(cat "$own_cgroup/cpu.max" 2>/dev/null)"
  if [[ -f "$own_cgroup/memory.swap.peak" ]]; then
    printf 'memory_swap_peak=%s\n' "$(cat "$own_cgroup/memory.swap.peak" 2>/dev/null)"
  else
    printf 'memory_swap_peak=%s\n' "$(cat "$own_cgroup/memory.swap.current" 2>/dev/null)"
  fi
} > "$limits_file"
exit $status
INNER

sampler_status=0
if [[ "$profile" == "canonical-idle" ]]; then
  export FASTI_IDLE_SETTLE_SECONDS=$(( warmup_seconds + measurement_seconds + 30 ))
  set +e
  "${runner[@]}" bash "$inner" "$peak_file" "$events_file" "$limits_file" "$cgroup_file" "$profile" "$@" &
  runner_pid=$!
  set -e

  for _ in $(seq 1 300); do
    [[ -s "$cgroup_file" ]] && break
    kill -0 "$runner_pid" 2>/dev/null || break
    sleep 0.1
  done
  cgroup_path="$(cat "$cgroup_file" 2>/dev/null || true)"
  if [[ -z "$cgroup_path" || ! -d "$cgroup_path" ]]; then
    echo "Canonical idle measurement could not discover the enforced cgroup." >&2
    sampler_status=1
  else
    set +e
    python3 - "$cgroup_path" "$runner_pid" "$warmup_seconds" "$measurement_seconds" \
      "$sample_interval_ms" "$max_sample_lateness_ns" "$samples_file" <<'PY'
import json
import os
from pathlib import Path
import sys
import time

cgroup = Path(sys.argv[1])
runner_pid = int(sys.argv[2])
warmup_seconds = int(sys.argv[3])
measurement_seconds = int(sys.argv[4])
sample_interval_ms = int(sys.argv[5])
max_sample_lateness_ns = int(sys.argv[6])
output = Path(sys.argv[7])

if warmup_seconds < 0 or measurement_seconds <= 0 or sample_interval_ms <= 0:
    raise SystemExit("canonical idle timing is invalid")
if measurement_seconds * 1000 % sample_interval_ms:
    raise SystemExit("canonical idle duration must contain a whole number of samples")

def read_cpu_usage_micros():
    for line in (cgroup / "cpu.stat").read_text().splitlines():
        key, value = line.split()
        if key == "usage_usec":
            return int(value)
    raise RuntimeError("cgroup cpu.stat omits usage_usec")

def require_runner():
    os.kill(runner_pid, 0)
    if not cgroup.is_dir():
        raise RuntimeError("measured cgroup disappeared")

run_started = time.monotonic_ns()
warmup_deadline = run_started + warmup_seconds * 1_000_000_000
time.sleep(max(0, (warmup_deadline - time.monotonic_ns()) / 1_000_000_000))
require_runner()
measurement_started = time.monotonic_ns()
if measurement_started > warmup_deadline + max_sample_lateness_ns:
    raise RuntimeError("canonical idle warm-up missed its deadline")
previous_at = measurement_started
previous_cpu = read_cpu_usage_micros()
observations = []
sample_count = measurement_seconds * 1000 // sample_interval_ms

for sequence in range(1, sample_count + 1):
    deadline = measurement_started + sequence * sample_interval_ms * 1_000_000
    time.sleep(max(0, (deadline - time.monotonic_ns()) / 1_000_000_000))
    require_runner()
    observed_at = time.monotonic_ns()
    if observed_at > deadline + max_sample_lateness_ns:
        raise RuntimeError(f"canonical idle sample {sequence} missed its deadline")
    cpu_usage = read_cpu_usage_micros()
    interval_ns = observed_at - previous_at
    cpu_delta = cpu_usage - previous_cpu
    cpu_basis_points = (cpu_delta * 10_000_000 + interval_ns - 1) // interval_ns
    observations.append({
        "sequence": sequence,
        "elapsed_ns": observed_at - measurement_started,
        "interval_ns": interval_ns,
        "memory_current_bytes": int((cgroup / "memory.current").read_text()),
        "cpu_usage_delta_micros": cpu_delta,
        "cpu_basis_points": cpu_basis_points,
    })
    previous_at = observed_at
    previous_cpu = cpu_usage

document = {
    "actual_warmup_ns": measurement_started - run_started,
    "actual_measurement_ns": observations[-1]["elapsed_ns"],
    "observations": observations,
}
with output.open("w", encoding="utf-8") as stream:
    json.dump(document, stream, separators=(",", ":"))
    stream.write("\n")
PY
    sampler_status=$?
    set -e
  fi

  set +e
  wait "$runner_pid"
  status=$?
  set -e
else
  set +e
  "${runner[@]}" bash "$inner" "$peak_file" "$events_file" "$limits_file" "$cgroup_file" "$profile" "$@"
  status=$?
  set -e
fi

if (( sampler_status != 0 )); then
  echo "Canonical idle sampling failed; no passing measurement is available." >&2
  exit 1
fi

# The cgroup must remain OOM-free even when a parent process masks a killed
# child. Checking only exit status would let that failure report success.
oom_count="$(grep -E '^(oom|oom_kill) ' "$events_file" 2>/dev/null | awk '{sum += $2} END {print sum+0}')"
if (( oom_count > 0 )); then
  if (( status > 128 )); then
    echo "Command was killed inside the ${ceiling_mib} MiB envelope by signal $(( status - 128 ))." >&2
  else
    echo "The envelope recorded ${oom_count} OOM event(s), even though the command returned ${status}." >&2
  fi
  echo "This is a hard ceiling breach, not a budget overshoot." >&2
  exit 1
fi

peak_bytes="$(cat "$peak_file" 2>/dev/null || true)"
memory_max="$(sed -n 's/^memory_max=//p' "$limits_file")"
memory_swap_max="$(sed -n 's/^memory_swap_max=//p' "$limits_file")"
cpu_max="$(sed -n 's/^cpu_max=//p' "$limits_file")"
memory_swap_peak="$(sed -n 's/^memory_swap_peak=//p' "$limits_file")"
read -r cpu_quota cpu_period <<< "$cpu_max"

if [[ ! "$peak_bytes" =~ ^[0-9]+$ ]]; then
  echo "Envelope produced no memory.peak reading; refusing to claim a measurement." >&2
  exit 1
fi
if [[ ! "$memory_max" =~ ^[0-9]+$ ]] || (( memory_max != ceiling_bytes )); then
  echo "Applied memory.max is ${memory_max:-missing}; expected ${ceiling_bytes}." >&2
  exit 1
fi
if [[ "$memory_swap_max" != "0" || ! "$memory_swap_peak" =~ ^[0-9]+$ ]] || (( memory_swap_peak != 0 )); then
  echo "Applied zero-swap envelope is invalid: max=${memory_swap_max:-missing}, peak=${memory_swap_peak:-missing}." >&2
  exit 1
fi
if [[ ! "$cpu_quota" =~ ^[0-9]+$ || ! "$cpu_period" =~ ^[0-9]+$ ]] || (( cpu_quota != cpu_period )); then
  echo "Applied cpu.max is ${cpu_max:-missing}; expected a finite one-vCPU quota." >&2
  exit 1
fi

steady_memory_peak_bytes="$peak_bytes"
cpu_average_basis_points=0
cpu_p95_basis_points=0
actual_warmup_ns=0
actual_measurement_ns=0
if [[ "$profile" == "canonical-idle" ]]; then
  read -r steady_memory_peak_bytes cpu_average_basis_points cpu_p95_basis_points actual_warmup_ns actual_measurement_ns < <(
    python3 - "$samples_file" "$warmup_seconds" "$measurement_seconds" "$sample_interval_ms" \
      "$max_sample_lateness_ns" <<'PY'
import json
from pathlib import Path
import sys

document = json.loads(Path(sys.argv[1]).read_text())
warmup_seconds = int(sys.argv[2])
measurement_seconds = int(sys.argv[3])
sample_interval_ms = int(sys.argv[4])
max_sample_lateness_ns = int(sys.argv[5])
observations = document["observations"]
expected_count = measurement_seconds * 1000 // sample_interval_ms
if len(observations) != expected_count or not observations:
    raise SystemExit("canonical idle sample count is invalid")
if document["actual_warmup_ns"] < warmup_seconds * 1_000_000_000:
    raise SystemExit("canonical idle warm-up ended early")
if document["actual_warmup_ns"] > warmup_seconds * 1_000_000_000 + max_sample_lateness_ns:
    raise SystemExit("canonical idle warm-up missed its deadline")
if document["actual_measurement_ns"] < measurement_seconds * 1_000_000_000:
    raise SystemExit("canonical idle measurement ended early")
if document["actual_measurement_ns"] > measurement_seconds * 1_000_000_000 + max_sample_lateness_ns:
    raise SystemExit("canonical idle measurement missed its deadline")
for sequence, observation in enumerate(observations, 1):
    if observation["sequence"] != sequence or observation["interval_ns"] <= 0:
        raise SystemExit("canonical idle observation sequence is invalid")
    deadline = sequence * sample_interval_ms * 1_000_000
    if not deadline <= observation["elapsed_ns"] <= deadline + max_sample_lateness_ns:
        raise SystemExit(f"canonical idle sample {sequence} missed its deadline")
steady_peak = max(observation["memory_current_bytes"] for observation in observations)
total_cpu_micros = sum(observation["cpu_usage_delta_micros"] for observation in observations)
average_bp = (
    total_cpu_micros * 10_000_000 + document["actual_measurement_ns"] - 1
) // document["actual_measurement_ns"]
ordered = sorted(observation["cpu_basis_points"] for observation in observations)
p95_bp = ordered[(95 * len(ordered) + 99) // 100 - 1]
print(
    steady_peak, average_bp, p95_bp,
    document["actual_warmup_ns"], document["actual_measurement_ns"],
)
PY
  )
  if [[ -z "$actual_measurement_ns" ]]; then
    echo "Canonical idle sample summary is invalid." >&2
    exit 1
  fi
  if (( steady_memory_peak_bytes > budget_bytes )); then
    echo "Steady idle memory ${steady_memory_peak_bytes} bytes exceeds the ${budget_mib} MiB idle budget." >&2
    exit 1
  fi
  if (( cpu_average_basis_points > cpu_average_limit_bp || cpu_p95_basis_points > cpu_p95_limit_bp )); then
    echo "Idle CPU exceeds policy: average ${cpu_average_basis_points} bp/${cpu_average_limit_bp} bp, p95 ${cpu_p95_basis_points} bp/${cpu_p95_limit_bp} bp." >&2
    exit 1
  fi
fi

peak_mib=$(( peak_bytes / 1048576 ))

if [[ "$profile" == "startup-smoke" ]] && (( peak_bytes > budget_bytes )); then
  echo "Peak memory ${peak_mib} MiB exceeds the ${target} budget of ${budget_mib} MiB (envelope ceiling ${ceiling_mib} MiB)" >&2
  exit $(( status != 0 ? status : 1 ))
fi

if (( status != 0 )); then
  echo "Command exited ${status}; no passing receipt was written." >&2
  exit "$status"
fi

if [[ -n "$receipt" ]]; then
  artifact_sha256_after="$(sha256sum "$artifact_absolute" | awk '{print $1}')"
  if [[ "$artifact_sha256_after" != "$artifact_sha256_before" ]]; then
    echo "Measured artifact changed during the envelope run." >&2
    exit 1
  fi

  architecture="$(uname -m)"
  case "$architecture" in
    x86_64|amd64) architecture="x86_64" ;;
    aarch64|arm64) architecture="aarch64" ;;
    *) echo "Unsupported receipt architecture: $architecture" >&2; exit 1 ;;
  esac
  if [[ -n "${FASTI_ENVELOPE_ARCH:-}" && "$FASTI_ENVELOPE_ARCH" != "$architecture" ]]; then
    echo "Declared envelope architecture ${FASTI_ENVELOPE_ARCH} does not match kernel architecture ${architecture}." >&2
    exit 1
  fi

  dirty=false
  if [[ -n "$(git -C "$repo_root" status --porcelain=v1 --untracked-files=all)" ]]; then
    dirty=true
  fi
  git_commit="$(git -C "$repo_root" rev-parse --verify HEAD)"
  git_tree="$(git -C "$repo_root" rev-parse 'HEAD^{tree}')"
  budgets_sha256="$(sha256sum "$budgets" | awk '{print $1}')"
  harness_sha256="$(sha256sum "${repo_root}/scripts/bench-envelope.sh" | awk '{print $1}')"
  workload_sha256="$(sha256sum "${repo_root}/scripts/bench-daemon-idle.sh" | awk '{print $1}')"

  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    for name in GITHUB_REPOSITORY GITHUB_WORKFLOW_REF GITHUB_WORKFLOW_SHA GITHUB_EVENT_NAME GITHUB_REF GITHUB_RUN_ID GITHUB_RUN_ATTEMPT GITHUB_JOB; do
      if [[ -z "${!name:-}" ]]; then
        echo "GitHub Actions receipt omits $name" >&2
        exit 1
      fi
    done
    ci_provider="github_actions"
    ci_repository="$GITHUB_REPOSITORY"
    ci_workflow_ref="$GITHUB_WORKFLOW_REF"
    ci_workflow_sha="$GITHUB_WORKFLOW_SHA"
    ci_event="$GITHUB_EVENT_NAME"
    ci_ref="$GITHUB_REF"
    ci_run="$GITHUB_RUN_ID"
    ci_attempt="$GITHUB_RUN_ATTEMPT"
    ci_job="$GITHUB_JOB"
  else
    ci_provider="local"
    ci_repository="local-unpublished"
    ci_workflow_ref="local-unpublished"
    ci_workflow_sha="$git_commit"
    ci_event="local"
    ci_ref="local"
    ci_run="local-unpublished"
    ci_attempt="1"
    ci_job="local-envelope"
  fi

  receipt_parent="$(dirname "$receipt")"
  if [[ -e "$receipt_parent" ]]; then
    echo "Receipt package already exists: $receipt_parent" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$receipt_parent")"
  mkdir "$receipt_parent"

  python3 - "$receipt" "$artifact_absolute" "$artifact_relative" "$artifact_sha256_before" "$artifact_size" \
    "$build_profile" "$git_commit" "$git_tree" "$dirty" "$ci_provider" "$ci_repository" \
    "$ci_workflow_ref" "$ci_workflow_sha" "$ci_event" "$ci_ref" "$ci_run" "$ci_attempt" "$ci_job" \
    "$architecture" "$(uname -r)" "$memory_max" "$memory_swap_max" "$cpu_quota" "$cpu_period" \
    "$peak_bytes" "$memory_swap_peak" "$oom_count" "$target" "$budget_bytes" \
    "$profile" "$steady_memory_peak_bytes" "$warmup_seconds" "$measurement_seconds" "$sample_interval_ms" "$max_sample_lateness_ns" \
    "$actual_warmup_ns" "$actual_measurement_ns" "$cpu_average_basis_points" "$cpu_p95_basis_points" \
    "$samples_file" "$artifact_budget_receipt_absolute" "$budgets_sha256" "$harness_sha256" "$workload_sha256" "$@" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys

(
    receipt_path, artifact_source, artifact_source_path, artifact_sha256,
    artifact_size, build_profile, git_commit, git_tree, dirty, provider,
    repository, workflow_ref, workflow_sha, event_name, ref, run_id,
    run_attempt, job, architecture, kernel_release, memory_max,
    memory_swap_max, cpu_quota, cpu_period, peak_memory, memory_swap_peak,
    oom_count, target, budget, profile, steady_memory_peak, warmup_seconds,
    measurement_seconds, sample_interval_ms, max_sample_lateness_ns, actual_warmup_ns,
    actual_measurement_ns, cpu_average_basis_points, cpu_p95_basis_points,
    samples_path, artifact_budget_source, budgets_sha256, harness_sha256,
    workload_sha256, *command,
) = sys.argv[1:]

observations = []
if profile == "canonical-idle":
    observations = json.loads(Path(samples_path).read_text())["observations"]

artifact_budget_binding = None
if artifact_budget_source:
    source_receipt = Path(artifact_budget_source)
    source_root = source_receipt.parent.resolve()
    artifact_budget = json.loads(source_receipt.read_text())
    destination_root = Path(receipt_path).parent / "artifact-budgets"
    destination_root.mkdir(mode=0o700)

    def copy_bound(relative_name, destination_name=None):
        relative = Path(relative_name)
        if relative.is_absolute() or ".." in relative.parts:
            raise SystemExit(f"unsafe artifact budget path: {relative_name}")
        source = source_root / relative
        current = source_root
        for part in relative.parts:
            current /= part
            if current.is_symlink():
                raise SystemExit(f"symlinked artifact budget path: {relative_name}")
        if not source.is_file():
            raise SystemExit(f"missing artifact budget file: {relative_name}")
        destination = destination_root / (destination_name or relative)
        destination.parent.mkdir(parents=True, exist_ok=True)
        with source.open("rb") as input_stream, destination.open("xb") as output_stream:
            os.fchmod(output_stream.fileno(), 0o600)
            shutil.copyfileobj(input_stream, output_stream)
        return destination

    for reference in artifact_budget["retained_artifacts"].values():
        copied = copy_bound(reference["path"])
        payload = copied.read_bytes()
        if len(payload) != reference["size_bytes"] or hashlib.sha256(payload).hexdigest() != reference["sha256"]:
            raise SystemExit(f"artifact budget retained bytes do not match {reference['path']}")
    copied_receipt = copy_bound(source_receipt.name, "evidence.json")
    artifact_budget_binding = {
        "path": "artifact-budgets/evidence.json",
        "sha256": hashlib.sha256(copied_receipt.read_bytes()).hexdigest(),
    }

package = Path(receipt_path).parent
artifact_name = f"sha256-{artifact_sha256}-fastid"
artifact_path = package / "artifacts" / artifact_name
artifact_path.parent.mkdir(mode=0o700)
with open(artifact_source, "rb") as source, open(artifact_path, "xb") as output:
    os.fchmod(output.fileno(), 0o600)
    shutil.copyfileobj(source, output)
    output.flush()
    os.fsync(output.fileno())

receipt = {
    "schema_version": "fasti.b1.performance-envelope.v1",
    "kind": "fasti.b1.performance-envelope",
    "status": "pass",
    "source": {
        "git_commit": git_commit,
        "git_tree": git_tree,
        "dirty": dirty == "true",
    },
    "ci": {
        "provider": provider,
        "repository": repository,
        "workflow_ref": workflow_ref,
        "workflow_sha": workflow_sha,
        "event": event_name,
        "ref": ref,
        "run": run_id,
        "run_attempt": run_attempt,
        "job": job,
    },
    "runner": {
        "architecture": architecture,
        "kernel_release": kernel_release,
        "cgroup_version": "v2",
    },
    "envelope": {
        "memory_max_bytes": int(memory_max),
        "memory_swap_max_bytes": int(memory_swap_max),
        "cpu_quota_micros": int(cpu_quota),
        "cpu_period_micros": int(cpu_period),
        "memory_swap_peak_bytes": int(memory_swap_peak),
        "oom_event_count": int(oom_count),
    },
    "measurement": {
        "profile": "canonical_idle_v1" if profile == "canonical-idle" else "startup_smoke_v1",
        "target": target,
        "budget_bytes": int(budget),
        "peak_memory_bytes": int(peak_memory),
        "steady_memory_peak_bytes": int(steady_memory_peak),
        "warmup_seconds": int(warmup_seconds),
        "measurement_seconds": int(measurement_seconds),
        "sample_interval_ms": int(sample_interval_ms),
        "max_sample_lateness_ns": int(max_sample_lateness_ns),
        "actual_warmup_ns": int(actual_warmup_ns),
        "actual_measurement_ns": int(actual_measurement_ns),
        "cpu_average_basis_points": int(cpu_average_basis_points),
        "cpu_p95_basis_points": int(cpu_p95_basis_points),
        "observations": observations,
        "network_isolation": "route_less_user_network_namespace" if profile == "canonical-idle" else "not_applied",
        "command_exit_code": 0,
        "command": command,
    },
    "policy": {
        "budgets_sha256": budgets_sha256,
        "harness_sha256": harness_sha256,
        "workload_sha256": workload_sha256,
    },
    "artifact": {
        "source_path": artifact_source_path,
        "path": f"artifacts/{artifact_name}",
        "sha256": artifact_sha256,
        "size_bytes": int(artifact_size),
        "build_profile": build_profile,
    },
    "artifact_budget_receipt": artifact_budget_binding,
}
with open(receipt_path, "x", encoding="utf-8") as output:
    json.dump(receipt, output, indent=2, sort_keys=True)
    output.write("\n")
    output.flush()
    os.fsync(output.fileno())
PY
fi

echo "envelope ${target}/${profile}: peak ${peak_mib} MiB, steady ${steady_memory_peak_bytes} bytes, CPU avg/p95 ${cpu_average_basis_points}/${cpu_p95_basis_points} bp, enforced ceiling ${ceiling_mib} MiB, command exit ${status}"
exit 0
