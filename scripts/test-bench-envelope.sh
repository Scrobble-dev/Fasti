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

# The kernel must kill a workload that exceeds the enforced ceiling. This is the
# property the whole gate rests on. Allocating well past the ceiling proves the
# limit is real rather than advisory.
check "ceiling breach is killed by the kernel" 1 \
  bash "$envelope" --target heavy -- python3 -c \
  "b=bytearray(400*1024*1024); b[::4096]=b'x'*(len(b)//4096)"

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

rm -f /tmp/bench-envelope-selftest.out /tmp/bench-envelope-selftest.err

if (( failures > 0 )); then
  echo "bench-envelope self-test: ${failures} check(s) failed; measurements from this envelope cannot be trusted" >&2
  exit 1
fi

echo "bench-envelope self-test: envelope enforces its limits"
