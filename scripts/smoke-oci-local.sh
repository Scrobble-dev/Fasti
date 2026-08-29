#!/usr/bin/env bash
set -euo pipefail

# Smoke-tests the `local` Dockerfile target (fastid + bundled web UI, one
# origin, no reverse proxy). Complements scripts/smoke-oci.sh, which covers
# the `runtime` target's contract (non-root, health-only-by-default,
# false-success, idle memory) -- that contract is untouched by `local` since
# `local` is `FROM runtime`, so this script only checks what `local` adds:
# the UI is actually served, and it doesn't change the durable-route
# security state machine.

image="${1:-fasti:local}"
oci_runtime="${2:-docker}"

if [[ "$oci_runtime" != "docker" && "$oci_runtime" != "podman" ]]; then
  echo "OCI runtime must be docker or podman" >&2
  exit 1
fi

bare_id=""
durable_id=""
durable_volume=""
cleanup() {
  [[ -n "$bare_id" ]] && "$oci_runtime" rm --force "$bare_id" >/dev/null 2>&1 || true
  [[ -n "$durable_id" ]] && "$oci_runtime" rm --force "$durable_id" >/dev/null 2>&1 || true
  [[ -n "$durable_volume" ]] && "$oci_runtime" volume rm --force "$durable_volume" >/dev/null 2>&1 || true
}
trap cleanup EXIT

wait_for_health() {
  local port="$1"
  local body
  for attempt in $(seq 1 30); do
    if body="$(curl --fail --silent --connect-timeout 5 --max-time 10 "http://127.0.0.1:${port}/api/v1/health")"; then
      printf '%s' "$body"
      return 0
    fi
    if [[ "$attempt" -eq 30 ]]; then
      return 1
    fi
    sleep 1
  done
}

# --- 1. Bare run: no FASTI_DATA_ROOT set. The UI must still be served (it
#     doesn't depend on the durable data root), and the durable API must
#     stay absent -- bundling the UI must not change that security default.
bare_id="$("$oci_runtime" run --detach --rm --publish 127.0.0.1::8420 "$image")"
bare_port="$("$oci_runtime" port "$bare_id" 8420/tcp | head -1 | awk -F: '{print $NF}')"

if ! wait_for_health "$bare_port" >/dev/null; then
  "$oci_runtime" logs "$bare_id"
  echo "Bare local image did not become healthy" >&2
  exit 1
fi

bare_ui_status="$(curl --silent --connect-timeout 5 --max-time 10 --output /dev/null --write-out '%{http_code}' "http://127.0.0.1:${bare_port}/")"
if [[ "$bare_ui_status" != "200" ]]; then
  echo "Bare local image did not serve the web UI at / (got $bare_ui_status)" >&2
  exit 1
fi

bare_init_status="$(
  curl --silent --connect-timeout 5 --max-time 10 --output /dev/null --write-out '%{http_code}' \
    --request POST --header 'content-type: application/json' --data '{}' \
    "http://127.0.0.1:${bare_port}/api/v1/node/initialization"
)"
if [[ "$bare_init_status" == "403" ]]; then
  echo "Bundling the UI must not mount durable routes by default (got 403 with no FASTI_DATA_ROOT)" >&2
  exit 1
fi

# --- 2. Fully configured run (the documented recipe: data volume +
#     FASTI_DATA_ROOT + FASTI_EXTERNAL_BIND_IP): UI and durable API both work
#     together, on the same origin.
durable_volume="fasti-smoke-local-data-$$"
"$oci_runtime" volume create "$durable_volume" >/dev/null
"$oci_runtime" run --rm --user 0:0 --volume "$durable_volume:/data" "$image" chown fasti:fasti /data
durable_id="$(
  "$oci_runtime" run --detach --rm --publish 127.0.0.1::8420 \
    --volume "$durable_volume:/data" \
    --env FASTI_DATA_ROOT=/data \
    --env FASTI_EXTERNAL_BIND_IP=127.0.0.1 \
    "$image"
)"
durable_port="$("$oci_runtime" port "$durable_id" 8420/tcp | head -1 | awk -F: '{print $NF}')"

if ! wait_for_health "$durable_port" >/dev/null; then
  "$oci_runtime" logs "$durable_id"
  echo "Configured local image did not become healthy" >&2
  exit 1
fi

durable_ui_status="$(curl --silent --connect-timeout 5 --max-time 10 --output /dev/null --write-out '%{http_code}' "http://127.0.0.1:${durable_port}/")"
if [[ "$durable_ui_status" != "200" ]]; then
  echo "Configured local image did not serve the web UI at / (got $durable_ui_status)" >&2
  exit 1
fi

spa_route_status="$(curl --silent --connect-timeout 5 --max-time 10 --output /dev/null --write-out '%{http_code}' "http://127.0.0.1:${durable_port}/status")"
if [[ "$spa_route_status" != "200" ]]; then
  echo "Client-side route /status did not fall back to index.html (got $spa_route_status)" >&2
  exit 1
fi

durable_init_status="$(
  curl --silent --connect-timeout 5 --max-time 10 --output /dev/null --write-out '%{http_code}' \
    --request POST --header 'content-type: application/json' --data '{}' \
    "http://127.0.0.1:${durable_port}/api/v1/node/initialization"
)"
if [[ "$durable_init_status" != "403" ]]; then
  echo "Durable routes did not mount with FASTI_DATA_ROOT + FASTI_EXTERNAL_BIND_IP set (got $durable_init_status instead of 403)" >&2
  exit 1
fi

api_still_wins_status="$(curl --silent --connect-timeout 5 --max-time 10 --output /dev/null --write-out '%{http_code}' "http://127.0.0.1:${durable_port}/api/v1/health")"
if [[ "$api_still_wins_status" != "200" ]]; then
  echo "The real API route did not take precedence over the static fallback (got $api_still_wins_status)" >&2
  exit 1
fi

printf 'PASS: runtime=%s image=%s bare_ui=200 bare_init=%s configured_ui=200 configured_spa_route=200 configured_init=403 api_precedence=200\n' \
  "$oci_runtime" "$image" "$bare_init_status"
