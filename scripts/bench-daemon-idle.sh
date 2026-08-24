#!/usr/bin/env bash
# Start fastid, wait until it serves health, let it settle, then exit.
#
# Intended to be run INSIDE scripts/bench-envelope.sh, which measures the peak
# memory of the whole scope. This script deliberately does no measuring of its
# own: the envelope owns that, reading the kernel watermark, so there is one
# way to obtain a memory number rather than two that can disagree.
set -euo pipefail

daemon="${1:-target/release/fastid}"
settle_seconds="${FASTI_IDLE_SETTLE_SECONDS:-3}"

if [[ ! -x "$daemon" ]]; then
  echo "Missing daemon binary $daemon; build with: cargo build --locked --release --bin fastid" >&2
  exit 1
fi

work_dir="$(mktemp -d)"
daemon_pid=""
cleanup() {
  if [[ -n "$daemon_pid" ]]; then
    kill "$daemon_pid" 2>/dev/null || true
    wait "$daemon_pid" 2>/dev/null || true
  fi
  rm -rf "$work_dir"
}
trap cleanup EXIT

export FASTI_LISTEN=127.0.0.1:8421
export FASTI_DATA_ROOT="${work_dir}/data"

"$daemon" >"${work_dir}/daemon.log" 2>&1 &
daemon_pid=$!

healthy=0
for _ in $(seq 1 30); do
  if timeout 2 bash -c '
      exec 3<>/dev/tcp/127.0.0.1/8421 || exit 1
      printf "GET /api/v1/health HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n" >&3
      grep -q "\"status\":\"healthy\"" <&3
    ' 2>/dev/null; then
    healthy=1
    break
  fi
  sleep 1
done

if (( healthy == 0 )); then
  cat "${work_dir}/daemon.log" >&2
  echo "Daemon did not become healthy inside the envelope" >&2
  exit 1
fi

# Let allocations settle so the peak reflects a resting daemon rather than
# startup churn.
sleep "$settle_seconds"

if ! kill -0 "$daemon_pid" 2>/dev/null; then
  cat "${work_dir}/daemon.log" >&2
  echo "Daemon exited while settling; the envelope may have killed it" >&2
  exit 1
fi

echo "daemon served health and settled for ${settle_seconds}s"
