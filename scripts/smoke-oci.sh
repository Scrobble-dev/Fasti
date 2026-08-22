#!/usr/bin/env bash
set -euo pipefail

image="${1:-fasti:b0}"
idle_limit_mib="${FASTI_IDLE_MEMORY_LIMIT_MIB:-64}"

if [[ "$(docker inspect --format '{{.Config.User}}' "$image")" != "fasti:fasti" ]]; then
  echo "OCI image must run as fasti:fasti" >&2
  exit 1
fi

container_id="$(docker run --detach --rm --publish 127.0.0.1::8420 "$image")"
isolated_id=""
cleanup() {
  docker rm --force "$container_id" >/dev/null 2>&1 || true
  if [[ -n "$isolated_id" ]]; then
    docker rm --force "$isolated_id" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT
host_port="$(docker port "$container_id" 8420/tcp | head -1 | awk -F: '{print $NF}')"

for attempt in $(seq 1 30); do
  if health_body="$(curl --fail --silent "http://127.0.0.1:${host_port}/api/v1/health")"; then
    break
  fi

  if [[ "$attempt" -eq 30 ]]; then
    docker logs "$container_id"
    echo "Daemon did not become healthy" >&2
    exit 1
  fi

  sleep 1
done

python3 - "$health_body" <<'PY'
import json
import sys

payload = json.loads(sys.argv[1])
if payload.get("status") != "healthy" or not payload.get("version"):
    raise SystemExit(f"Unexpected health payload: {payload!r}")
PY

post_status="$(
  curl --silent --output /dev/null --write-out '%{http_code}' \
    --request POST \
    --header 'content-type: application/json' \
    --data '{}' \
    "http://127.0.0.1:${host_port}/api/v1/events"
)"

if [[ "$post_status" != "404" ]]; then
  echo "Unsupported event submission returned HTTP $post_status instead of 404" >&2
  exit 1
fi

cli_stdout="$(mktemp)"
cli_stderr="$(mktemp)"
trap 'rm -f "$cli_stdout" "$cli_stderr"; docker rm --force "$container_id" >/dev/null 2>&1 || true' EXIT

if docker run --rm --network none "$image" /usr/local/bin/fasti verify >"$cli_stdout" 2>"$cli_stderr"; then
  echo "Unavailable verify command reported success" >&2
  exit 1
fi

isolated_id="$(docker run --detach --rm --network none "$image")"
for attempt in $(seq 1 30); do
  if isolated_health="$(docker exec "$isolated_id" wget -q -O - http://127.0.0.1:8420/api/v1/health)"; then
    break
  fi

  if [[ "$attempt" -eq 30 ]]; then
    docker logs "$isolated_id"
    echo "Daemon did not become healthy with external networking disabled" >&2
    exit 1
  fi

  sleep 1
done

python3 - "$isolated_health" <<'PY'
import json
import sys

payload = json.loads(sys.argv[1])
if payload.get("status") != "healthy" or not payload.get("version"):
    raise SystemExit(f"Unexpected network-denied health payload: {payload!r}")
PY

if [[ -s "$cli_stdout" ]] || ! grep -Fq "not available in B0" "$cli_stderr"; then
  echo "Unavailable verify command did not fail explicitly and quietly" >&2
  exit 1
fi

memory_sample="$(docker stats --no-stream --format '{{.MemUsage}}' "$container_id" | awk '{print $1}')"
memory_bytes="$(python3 - "$memory_sample" <<'PY'
import re
import sys

match = re.fullmatch(r"([0-9]+(?:\.[0-9]+)?)(B|KiB|MiB|GiB)", sys.argv[1])
if match is None:
    raise SystemExit(f"Unrecognized Docker memory value: {sys.argv[1]!r}")

scale = {"B": 1, "KiB": 1024, "MiB": 1024**2, "GiB": 1024**3}
print(round(float(match.group(1)) * scale[match.group(2)]))
PY
)"
memory_limit_bytes="$((idle_limit_mib * 1024 * 1024))"

if ((memory_bytes > memory_limit_bytes)); then
  echo "Idle OCI memory $memory_sample exceeds ${idle_limit_mib} MiB" >&2
  exit 1
fi

image_size_bytes="$(docker image inspect "$image" --format '{{.Size}}')"
printf 'PASS: image=%s user=fasti:fasti health=healthy network_denied=pass post_events=404 cli_verify=nonzero idle_memory=%s idle_limit=%sMiB image_size_bytes=%s\n' \
  "$image" "$memory_sample" "$idle_limit_mib" "$image_size_bytes"
