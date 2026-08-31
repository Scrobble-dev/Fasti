#!/usr/bin/env bash
set -euo pipefail

image="${1:-fasti:b0}"
expected_architecture="${2:-}"
oci_runtime="${3:-docker}"
idle_limit_mib="${FASTI_IDLE_MEMORY_LIMIT_MIB:-64}"

if [[ "$oci_runtime" != "docker" && "$oci_runtime" != "podman" ]]; then
  echo "OCI runtime must be docker or podman" >&2
  exit 1
fi

if [[ -n "$expected_architecture" ]] && \
  [[ "$("$oci_runtime" image inspect "$image" --format '{{.Architecture}}')" != "$expected_architecture" ]]; then
  echo "OCI image architecture does not match required $expected_architecture" >&2
  exit 1
fi

if [[ "$("$oci_runtime" inspect --format '{{.Config.User}}' "$image")" != "fasti:fasti" ]]; then
  echo "OCI image must run as fasti:fasti" >&2
  exit 1
fi

container_id="$("$oci_runtime" run --detach --rm --publish 127.0.0.1::8420 "$image")"
isolated_id=""
durable_id=""
durable_volume=""
cli_stdout=""
cli_stderr=""
# cleanup removes temporary CLI output files and force-removes the test containers.
cleanup() {
  if [[ -n "$cli_stdout" ]]; then
    rm -f "$cli_stdout"
  fi
  if [[ -n "$cli_stderr" ]]; then
    rm -f "$cli_stderr"
  fi
  "$oci_runtime" rm --force "$container_id" >/dev/null 2>&1 || true
  if [[ -n "$isolated_id" ]]; then
    "$oci_runtime" rm --force "$isolated_id" >/dev/null 2>&1 || true
  fi
  if [[ -n "$durable_id" ]]; then
    "$oci_runtime" rm --force "$durable_id" >/dev/null 2>&1 || true
  fi
  if [[ -n "$durable_volume" ]]; then
    "$oci_runtime" volume rm --force "$durable_volume" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT
host_port="$("$oci_runtime" port "$container_id" 8420/tcp | head -1 | awk -F: '{print $NF}')"

for attempt in $(seq 1 30); do
  if health_body="$(curl --fail --silent --connect-timeout 5 --max-time 10 "http://127.0.0.1:${host_port}/api/v1/health")"; then
    break
  fi

  if [[ "$attempt" -eq 30 ]]; then
    "$oci_runtime" logs "$container_id"
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
    --connect-timeout 5 \
    --max-time 10 \
    --request POST \
    --header 'content-type: application/json' \
    --data '{}' \
    "http://127.0.0.1:${host_port}/api/v1/events"
)"

if [[ "$post_status" != "404" ]]; then
  echo "Unsupported event submission returned HTTP $post_status instead of 404" >&2
  exit 1
fi

bootstrap_status="$(
  curl --silent --output /dev/null --write-out '%{http_code}' \
    --connect-timeout 5 \
    --max-time 10 \
    --request POST \
    --header 'content-type: application/json' \
    --data '{}' \
    "http://127.0.0.1:${host_port}/api/v1/node/initialization"
)"

if [[ "$bootstrap_status" != "404" ]]; then
  echo "Remote bootstrap returned HTTP $bootstrap_status instead of 404" >&2
  exit 1
fi

durable_volume="fasti-smoke-data-$$"
"$oci_runtime" volume create "$durable_volume" >/dev/null
"$oci_runtime" run --rm --user 0:0 --volume "$durable_volume:/data" \
  "$image" chown fasti:fasti /data
durable_id="$(
  "$oci_runtime" run --detach --rm --publish 127.0.0.1::8420 \
    --volume "$durable_volume:/data" \
    --env FASTI_DATA_ROOT=/data \
    --env FASTI_EXTERNAL_BIND_IP=127.0.0.1 \
    "$image"
)"
durable_port="$("$oci_runtime" port "$durable_id" 8420/tcp | head -1 | awk -F: '{print $NF}')"

