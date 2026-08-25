#!/usr/bin/env bash
# Self-test for scripts/bench-envelope.sh.
#
# A memory gate that cannot fail is worse than no gate: it reports success
# forever and nobody looks again. This proves the envelope actually enforces
# before any measurement taken with it is trusted. It runs first in CI for that
# reason.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
envelope="${here}/bench-envelope.sh"
failures=0
receipt_root="$(mktemp -d)"
receipt_path="${receipt_root}/package/receipt.json"
cleanup() {
  rm -rf "$receipt_root"
  rm -f /tmp/bench-envelope-selftest.out /tmp/bench-envelope-selftest.err
}
trap cleanup EXIT

check() {
  local name="$1" expected="$2"; shift 2
  local status=0
  "$@" >/tmp/bench-envelope-selftest.out 2>/tmp/bench-envelope-selftest.err || status=$?
  if [[ "$status" -eq "$expected" ]]; then
    printf '  ok    %-46s exit %s\n' "$name" "$status"
  else
    printf '  FAIL  %-46s exit %s, wanted %s\n' "$name" "$status" "$expected"
    sed 's/^/        /' /tmp/bench-envelope-selftest.err | tail -3
    failures=$((failures + 1))
  fi
}

echo "bench-envelope self-test"

# A trivial command must pass the idle budget. If this fails the envelope is not
# usable on this host at all.
check "trivial command within idle budget" 0 \
  bash "$envelope" --target idle -- true

# A passing receipt must record the controls the kernel actually applied and
# retain the measured artifact. This local fixture checks shape only; the Rust
# milestone verifier separately rejects non-GitHub and non-dev receipts.
check "passing run emits applied-limit receipt" 0 \
  bash "$envelope" --target idle \
    --receipt "$receipt_path" \
    --artifact "$here/bench-daemon-idle.sh" \
    --build-profile release \
    -- true
check "receipt binds controls and retained artifact" 0 \
  python3 - "$receipt_path" <<'PY'
import json
from pathlib import Path
import sys

path = Path(sys.argv[1])
receipt = json.loads(path.read_text())
assert receipt["envelope"]["memory_max_bytes"] == 201326592
assert receipt["envelope"]["memory_swap_max_bytes"] == 0
assert receipt["envelope"]["memory_swap_peak_bytes"] == 0
assert receipt["envelope"]["oom_event_count"] == 0
assert receipt["envelope"]["cpu_quota_micros"] == receipt["envelope"]["cpu_period_micros"]
assert receipt["measurement"]["target"] == "idle"
assert receipt["measurement"]["profile"] == "startup_smoke_v1"
assert receipt["measurement"]["peak_memory_bytes"] <= receipt["measurement"]["budget_bytes"]
assert (path.parent / receipt["artifact"]["path"]).is_file()
PY

# Exercise the timed sampler without making the self-test wait for the governed
# 25-minute window. Test timings cannot produce a receipt.
check "canonical sampler enforces timed CPU and memory" 0 \
  env FASTI_ENVELOPE_TEST_WARMUP_SECONDS=1 FASTI_ENVELOPE_TEST_MEASUREMENT_SECONDS=5 \
  bash "$envelope" --target idle --profile canonical-idle -- sleep 8

# The kernel must kill a workload that exceeds the enforced ceiling. This is the
# property the whole gate rests on. Allocating well past the ceiling proves the
# limit is real rather than advisory.
check "ceiling breach is killed by the kernel" 1 \
  bash "$envelope" --target heavy -- python3 -c \
  "b=bytearray(400*1024*1024); b[::4096]=b'x'*(len(b)//4096)"

# A supervising process can mask a killed child's status. The cgroup OOM event
# must still fail the run.
check "masked child OOM still fails the envelope" 1 \
  bash "$envelope" --target heavy -- bash -c \
  'python3 -c "b=bytearray(400*1024*1024); b[::4096]=bytes(len(b)//4096)" || true'

# A workload that fits the ceiling but exceeds the smaller idle budget must fail
# the assertion rather than the kernel, so the two remain distinguishable.
check "budget overshoot fails the assertion" 1 \
  bash "$envelope" --target idle -- python3 -c \
  "b=bytearray(100*1024*1024); b[::4096]=b'x'*(len(b)//4096)"

# The same workload must pass against the larger budget. Without this control
# the previous case could be failing for an unrelated reason.
check "same workload passes the heavy budget" 0 \
  bash "$envelope" --target heavy -- python3 -c \
  "b=bytearray(100*1024*1024); b[::4096]=b'x'*(len(b)//4096)"

# A failing command must propagate, so a broken benchmark cannot look like a
# passing one.
check "command failure propagates" 1 \
  bash "$envelope" --target idle -- false

if (( failures > 0 )); then
  echo "bench-envelope self-test: ${failures} check(s) failed; measurements from this envelope cannot be trusted" >&2
  exit 1
fi

echo "bench-envelope self-test: envelope enforces its limits"
