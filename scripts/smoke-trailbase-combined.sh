#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
trailbase_root="$repo_root/.dev-trailbase"
fastid="$repo_root/target/debug/fastid"
temporary_root="$(mktemp -d -p "$repo_root/target" fasti-access-b-combined.XXXXXX)"
trailbase_pid=""
fastid_pid=""

cleanup() {
  [[ -z "$fastid_pid" ]] || kill "$fastid_pid" 2>/dev/null || true
  [[ -z "$trailbase_pid" ]] || kill "$trailbase_pid" 2>/dev/null || true
  [[ -z "$fastid_pid" ]] || wait "$fastid_pid" 2>/dev/null || true
  [[ -z "$trailbase_pid" ]] || wait "$trailbase_pid" 2>/dev/null || true
  rm -rf -- "$temporary_root"
}
trap cleanup EXIT INT TERM

[[ -x "$fastid" ]] || { echo "Build fastid before the combined resource probe." >&2; exit 1; }
python3 -B "$repo_root/scripts/trailbase_runtime.py" verify-root "$trailbase_root" >/dev/null
mkdir -m 700 "$temporary_root/fasti-data"

python3 -B "$repo_root/scripts/trailbase_runtime.py" run-native "$trailbase_root" \
  --public-url http://127.0.0.1:28400 \
  --address 127.0.0.1:28400 \
  --admin-address 127.0.0.1:28401 \
  --cors-origin http://127.0.0.1:28400 \
  >"$temporary_root/trailbase.log" 2>&1 &
trailbase_pid=$!

FASTI_DATA_ROOT="$temporary_root/fasti-data" \
FASTI_LISTEN=127.0.0.1:28420 \
  "$fastid" >"$temporary_root/fastid.log" 2>&1 &
fastid_pid=$!

for _ in {1..100}; do
  kill -0 "$trailbase_pid" 2>/dev/null || { echo "TrailBase exited during the combined resource probe." >&2; exit 1; }
  kill -0 "$fastid_pid" 2>/dev/null || { echo "fastid exited during the combined resource probe." >&2; exit 1; }
  if [[ "$(curl --silent --max-time 1 http://127.0.0.1:28400/api/healthcheck 2>/dev/null || true)" == Ok ]] &&
    curl --silent --fail --max-time 1 http://127.0.0.1:28420/api/v1/health >/dev/null 2>&1; then
    sleep 2
    echo "PASS: combined Fasti and TrailBase health under the governed resource envelope"
    exit 0
  fi
  sleep 0.1
done

echo "Combined Fasti and TrailBase health timed out." >&2
exit 1