for attempt in $(seq 1 30); do
  durable_bootstrap_status="$(
    curl --silent --output /dev/null --write-out '%{http_code}' \
      --connect-timeout 5 \
      --max-time 10 \
      --request POST \
      --header 'content-type: application/json' \
      --data '{}' \
      "http://127.0.0.1:${durable_port}/api/v1/node/initialization"
  )" || true
  if [[ "$durable_bootstrap_status" == "403" ]]; then
    break
  fi

  if [[ "$attempt" -eq 30 ]]; then
    "$oci_runtime" logs "$durable_id"
    echo "Trusted loopback port forward did not mount the durable API" >&2
    exit 1
  fi

  sleep 1
done

cli_stdout="$(mktemp)"
cli_stderr="$(mktemp)"

if "$oci_runtime" run --rm --network none "$image" /usr/local/bin/fasti verify >"$cli_stdout" 2>"$cli_stderr"; then
  echo "Unavailable verify command reported success" >&2
  exit 1
fi
if [[ -s "$cli_stdout" ]] || \
  ! grep -Fq "capability_id=portability.workspace.verify" "$cli_stderr" || \
  ! grep -Fq "not available in the current runtime" "$cli_stderr" || \
  ! grep -Fq "owned by B3" "$cli_stderr"; then
  echo "Unavailable verify command did not fail explicitly and quietly" >&2
  exit 1
fi

if ! "$oci_runtime" run --rm --network none "$image" /usr/local/bin/fasti \
  access bootstrap-administrator --help >"$cli_stdout" 2>"$cli_stderr"; then
  echo "First-administrator CLI help is unavailable" >&2
  exit 1
fi
if grep -Eiq 'password|token|bootstrap.secret|browser.binding' "$cli_stdout"; then
  echo "First-administrator CLI help exposes a forbidden credential argument" >&2
  exit 1
fi
if "$oci_runtime" run --rm --network none "$image" /usr/local/bin/fasti \
  access bootstrap-administrator \
  --data-root /missing-fasti-data-root \
  --trailbase-root /missing-trailbase-root \
  --password must-not-echo >"$cli_stdout" 2>"$cli_stderr"; then
  echo "First-administrator CLI accepted a password argument" >&2
  exit 1
fi
if grep -Fq 'must-not-echo' "$cli_stdout" "$cli_stderr"; then
  echo "First-administrator CLI repeated a rejected credential value" >&2
  exit 1
fi

isolated_id="$("$oci_runtime" run --detach --rm --network none "$image")"
for attempt in $(seq 1 30); do
  if isolated_health="$("$oci_runtime" exec "$isolated_id" wget -q -O - http://127.0.0.1:8420/api/v1/health)"; then
    break
  fi

  if [[ "$attempt" -eq 30 ]]; then
    "$oci_runtime" logs "$isolated_id"
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

memory_field='{{.MemUsage}}'
if [[ "$oci_runtime" == "podman" ]]; then
  memory_field='{{.MemUsageBytes}}'
fi
memory_sample="$("$oci_runtime" stats --no-stream --format "$memory_field" "$container_id" | awk '{print $1}')"
memory_bytes="$(python3 - "$memory_sample" <<'PY'
import re
import sys

match = re.fullmatch(r"([0-9]+(?:\.[0-9]+)?)(B|KiB|MiB|GiB)", sys.argv[1])
if match is None:
    raise SystemExit(f"Unrecognized OCI runtime memory value: {sys.argv[1]!r}")

scale = {"B": 1, "KiB": 1024, "MiB": 1024**2, "GiB": 1024**3}
print(round(float(match.group(1)) * scale[match.group(2)]))
PY
)"
memory_limit_bytes="$((idle_limit_mib * 1024 * 1024))"

if ((memory_bytes > memory_limit_bytes)); then
  echo "Idle OCI memory $memory_sample exceeds ${idle_limit_mib} MiB" >&2
  exit 1
fi

image_size_bytes="$("$oci_runtime" image inspect "$image" --format '{{.Size}}')"
printf 'PASS: runtime=%s image=%s user=fasti:fasti health=healthy network_denied=pass post_events=404 post_initialization=404 durable_loopback_initialization=403 cli_verify=nonzero idle_memory=%s idle_limit=%sMiB image_size_bytes=%s\n' \
  "$oci_runtime" "$image" "$memory_sample" "$idle_limit_mib" "$image_size_bytes"
