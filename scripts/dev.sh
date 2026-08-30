#!/usr/bin/env bash
# Fasti Local Development Launcher
#
# Usage:
#   ./scripts/dev.sh             # Start the native daemon (+ web, if present)
#   ./scripts/dev.sh --podman    # Start Fasti in a scoped Podman container
#   ./scripts/dev.sh --docker    # Start Fasti in a scoped Docker container
#   ./scripts/dev.sh --desktop   # Start the trusted Desktop review host
#   ./scripts/dev.sh --status    # Check this worktree's daemon and API health
#   ./scripts/dev.sh --stop      # Stop this worktree's daemon or container
#   ./scripts/dev.sh --reset-access [--full-dev-root]
#                                # Reset the confirmed development root
#   ./scripts/dev.sh --open      # Open the web UI, or the API health check
#   ./scripts/dev.sh --self-test # Verify scoped process cleanup
#
# apps/web (the local Svelte Workbench) only exists on
# worktrees checked out at a branch that carries it -- not every worktree
# has it, so this is detected at runtime rather than assumed.
#
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOGDIR="$PROJECT_ROOT/.dev-logs"

_resolve_data_root() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "$PROJECT_ROOT" "$1" ;;
  esac
}

FASTI_DATA_ROOT_EXPLICIT=1
[[ -n "${FASTI_DATA_ROOT:-}" ]] || FASTI_DATA_ROOT_EXPLICIT=0
DATADIR="$(_resolve_data_root "${FASTI_DATA_ROOT:-.dev-data}")"
RUNDIR="$PROJECT_ROOT/.dev-run"
FASTI_PORT="${FASTI_PORT:-8420}"
FASTI_LISTEN="${FASTI_LISTEN:-127.0.0.1:$FASTI_PORT}"
# apps/web's vite.config.ts hardcodes this port with strictPort -- not
# configurable here without also changing that file.
WEB_PORT=5173
FASTI_IMAGE="${FASTI_IMAGE:-fasti:b0}"
FASTI_PORT_FALLBACK="${FASTI_PORT_FALLBACK:-fail}"
FASTI_CONTAINER_RUNTIME="${FASTI_CONTAINER_RUNTIME:-podman}"
FASTI_PUBLIC_URL="${FASTI_PUBLIC_URL:-}"
BOUND_ADDR_FILE="$RUNDIR/bound-addr"
FASTI_API_URL_EXPLICIT=1
if [[ -z "${FASTI_API_URL:-}" ]]; then
  FASTI_API_URL_EXPLICIT=0
  case "$FASTI_LISTEN" in
    0.0.0.0:*) FASTI_API_URL="http://127.0.0.1:${FASTI_LISTEN##*:}" ;;
    \[::\]:*) FASTI_API_URL="http://[::1]:${FASTI_LISTEN##*:}" ;;
    *) FASTI_API_URL="http://$FASTI_LISTEN" ;;
  esac
fi
FASTI_API_URL="${FASTI_API_URL%/}"
FASTI_PUBLIC_URL="${FASTI_PUBLIC_URL%/}"
DEV_SCOPE="${FASTI_DEV_SCOPE:-$(basename "$PROJECT_ROOT")}"
DEV_SCOPE="${DEV_SCOPE//[^A-Za-z0-9_.-]/-}"
CONTAINER_NAME="fasti-dev-$DEV_SCOPE"
TRAILBASE_ROOT="$PROJECT_ROOT/.dev-trailbase"
TRAILBASE_PUBLIC_ADDR="127.0.0.1:4000"
TRAILBASE_ADMIN_ADDR="127.0.0.1:4001"
TRAILBASE_PUBLIC_URL="http://$TRAILBASE_PUBLIC_ADDR"
TRAILBASE_CONTAINER_NAME="trailbase-dev-$DEV_SCOPE"

_validate_port() {
  if [[ "$2" =~ ^[0-9]+$ && ${#2} -le 5 ]] && ((10#$2 >= 1 && 10#$2 <= 65535)); then
    return 0
  fi
  echo "$1 must be an integer from 1 to 65535" >&2
  return 1
}

_validate_port FASTI_PORT "$FASTI_PORT"

case "$FASTI_PORT_FALLBACK" in
  auto|fail) ;;
  *) echo "FASTI_PORT_FALLBACK must be auto or fail" >&2; exit 1 ;;
esac

case "$FASTI_CONTAINER_RUNTIME" in
  podman|docker) ;;
  *) echo "FASTI_CONTAINER_RUNTIME must be podman or docker" >&2; exit 1 ;;
esac

_validate_origin_url() {
  local label="$1"
  local value="$2"
  local rest=""
  local lower=""
  local port=""
  case "$value" in
    http://*) rest="${value#http://}" ;;
    https://*) rest="${value#https://}" ;;
    *) echo "$label must use http or https" >&2; return 1 ;;
  esac
  if [[ -z "$rest" || "$rest" == */* || "$rest" == *"?"* || "$rest" == *"#"* || "$rest" == *"@"* || "$rest" == *[$'\t\r\n ']* ]]; then
    echo "$label must be an origin URL without credentials, path, query, or fragment" >&2
    return 1
  fi
  if [[ "$rest" == \[* ]]; then
    if [[ "$rest" =~ ^\[[^]]+\]$ ]]; then
      :
    elif [[ "$rest" =~ ^\[[^]]+\]:(.*)$ ]]; then
      port="${BASH_REMATCH[1]}"
    else
      echo "$label must contain a valid host and optional port" >&2
      return 1
    fi
  elif [[ "$rest" =~ ^[^:]+$ ]]; then
    :
  elif [[ "$rest" =~ ^[^:]+:(.*)$ ]]; then
    port="${BASH_REMATCH[1]}"
  else
    echo "$label must contain a valid host and optional port" >&2
    return 1
  fi
  if [[ -n "$port" ]]; then
    _validate_port "$label port" "$port" || return 1
  elif [[ "$rest" == *: ]]; then
    _validate_port "$label port" "$port" || return 1
  fi
  if [[ "$value" == http://* ]]; then
    lower="${rest,,}"
    case "$lower" in
      localhost|localhost:*|127.0.0.1|127.0.0.1:*|\[::1\]|\[::1\]:*) ;;
      *) echo "$label must use https for non-loopback hosts" >&2; return 1 ;;
    esac
  fi
}

_validate_origin_url FASTI_API_URL "$FASTI_API_URL"
[[ -z "$FASTI_PUBLIC_URL" ]] || _validate_origin_url FASTI_PUBLIC_URL "$FASTI_PUBLIC_URL"

_validate_open_target() {
  python3 - "$1" <<'PY'
import sys
from urllib.parse import urlsplit

value = sys.argv[1]
if any(character.isspace() or ord(character) < 32 for character in value):
    raise SystemExit("browser target must not contain whitespace or control characters")
parsed = urlsplit(value)
if parsed.scheme not in {"http", "https"} or not parsed.hostname:
    raise SystemExit("browser target must be an HTTP or HTTPS URL")
if parsed.username is not None or parsed.password is not None:
    raise SystemExit("browser target must not contain credentials")
try:
    parsed.port
except ValueError as error:
    raise SystemExit(f"invalid browser target port: {error}") from error
PY
}

_api_url_for_addr() {
  case "$1" in
    0.0.0.0:*) printf 'http://127.0.0.1:%s\n' "${1##*:}" ;;
    \[::\]:*) printf 'http://[::1]:%s\n' "${1##*:}" ;;
    *) printf 'http://%s\n' "$1" ;;
  esac
}

_canonical_socket_addr() {
  python3 - "$1" <<'PY'
import ipaddress
import sys

value = sys.argv[1]
if value.startswith("["):
    end = value.find("]")
    if end < 0 or value[end + 1 : end + 2] != ":":
        raise SystemExit("invalid IPv6 socket address")
    host = value[1:end]
    port = value[end + 2 :]
    address = f"[{ipaddress.ip_address(host).compressed}]"
else:
    host, port = value.rsplit(":", 1)
    address = str(ipaddress.ip_address(host))
port_number = int(port)
if not 1 <= port_number <= 65535:
    raise SystemExit("socket port must be between 1 and 65535")
print(f"{address}:{port_number}")
PY
}

_listener_fell_back() {
  local preferred=""
  preferred="$(_canonical_socket_addr "$FASTI_LISTEN")" || return 0
  [[ "$1" != "$preferred" ]]
}

_memory_ceiling_mib() {
  python3 - "$PROJECT_ROOT/benchmarks/b1/budgets.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    ceiling = json.load(source)["memory_bytes"]["absolute_ceiling"]
if not isinstance(ceiling, int) or ceiling <= 0 or ceiling % (1024 * 1024):
    raise SystemExit("absolute memory ceiling must be a positive whole MiB value")
print(ceiling // (1024 * 1024))
PY
}

NATIVE_SCOPE_RUNNER=()

_configure_native_scope() {
  local ceiling_mib=""
  local ceiling_bytes=""
  ceiling_mib="$(_memory_ceiling_mib)"
  ceiling_bytes=$((ceiling_mib * 1024 * 1024))
  local properties=(-p "MemoryMax=${ceiling_bytes}" -p "MemorySwapMax=0")
  if systemd-run --user --scope --quiet "${properties[@]}" -- python3 -c '
from pathlib import Path
import os, sys

entry = next(line for line in Path("/proc/self/cgroup").read_text().splitlines() if line.startswith("0::"))
cgroup_rel = entry[3:].lstrip("/")
root = Path("/sys/fs/cgroup") / cgroup_rel
if (root / "memory.max").exists():
    valid = (root / "memory.max").read_text().strip() == sys.argv[1]
    valid = valid and (root / "memory.swap.max").read_text().strip() == "0"
    raise SystemExit(0 if valid else 1)

if Path("/run/.containerenv").exists() or Path("/.dockerenv").exists() or "DISTROBOX_ENTER_PATH" in os.environ:
    raise SystemExit(0)

raise SystemExit(1)
  ' "$ceiling_bytes" 2>/dev/null; then
    NATIVE_SCOPE_RUNNER=(systemd-run --user --scope --quiet "${properties[@]}" --)
    return 0
  fi
  echo "Native mode requires a user cgroup v2 scope with a ${ceiling_mib} MiB memory ceiling and swap disabled. Start a user systemd session or use --podman." >&2
  return 1
}
_wait_for_health() {
  local pid="${1:-}"
  for _ in {1..10}; do
    [[ -z "$pid" ]] || kill -0 "$pid" 2>/dev/null || return 1
    curl --connect-timeout 2 --max-time 5 --silent --fail "$FASTI_API_URL/api/v1/health" >/dev/null 2>&1 && return 0
    sleep 0.5
  done
  return 1
}

_durable_api_is_mounted() {
  local status=""
  status="$(curl --connect-timeout 2 --max-time 5 --silent --output /dev/null \
    --write-out '%{http_code}' --request POST --header 'content-type: application/json' \
    --data '{}' "$FASTI_API_URL/api/v1/node/initialization")" || return 1
  [[ "$status" == 403 ]]
}

_read_bound_addr() {
  local pid="$1"
  local actual=""
  for _ in {1..20}; do
    kill -0 "$pid" 2>/dev/null || return 1
    if [[ -s "$BOUND_ADDR_FILE" ]]; then
      actual="$(<"$BOUND_ADDR_FILE")"
      [[ -n "$actual" ]] || return 1
      printf '%s\n' "$actual"
      return 0
    fi
    sleep 0.1
  done
  return 1
}

_write_bound_addr() {
  local temporary="$BOUND_ADDR_FILE.$$"
  mkdir -p "$RUNDIR"
  printf '%s\n' "$1" > "$temporary"
  mv "$temporary" "$BOUND_ADDR_FILE"
}

_process_identity() {
  local stat=""
  local fields=()
  [[ -r "/proc/$1/stat" ]] || return 1
  IFS= read -r stat < "/proc/$1/stat" || return 1
  stat="${stat##*) }"
  read -r -a fields <<< "$stat"
  [[ ${#fields[@]} -gt 19 ]] || return 1
  printf '%s\n' "${fields[19]}"
}

_write_pidfile() {
  local name="$1"
  local pid="$2"
  local started
  started="$(_process_identity "$pid")"
  [[ -n "$started" ]] || return 1
  mkdir -p "$RUNDIR"
  printf '%s|%s\n' "$pid" "$started" > "$RUNDIR/$name.pid"
}

_tracked_pid() {
  local name="$1"
  local pid=""
  local started=""
  local current=""
  [[ -f "$RUNDIR/$name.pid" ]] || return 1
  IFS='|' read -r pid started < "$RUNDIR/$name.pid"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  current="$(_process_identity "$pid")"
  [[ -n "$current" && "$current" == "$started" ]] || return 1
  printf '%s\n' "$pid"
}

_stop_pidfile() {
  local name="$1"
  local pid=""
  if pid="$(_tracked_pid "$name")"; then
    kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true
    for _ in {1..10}; do
      [[ "$(_tracked_pid "$name" 2>/dev/null || true)" == "$pid" ]] || break
      sleep 0.1
    done
    if [[ "$(_tracked_pid "$name" 2>/dev/null || true)" == "$pid" ]]; then
      kill -KILL -- "-$pid" 2>/dev/null || kill -KILL "$pid" 2>/dev/null || true
      for _ in {1..10}; do
        [[ "$(_tracked_pid "$name" 2>/dev/null || true)" == "$pid" ]] || break
        sleep 0.1
      done
    fi
  fi
  rm -f "$RUNDIR/$name.pid"
}

_has_web() {
  [[ -f "$PROJECT_ROOT/apps/web/package.json" ]]
}

_stop_processes() {
  _stop_pidfile daemon
  _stop_pidfile web
  _stop_pidfile reset-daemon
  _stop_pidfile trailbase
  rm -f "$BOUND_ADDR_FILE"
}

_trailbase_health() {
  [[ "$(curl --connect-timeout 2 --max-time 5 --silent --fail "$TRAILBASE_PUBLIC_URL/api/healthcheck" 2>/dev/null)" == Ok ]]
}

_trailbase_route_boundary() {
  local mode="${1:-native}"
  local main_admin=""
  local main_records=""
  local private_health=""
  main_admin="$(curl --connect-timeout 2 --max-time 5 --silent --output /dev/null --write-out '%{http_code}' "$TRAILBASE_PUBLIC_URL/api/_admin/info" || true)"
  main_records="$(curl --connect-timeout 2 --max-time 5 --silent --output /dev/null --write-out '%{http_code}' "$TRAILBASE_PUBLIC_URL/api/records/v1" || true)"
  [[ "$main_admin" == 404 && "$main_records" == 404 ]] || return 1
  if [[ "$mode" == native ]]; then
    private_health="$(curl --connect-timeout 2 --max-time 5 --silent --output /dev/null --write-out '%{http_code}' "http://$TRAILBASE_ADMIN_ADDR/api/healthcheck" || true)"
    [[ "$private_health" == 404 ]]
  fi
}

_trailbase_ports_are_free() {
  local port=""
  local status=0
  for port in 4000 4001; do
    _port_in_use "$port" || status=$?
    if ((status == 2)); then
      echo "Cannot verify TrailBase port $port availability (ss probe unavailable)" >&2
      return 1
    elif ((status == 0)); then
      echo "TrailBase port $port is already in use" >&2
      return 1
    fi
    status=0
  done
}

_trailbase_container_runtime() {
  local runtime=""
  for runtime in podman docker; do
    command -v "$runtime" >/dev/null 2>&1 || continue
    if [[ "$("$runtime" inspect "$TRAILBASE_CONTAINER_NAME" --format '{{.State.Running}}' 2>/dev/null || true)" == true ]]; then
      printf '%s\n' "$runtime"
      return 0
    fi
  done
  return 1
}

_trailbase_initialize() {
  if [[ -f "$TRAILBASE_ROOT/bootstrap.json" ]]; then
    echo "TrailBase is already initialized for this worktree." >&2
    return 1
  fi
  _trailbase_ports_are_free
  _configure_native_scope
  "${NATIVE_SCOPE_RUNNER[@]}" python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" \
    bootstrap-native "$TRAILBASE_ROOT" \
    --public-url "$TRAILBASE_PUBLIC_URL" \
    --address "$TRAILBASE_PUBLIC_ADDR" \
    --admin-address "$TRAILBASE_ADMIN_ADDR" \
    --cors-origin "$TRAILBASE_PUBLIC_URL"
}

_trailbase_start_native() {
  local pid=""
  local container_runtime=""
  if container_runtime="$(_trailbase_container_runtime 2>/dev/null)"; then
    echo "Stop the running TrailBase $container_runtime container before starting native mode." >&2
    return 1
  fi
  if pid="$(_tracked_pid trailbase 2>/dev/null)"; then
    echo "TrailBase is already running (PID: $pid)."
    return 0
  fi
  if [[ ! -f "$TRAILBASE_ROOT/bootstrap.json" ]]; then
    echo "TrailBase is not initialized. Run './scripts/dev.sh trailbase initialize' from the owning terminal." >&2
    return 1
  fi
  python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" verify-root "$TRAILBASE_ROOT" >/dev/null
  _trailbase_ports_are_free
  _configure_native_scope
  (umask 077; mkdir -p "$LOGDIR" "$RUNDIR")
  set -m
  (
    umask 077
    exec "${NATIVE_SCOPE_RUNNER[@]}" python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" \
      run-native "$TRAILBASE_ROOT" \
      --public-url "$TRAILBASE_PUBLIC_URL" \
      --address "$TRAILBASE_PUBLIC_ADDR" \
      --admin-address "$TRAILBASE_ADMIN_ADDR" \
      --cors-origin "$TRAILBASE_PUBLIC_URL" >"$LOGDIR/trailbase.log" 2>&1
  ) &
  pid=$!
  set +m
  _write_pidfile trailbase "$pid"
  for _ in {1..50}; do
    kill -0 "$pid" 2>/dev/null || break
    _trailbase_health && break
    sleep 0.1
  done
  if ! kill -0 "$pid" 2>/dev/null || ! _trailbase_health || ! _trailbase_route_boundary; then
    _stop_pidfile trailbase
    echo "TrailBase failed its liveness or private-route boundary; see .dev-logs/trailbase.log" >&2
    return 1
  fi
  echo "TrailBase v0.33.5 is running on $TRAILBASE_PUBLIC_URL (private admin: http://$TRAILBASE_ADMIN_ADDR)."
  echo "Fasti session exchange remains unavailable until Package C1."
}

_trailbase_start_container() {
  local runtime="$1"
  local existing_runtime=""
  local reference=""
  local user_id=""
  local group_id=""
  local user_args=()
  local pid=""
  if pid="$(_tracked_pid trailbase 2>/dev/null)"; then
    echo "Stop the running native TrailBase process before starting container mode (PID: $pid)." >&2
    return 1
  fi
  if existing_runtime="$(_trailbase_container_runtime 2>/dev/null)"; then
    python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" verify-oci-container \
      "$TRAILBASE_ROOT" --runtime "$existing_runtime" --name "$TRAILBASE_CONTAINER_NAME" >/dev/null
    echo "TrailBase container $TRAILBASE_CONTAINER_NAME is already running."
    return 0
  fi
  if [[ ! -f "$TRAILBASE_ROOT/bootstrap.json" ]]; then
    echo "TrailBase is not initialized. Run './scripts/dev.sh trailbase initialize' first." >&2
    return 1
  fi
  python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" verify-root "$TRAILBASE_ROOT" >/dev/null
  reference="$(python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" prepare-oci "$TRAILBASE_ROOT" --runtime "$runtime" --offline)"
  local port_probe_result=0
  _port_in_use 4000 || port_probe_result=$?
  if ((port_probe_result == 2)); then
    echo "Cannot verify TrailBase port 4000 availability (ss probe unavailable)" >&2
    return 1
  elif ((port_probe_result == 0)); then
    echo "TrailBase port 4000 is already in use" >&2
    return 1
  fi
  user_id="$(id -u)"
  group_id="$(id -g)"
  if [[ "$user_id" == 0 ]]; then
    echo "Container mode refuses to run TrailBase as root" >&2
    return 1
  fi
  user_args=(--user "$user_id:$group_id")
  if [[ "$runtime" == podman ]]; then
    user_args=(--userns keep-id --user "$user_id:$group_id")
  fi
  "$runtime" run -d --name "$TRAILBASE_CONTAINER_NAME" --rm --pull never \
    --log-driver none \
    "${user_args[@]}" \
    --memory 192m --memory-swap 192m --cpus 1 --pids-limit 128 \
    --read-only --security-opt no-new-privileges --cap-drop ALL \
    --publish 127.0.0.1:4000:4000 \
    --volume "$TRAILBASE_ROOT:/app/trailroot:Z" \
    --entrypoint /usr/bin/flock \
    "$reference" \
    /app/trailroot/runtime.lock \
    /app/trail \
    --depot /app/trailroot/depot \
    --public-url "$TRAILBASE_PUBLIC_URL" \
    run \
    --address 0.0.0.0:4000 \
    --admin-address 127.0.0.1:4001 \
    --cors-allowed-origins "$TRAILBASE_PUBLIC_URL" \
    --runtime-threads 1 \
    --stderr-logging >/dev/null
  for _ in {1..50}; do
    [[ "$("$runtime" inspect "$TRAILBASE_CONTAINER_NAME" --format '{{.State.Running}}' 2>/dev/null || true)" == true ]] || break
    _trailbase_health && break
    sleep 0.1
  done
  if ! _trailbase_health || ! _trailbase_route_boundary oci; then
    "$runtime" stop "$TRAILBASE_CONTAINER_NAME" >/dev/null 2>&1 || true
    echo "TrailBase $runtime container failed its liveness or public-route boundary." >&2
    return 1
  fi
  if ! python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" verify-oci-container \
    "$TRAILBASE_ROOT" --runtime "$runtime" --name "$TRAILBASE_CONTAINER_NAME" >/dev/null; then
    "$runtime" stop "$TRAILBASE_CONTAINER_NAME" >/dev/null 2>&1 || true
    echo "TrailBase $runtime container identity differs from the release lock." >&2
    return 1
  fi
  if [[ "$("$runtime" exec "$TRAILBASE_CONTAINER_NAME" /app/trail --version | head -1)" != "trail v0.33.5-0-gb4c85d51 (2026-08-27)" ]]; then
    "$runtime" stop "$TRAILBASE_CONTAINER_NAME" >/dev/null 2>&1 || true
    echo "TrailBase $runtime container executable version differs from the release lock." >&2
    return 1
  fi
  echo "TrailBase v0.33.5 is running from the exact $runtime OCI digest on $TRAILBASE_PUBLIC_URL."
  echo "The admin listener remains inside the container and is not host-published."
  echo "Fasti session exchange remains unavailable until Package C1."
}

_trailbase_start() {
  case "${1:-native}" in
    native) _trailbase_start_native ;;
    --podman) _trailbase_start_container podman ;;
    --docker) _trailbase_start_container docker ;;
    *) echo "Usage: ./scripts/dev.sh trailbase start [--podman|--docker]" >&2; return 1 ;;
  esac
}

_trailbase_stop() {
  local runtime=""
  _stop_pidfile trailbase
  for runtime in podman docker; do
    command -v "$runtime" >/dev/null 2>&1 || continue
    "$runtime" stop "$TRAILBASE_CONTAINER_NAME" >/dev/null 2>&1 || true
  done
  echo "Stopped TrailBase for Fasti dev scope $DEV_SCOPE."
}

_trailbase_backup() {
  python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" \
    backup-depot "$TRAILBASE_ROOT" "$PROJECT_ROOT/.dev-trailbase-backups"
}

_trailbase_restore() {
  if (($# != 2)); then
    echo "Usage: ./scripts/dev.sh trailbase restore BACKUP ISOLATED_TARGET" >&2
    return 1
  fi
  python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" restore-depot "$1" "$2"
}

_trailbase_status() {
  local pid=""
  local container_runtime=""
  python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" verify-release >/dev/null
  echo "=== TrailBase Dev Status ($DEV_SCOPE) ==="
  echo "  Release: v0.33.5 (exact lock verified at command time)"
  echo "  Root: $TRAILBASE_ROOT"
  echo "  Account URL: $TRAILBASE_PUBLIC_URL"
  echo "  Admin URL: http://$TRAILBASE_ADMIN_ADDR (loopback only)"
  if [[ ! -f "$TRAILBASE_ROOT/bootstrap.json" ]]; then
    echo "  State: NOT INITIALIZED"
    echo "  Next action: ./scripts/dev.sh trailbase initialize"
    echo "  Fasti session exchange: UNAVAILABLE UNTIL PACKAGE C1"
    return 0
  fi
  if ! python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" verify-root "$TRAILBASE_ROOT" >/dev/null; then
    echo "  State: NEEDS ATTENTION (private root or bootstrap receipt failed verification)"
    return 1
  fi
  if pid="$(_tracked_pid trailbase 2>/dev/null)"; then
    echo "  Process: RUNNING (PID: $pid)"
    if _trailbase_health && _trailbase_route_boundary; then
      echo "  Evidence: liveness healthy; admin absent from account listener; no Record API configured"
    else
      echo "  Evidence: NEEDS ATTENTION (liveness or route boundary failed)"
      return 1
    fi
  elif container_runtime="$(_trailbase_container_runtime 2>/dev/null)"; then
    echo "  Process: RUNNING ($container_runtime container: $TRAILBASE_CONTAINER_NAME)"
    if ! python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" verify-oci-container \
      "$TRAILBASE_ROOT" --runtime "$container_runtime" --name "$TRAILBASE_CONTAINER_NAME" >/dev/null; then
      echo "  Evidence: NEEDS ATTENTION (running OCI identity or isolation policy failed verification)"
      return 1
    fi
    if _trailbase_health && _trailbase_route_boundary oci; then
      echo "  Evidence: exact OCI identity; liveness healthy; admin not published; no Record API configured"
    else
      echo "  Evidence: NEEDS ATTENTION (OCI identity, liveness, or route boundary failed)"
      return 1
    fi
  else
    rm -f "$RUNDIR/trailbase.pid"
    echo "  Process: STOPPED"
    echo "  Next action: ./scripts/dev.sh trailbase start"
  fi
  echo "  Fasti session exchange: UNAVAILABLE UNTIL PACKAGE C1"
}

_trailbase_help() {
  cat <<'EOF'
Usage: ./scripts/dev.sh trailbase {initialize|start|status|stop|backup|restore|help}

Prepare first: ./scripts/dev.sh --prepare-offline
Initialize once from the owning terminal: ./scripts/dev.sh trailbase initialize
Start native: ./scripts/dev.sh trailbase start
Start OCI: ./scripts/dev.sh trailbase start --podman|--docker
Restore: ./scripts/dev.sh trailbase restore BACKUP ISOLATED_TARGET

Requires Linux, Python 3, curl, and ss. Native resource limits require user
systemd with cgroup v2. OCI mode requires an installed Podman or Docker runtime.
EOF
}

_trailbase_command() {
  local command="${1:-}"
  shift 1 2>/dev/null || true
  if [[ "$command" == start ]] && (($# > 1)); then
    echo "TrailBase start accepts at most one --podman or --docker argument" >&2
    return 1
  elif [[ "$command" != restore && "$command" != start ]] && (($#)); then
    echo "TrailBase command '$command' does not accept additional arguments" >&2
    return 1
  fi
  case "$command" in
    help|--help|-h) _trailbase_help ;;
    initialize) _trailbase_initialize ;;
    start) _trailbase_start "${1:-native}" ;;
    status) _trailbase_status ;;
    stop) _trailbase_stop ;;
    backup) _trailbase_backup ;;
    restore) _trailbase_restore "$@" ;;
    *) _trailbase_help >&2; return 1 ;;
  esac
}

_prepare_offline() {
  echo "=== Preparing locked Fasti and TrailBase inputs for offline use ==="
  cargo fetch --locked
  if _has_web; then
    pnpm fetch --frozen-lockfile
  fi
  python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" prepare-native "$TRAILBASE_ROOT" >/dev/null
  python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" prepare-upgrade-fixture "$TRAILBASE_ROOT" >/dev/null
  python3 -B "$PROJECT_ROOT/scripts/trailbase_runtime.py" prepare-oci "$TRAILBASE_ROOT" --runtime "$FASTI_CONTAINER_RUNTIME" >/dev/null
  echo "Prepared locked Rust, pnpm, exact TrailBase v0.33.5 native/OCI, and v0.33.4 upgrade inputs."
}

# Re-reads FASTI_API_URL from the bound-address file if the daemon picked a
# fallback port and the caller didn't pin FASTI_API_URL/FASTI_PUBLIC_URL
# explicitly. Shared by _status and _open so both report the port actually
# in use, not just the preferred one.
_resolve_actual_api_url() {
  ((!FASTI_API_URL_EXPLICIT)) && [[ -s "$BOUND_ADDR_FILE" ]] || return 0
  FASTI_API_URL="$(_api_url_for_addr "$(<"$BOUND_ADDR_FILE")")"
}

_cleanup() {
  trap - EXIT INT TERM
  _stop_processes
}

_status_line() {
  local label="$1"
  local name="$2"
  local pid=""
  if pid="$(_tracked_pid "$name")"; then
    printf '  %-19s RUNNING (PID: %s)\n' "$label:" "$pid"
  else
    rm -f "$RUNDIR/$name.pid"
    printf '  %-19s NOT RUNNING\n' "$label:"
  fi
}

_container_runtime_for_scope() {
  local runtime=""
  for runtime in podman docker; do
    command -v "$runtime" >/dev/null 2>&1 || continue
    if [[ "$("$runtime" inspect "$CONTAINER_NAME" --format '{{.State.Running}}' 2>/dev/null || true)" == true ]]; then
      printf '%s\n' "$runtime"
      return 0
    fi
  done
  return 1
}

_status() {
  local health=""
  local container_runtime=""
  local daemon_pid=""
  daemon_pid="$(_tracked_pid daemon 2>/dev/null || true)"
  container_runtime="$(_container_runtime_for_scope 2>/dev/null || true)"
  if [[ -z "$daemon_pid" && -z "$container_runtime" ]]; then
    rm -f "$BOUND_ADDR_FILE"
  else
    _resolve_actual_api_url
  fi
  echo "=== Fasti Dev Status ($DEV_SCOPE) ==="
  _status_line "Daemon (fastid)" daemon
  if _has_web; then
    _status_line "Web (Vite)" web
  fi
  if [[ -n "$container_runtime" ]]; then
    printf '  %-19s RUNNING (%s)\n' "Container:" "$container_runtime"
  else
    printf '  %-19s NOT RUNNING\n' "Container:"
  fi
  echo ""
  if _tracked_pid web >/dev/null 2>&1; then
    echo "  Web URL: http://127.0.0.1:$WEB_PORT"
  fi
  echo "  API URL: $FASTI_API_URL"
  if [[ -n "$FASTI_PUBLIC_URL" ]]; then
    echo "  Public URL: $FASTI_PUBLIC_URL"
  else
    echo "  Public URL: NOT CONFIGURED"
  fi
  echo "  Port fallback: $FASTI_PORT_FALLBACK"
  if health="$(curl --connect-timeout 2 --max-time 5 --silent --fail "$FASTI_API_URL/api/v1/health" 2>/dev/null)"; then
    echo "  API Probe: HEALTHY ($health)"
  else
    echo "  API Probe: NOT REACHABLE"
  fi
}

_open() {
  local custom_target="${1:-}"
  local target=""

  if [[ -n "$custom_target" ]]; then
    if [[ "$custom_target" == /* ]]; then
      if [[ -n "$FASTI_PUBLIC_URL" ]]; then
        target="${FASTI_PUBLIC_URL%/}$custom_target"
      else
        target="http://127.0.0.1:$WEB_PORT$custom_target"
      fi
    elif [[ "$custom_target" =~ ^[0-9]+$ ]]; then
      target="http://127.0.0.1:$custom_target"
    elif [[ "$custom_target" == http://* || "$custom_target" == https://* ]]; then
      target="$custom_target"
    elif [[ "$custom_target" == localhost* || "$custom_target" == 127.0.0.1* || "$custom_target" == *.local* ]]; then
      target="http://$custom_target"
    else
      target="http://$custom_target"
    fi
  elif [[ -n "$FASTI_PUBLIC_URL" ]]; then
    target="$FASTI_PUBLIC_URL"
  elif _has_web; then
    target="http://127.0.0.1:$WEB_PORT"
  else
    _resolve_actual_api_url
    target="$FASTI_API_URL/api/v1/health"
  fi

  _validate_open_target "$target"
  echo "Opening browser target..."
  if command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$target" >/dev/null 2>&1 &
  elif command -v open >/dev/null 2>&1; then
    open "$target" >/dev/null 2>&1 &
  elif command -v powershell.exe >/dev/null 2>&1; then
    FASTI_OPEN_TARGET="$target" powershell.exe -NoProfile -Command 'Start-Process -FilePath $env:FASTI_OPEN_TARGET' >/dev/null 2>&1 &
  else
    echo "No supported browser opener found." >&2
    return 1
  fi
}

_update() {
  echo "=== Updating Fasti to latest dev ==="
  (
    cd "$PROJECT_ROOT" || exit 1
    git fetch origin || { echo "git fetch failed" >&2; return 1; }
    local current_branch
    current_branch="$(git branch --show-current 2>/dev/null || echo "")"
    if [[ "$current_branch" == "dev" ]]; then
      echo "Pulling latest changes on dev..."
      git pull --ff-only || { echo "git pull failed" >&2; return 1; }
    else
      echo "Rebasing $current_branch on origin/dev..."
      git rebase origin/dev || { echo "git rebase failed" >&2; return 1; }
    fi
    echo "Fetching Cargo dependencies..."
    cargo fetch --locked
    if _has_web; then
      echo "Installing pnpm dependencies and building packages..."
      pnpm install --frozen-lockfile
      pnpm run build
    fi
    echo "=== Fasti is up to date. Run './scripts/dev.sh' to launch. ==="
  )
}

_stop() {
  local runtime=""
  echo "Stopping Fasti dev scope $DEV_SCOPE..."
  _stop_processes
  for runtime in podman docker; do
    command -v "$runtime" >/dev/null 2>&1 || continue
    "$runtime" stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
    "$runtime" stop "$TRAILBASE_CONTAINER_NAME" >/dev/null 2>&1 || true
  done
  rm -f "$BOUND_ADDR_FILE"
  echo "Stopped Fasti dev scope $DEV_SCOPE."
}

_validated_reset_root() {
  python3 - "$PROJECT_ROOT" "$1" <<'PY'
from pathlib import Path
import subprocess
import sys

project = Path(sys.argv[1]).resolve(strict=True)
candidate = Path(sys.argv[2]).resolve(strict=False)
expected = project / ".dev-data"

try:
    top = Path(
        subprocess.run(
            ["git", "-C", str(project), "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    ).resolve(strict=True)
except (OSError, subprocess.CalledProcessError):
    raise SystemExit("reset requires a Git worktree")

if top != project:
    raise SystemExit("reset root is ambiguous because the launcher is not at the worktree root")
if candidate != expected:
    raise SystemExit(f"reset refuses non-development, outside-worktree, or symlink-escaped root: {candidate}")
print(candidate)
PY
}

_data_root_is_active() {
  local root="$1"
  local lock="$root/fasti.lock"
  [[ -e "$lock" ]] || return 1
  [[ -f "$lock" && ! -L "$lock" ]] || return 0
  command -v flock >/dev/null 2>&1 || return 0
  local lock_fd
  exec {lock_fd}<"$lock" || return 0
  if flock --nonblock "$lock_fd"; then
    flock --unlock "$lock_fd"
    exec {lock_fd}>&-
    return 1
  fi
  exec {lock_fd}>&-
  return 0
}

_confirm_full_dev_root_reset() {
  local root="$1"
  local expected="RESET $root"
  local answer=""
  printf 'This removes all Fasti development data in this root, including Chronicle data.\n'
  printf 'Type %s to continue: ' "$expected"
  IFS= read -r answer || return 1
  [[ "$answer" == "$expected" ]]
}

_rebuild_fasti_root() {
  local root="$1"
  local reset_bound_addr="$RUNDIR/reset-bound-addr"
  local reset_log="$LOGDIR/reset-fastid.log"
  local pid=""
  local addr=""
  local api_url=""
  local status=""

  (umask 077; mkdir -p "$root" "$LOGDIR" "$RUNDIR")
  rm -f "$reset_bound_addr"
  set -m
  (
    umask 077
    FASTI_DATA_ROOT="$root" \
      FASTI_LISTEN=127.0.0.1:0 \
      FASTI_PORT_FALLBACK=fail \
      FASTI_BOUND_ADDR_FILE="$reset_bound_addr" \
      exec "$PROJECT_ROOT/target/debug/fastid" >"$reset_log" 2>&1
  ) &
  pid=$!
  set +m
  _write_pidfile reset-daemon "$pid"
  for _ in {1..50}; do
    kill -0 "$pid" 2>/dev/null || break
    [[ -s "$reset_bound_addr" ]] && break
    sleep 0.1
  done
  if [[ ! -s "$reset_bound_addr" ]]; then
    _stop_pidfile reset-daemon
    echo "Fasti reset migration failed; see $reset_log" >&2
    return 1
  fi
  addr="$(<"$reset_bound_addr")"
  api_url="$(_api_url_for_addr "$addr")"
  for _ in {1..20}; do
    if curl --connect-timeout 2 --max-time 5 --silent --fail "$api_url/api/v1/health" >/dev/null 2>&1; then
      status="$(curl --connect-timeout 2 --max-time 5 --silent --output /dev/null \
        --write-out '%{http_code}' --request POST --header 'content-type: application/json' \
        --data '{}' "$api_url/api/v1/node/initialization" || true)"
      [[ "$status" == 403 ]] && break
    fi
    sleep 0.1
  done
  _stop_pidfile reset-daemon
  rm -f "$reset_bound_addr"
  if [[ "$status" != 403 ]]; then
    echo "Fasti reset migration did not expose the durable initialization surface; see $reset_log" >&2
    return 1
  fi
}

_reset_access() {
  local selection="${1:-}"
  local root=""
  local backup=""
  local failed=""

  case "$selection" in
    "") ;;
    --full-dev-root) ;;
    *) echo "--reset-access accepts only --full-dev-root" >&2; return 1 ;;
  esac

  root="$(_validated_reset_root "$DATADIR")" || return 1
  echo "=== Fasti Access Development Reset ($DEV_SCOPE) ==="
  echo "  Fasti development root: $root"
  echo "  TrailBase development root: $TRAILBASE_ROOT (separate and retained)"

  if [[ "$selection" != --full-dev-root ]]; then
    echo "Access-only reset is unavailable because no public Access reset service is mounted." >&2
    echo "No data changed. Use --full-dev-root only when Chronicle data may also be reset." >&2
    return 2
  fi

  _confirm_full_dev_root_reset "$root" || {
    echo "Reset canceled; no data changed." >&2
    return 1
  }
  trap _cleanup EXIT
  trap '_cleanup; exit 130' INT
  trap '_cleanup; exit 143' TERM
  _stop
  if _data_root_is_active "$root"; then
    echo "Reset refused because the Fasti development root is active or its lock is ambiguous." >&2
    return 1
  fi

  cargo build --locked --bin fastid
  (umask 077; mkdir -p "$PROJECT_ROOT/.dev-reset-backups")
  if [[ -e "$root" ]]; then
    backup="$PROJECT_ROOT/.dev-reset-backups/fasti-$(date -u +%Y%m%dT%H%M%SZ)-$$"
    mv -- "$root" "$backup"
    echo "Previous Fasti development root retained at: $backup"
  fi

  if ! _rebuild_fasti_root "$root"; then
    if [[ -e "$root" ]]; then
      failed="$PROJECT_ROOT/.dev-reset-backups/failed-$(date -u +%Y%m%dT%H%M%SZ)-$$"
      mv -- "$root" "$failed"
      echo "Failed replacement root retained at: $failed" >&2
    fi
    if [[ -n "$backup" && -d "$backup" ]]; then
      mv -- "$backup" "$root"
      echo "Previous Fasti development root restored after the failed rebuild." >&2
    fi
    return 1
  fi
  echo "Fasti development root rebuilt through normal forward migrations and public service probes."
  echo "TrailBase development data was retained unchanged; use its backup and isolated restore commands."
  trap - EXIT INT TERM
}

_require_container_image() {
  if ! command -v "$FASTI_CONTAINER_RUNTIME" >/dev/null 2>&1; then
    echo "$FASTI_CONTAINER_RUNTIME is not installed or not on PATH" >&2
    return 1
  fi
  if "$FASTI_CONTAINER_RUNTIME" image inspect "$FASTI_IMAGE" >/dev/null 2>&1; then
    return 0
  fi
  printf 'Container image %s is not available. Build it with: %q build --tag %q %q\n' \
    "$FASTI_IMAGE" "$FASTI_CONTAINER_RUNTIME" "$FASTI_IMAGE" "$PROJECT_ROOT" >&2
  return 1
}

_port_in_use() {
  # Return 2 (probe unavailable) when ss is missing or fails, distinct from
  # 1 (port free) -- callers must treat "couldn't check" as "don't proceed",
  # not silently equivalent to "the port is free".
  command -v ss >/dev/null 2>&1 || return 2
  local listeners
  listeners="$(ss -H -ltn "sport = :$1" 2>/dev/null)" || return 2
  [[ -n "$listeners" ]]
}

_container_port() {
  local mapping=""
  mapping="$("$FASTI_CONTAINER_RUNTIME" port "$CONTAINER_NAME" 8420/tcp)"
  mapping="${mapping%%$'\n'*}"
  [[ "$mapping" == 127.0.0.1:* ]] || return 1
  printf '%s\n' "${mapping##*:}"
}

_run_container() {
  local ceiling_mib=""
  local user_id=""
  local group_id=""
  local user_args=()
  user_id="$(id -u)"
  if [[ "$user_id" == 0 ]]; then
    echo "Container mode refuses to run Fasti as root" >&2
    return 1
  fi
  group_id="$(id -g)"
  user_args=(--user "$user_id:$group_id")
  ceiling_mib="$(_memory_ceiling_mib)"
  if [[ "$FASTI_CONTAINER_RUNTIME" == podman ]]; then
    user_args=(--userns keep-id --user "$user_id:$group_id")
  fi
  "$FASTI_CONTAINER_RUNTIME" run -d --name "$CONTAINER_NAME" --rm \
    "${user_args[@]}" \
    --memory "${ceiling_mib}m" --memory-swap "${ceiling_mib}m" \
    --publish "$1" \
    -v "$DATADIR:/data:Z" \
    -e FASTI_DATA_ROOT=/data \
    -e FASTI_EXTERNAL_BIND_IP=127.0.0.1 \
    "$FASTI_IMAGE"
}

_start_container() {
  if _tracked_pid daemon >/dev/null 2>&1; then
    echo "Fasti native daemon is currently running. Stopping native daemon to switch to container mode..."
    _stop_pidfile daemon
    sleep 0.5
  fi
  local running_runtime=""
  if running_runtime="$(_container_runtime_for_scope 2>/dev/null)"; then
    echo "Fasti $running_runtime container ($CONTAINER_NAME) is already running on $FASTI_API_URL."
    return 0
  fi

  local publish="127.0.0.1:$FASTI_PORT:8420"
  local used_fallback=0
  local actual_port="$FASTI_PORT"
  if [[ "$FASTI_LISTEN" != "127.0.0.1:$FASTI_PORT" ]]; then
    echo "Container mode supports FASTI_LISTEN=127.0.0.1:FASTI_PORT only; the container still listens on 0.0.0.0:8420 internally" >&2
    return 1
  fi
  local container_port_status=0
  _port_in_use "$FASTI_PORT" || container_port_status=$?
  if ((container_port_status == 2)); then
    echo "Cannot verify port $FASTI_PORT availability (ss probe unavailable)" >&2
    return 1
  elif ((container_port_status == 0)); then
    if [[ "$FASTI_PORT_FALLBACK" == fail ]]; then
      echo "FASTI_PORT $FASTI_PORT is already in use" >&2
      return 1
    fi
    publish="127.0.0.1::8420"
    used_fallback=1
  fi
  if ((used_fallback)) && { ((FASTI_API_URL_EXPLICIT)) || [[ -n "$FASTI_PUBLIC_URL" ]]; }; then
    echo "The preferred port is occupied. Automatic fallback is unsafe with FASTI_API_URL or FASTI_PUBLIC_URL configured." >&2
    return 1
  fi

  echo "=== Launching Fasti $FASTI_CONTAINER_RUNTIME container ($CONTAINER_NAME) ==="
  mkdir -p "$DATADIR"
  _require_container_image
  # ponytail: use the portable listener preflight; a bind race fails closed.
  if ! _run_container "$publish"; then
    echo "Failed to start $FASTI_CONTAINER_RUNTIME container $CONTAINER_NAME" >&2
    return 1
  fi

  if ((used_fallback)); then
    actual_port="$(_container_port)" || {
      echo "Could not resolve the fallback container port" >&2
      "$FASTI_CONTAINER_RUNTIME" stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
      return 1
    }
    FASTI_API_URL="http://127.0.0.1:$actual_port"
    echo "Preferred port $FASTI_PORT was occupied; using $actual_port on 127.0.0.1."
  fi
  _write_bound_addr "127.0.0.1:$actual_port"

  if _wait_for_health && _durable_api_is_mounted; then
    echo "Fasti $FASTI_CONTAINER_RUNTIME container is healthy with durable routes on $FASTI_API_URL"
  else
    echo "Fasti $FASTI_CONTAINER_RUNTIME container failed its health or durable-route probe:" >&2
    "$FASTI_CONTAINER_RUNTIME" logs "$CONTAINER_NAME" >&2 || true
    "$FASTI_CONTAINER_RUNTIME" stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
    return 1
  fi
}

_start_native() {
  trap _cleanup EXIT
  trap '_cleanup; exit 130' INT
  trap '_cleanup; exit 143' TERM
  set -m
  mkdir -p "$LOGDIR" "$DATADIR" "$RUNDIR"
  rm -f "$BOUND_ADDR_FILE"

  local running_runtime=""
  if running_runtime="$(_container_runtime_for_scope 2>/dev/null)"; then
    echo "Fasti $running_runtime container ($CONTAINER_NAME) is currently running. Stopping container to switch to native mode..."
    "$running_runtime" stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
    sleep 0.5
  fi
  if _tracked_pid daemon >/dev/null 2>&1; then
    echo "Fasti native daemon is already running. Stopping previous instance..."
    _stop_pidfile daemon
    sleep 0.5
  fi

  echo "=== 1. Compiling and starting Fasti daemon ==="
  _configure_native_scope
  cargo build --locked --bin fastid
  export FASTI_LISTEN FASTI_API_URL FASTI_PORT_FALLBACK
  export FASTI_BOUND_ADDR_FILE="$BOUND_ADDR_FILE"
  export FASTI_DATA_ROOT="$DATADIR"

  "${NATIVE_SCOPE_RUNNER[@]}" "$PROJECT_ROOT/target/debug/fastid" > "$LOGDIR/fastid.log" 2>&1 &
  local daemon_pid=$!
  set +m
  _write_pidfile daemon "$daemon_pid"
  echo "Fasti daemon started (PID: $daemon_pid, log: .dev-logs/fastid.log)"

  local actual_addr=""
  actual_addr="$(_read_bound_addr "$daemon_pid")" || {
    echo "Fasti daemon did not publish its bound address; see .dev-logs/fastid.log" >&2
    return 1
  }
  if _listener_fell_back "$actual_addr"; then
    if ((FASTI_API_URL_EXPLICIT)) || [[ -n "$FASTI_PUBLIC_URL" ]]; then
      echo "The preferred port is occupied. Automatic fallback is unsafe with FASTI_API_URL or FASTI_PUBLIC_URL configured." >&2
      return 1
    fi
    FASTI_API_URL="$(_api_url_for_addr "$actual_addr")"
    echo "Preferred listener $FASTI_LISTEN was occupied; using $actual_addr."
  elif ((!FASTI_API_URL_EXPLICIT)); then
    FASTI_API_URL="$(_api_url_for_addr "$actual_addr")"
  fi

  echo "Waiting for daemon health probe..."
  if _wait_for_health "$daemon_pid" && _durable_api_is_mounted; then
    echo "Fasti daemon is healthy with durable routes on $FASTI_API_URL"
  else
    echo "Fasti daemon failed its health or durable-route probe; see .dev-logs/fastid.log" >&2
    return 1
  fi

  if _has_web; then
    local port_status=0
    _port_in_use "$WEB_PORT" || port_status=$?
    if ((port_status == 2)); then
      echo "Cannot verify port $WEB_PORT availability (ss probe unavailable); not starting the web workbench." >&2
    elif ((port_status == 0)); then
      echo "Port $WEB_PORT is already in use; not starting the web workbench." >&2
    else
      echo "=== 2. Building and starting the web workbench ==="
      # apps/web imports @fasti/tokens and @fasti/sdk as built workspace
      # packages, not raw TS sources -- build them first. Filtered to just
      # web's dependency chain, not the whole repo, for a faster dev loop.
      if ! pnpm --filter @fasti/tokens --filter @fasti/sdk --filter @fasti/ui --filter @fasti/web run build >"$LOGDIR/web-build.log" 2>&1; then
        echo "Web workbench build failed; see $LOGDIR/web-build.log" >&2
        return 1
      fi
      set -m
      FASTI_QA_PROXY_TARGET="$FASTI_API_URL" pnpm --filter @fasti/web run dev >"$LOGDIR/web.log" 2>&1 &
      local web_pid=$!
      set +m
      _write_pidfile web "$web_pid"
      echo "Web workbench starting: http://127.0.0.1:$WEB_PORT (see .dev-logs/web.log)"
    fi
  else
    echo "apps/web is not present in this worktree; skipping web workbench."
  fi

  echo "Press Ctrl+C or run ./scripts/dev.sh --stop to shut down."
  wait "$daemon_pid"
}

_start_desktop() {
  local manifest="$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml"
  if (( ! FASTI_DATA_ROOT_EXPLICIT )); then
    echo "Desktop mode requires an explicit private FASTI_DATA_ROOT." >&2
    return 1
  fi
  if [[ ! -f "$manifest" || ! -f "$PROJECT_ROOT/apps/web/package.json" ]]; then
    echo "Desktop sources are not present in this worktree." >&2
    return 1
  fi

  (umask 077; mkdir -p "$DATADIR")
  [[ -n "${PKG_CONFIG:-}" || ! -x /usr/bin/pkg-config ]] || export PKG_CONFIG=/usr/bin/pkg-config
  pnpm --dir "$PROJECT_ROOT" run build
  FASTI_DATA_ROOT="$DATADIR" cargo run --locked \
    --manifest-path "$manifest" \
    --bin fasti-desktop
}

_self_test() {
  # Nested self-invocations ("$0" --status) must not inherit this shell's
  # already-resolved FASTI_API_URL/FASTI_PUBLIC_URL -- each assertion sets
  # exactly the variables it means to test and expects the rest to fall
  # back to fresh defaults, not whatever the caller's environment exported.
  unset FASTI_API_URL FASTI_PUBLIC_URL
  local old_rundir="$RUNDIR"
  local ceiling_mib=""
  local exec_comm=""
  local exec_go=""
  local exec_pid=""
  local exec_ready=""
  local leader_file=""
  local desktop_calls=""
  local old_datadir="$DATADIR"
  local old_data_root_explicit="$FASTI_DATA_ROOT_EXPLICIT"
  RUNDIR="$(mktemp -d)"
  leader_file="$RUNDIR/leader"
  exec_ready="$RUNDIR/exec-ready"
  exec_go="$RUNDIR/exec-go"
  desktop_calls="$RUNDIR/desktop-calls"
  trap '_stop_pidfile child; _stop_pidfile exec-child; rm -f "$RUNDIR/stale.pid" "$RUNDIR/leader" "$RUNDIR/exec-ready" "$RUNDIR/exec-go" "$RUNDIR/desktop-calls"; rmdir "$RUNDIR/desktop-data" 2>/dev/null || true; rmdir "$RUNDIR" 2>/dev/null || true' EXIT
  # The values expand in the child shell.
  # shellcheck disable=SC2016
  setsid --fork --wait bash -c 'printf "%s\n" "$$" > "$1"; trap "" TERM; sleep 30 & wait' _ "$leader_file" 2>/dev/null &
  local launcher=$!
  for _ in {1..10}; do
    [[ -s "$leader_file" ]] && break
    sleep 0.1
  done
  if [[ ! -s "$leader_file" ]]; then
    kill "$launcher" 2>/dev/null || true
    wait "$launcher" 2>/dev/null || true
    echo "self-test process group did not report its leader" >&2
    return 1
  fi
  local leader=""
  leader="$(<"$leader_file")"
  _write_pidfile child "$leader"
  [[ "$(_tracked_pid child)" == "$leader" ]]
  _stop_pidfile child
  wait "$launcher" 2>/dev/null || true
  if kill -0 "$leader" 2>/dev/null; then
    echo "self-test process group survived forced cleanup" >&2
    return 1
  fi
  bash -c 'printf "%s\n" "$$" > "$1"; while [[ ! -e "$2" ]]; do sleep 0.01; done; exec sleep 30' _ "$exec_ready" "$exec_go" &
  exec_pid=$!
  for _ in {1..20}; do
    [[ -s "$exec_ready" ]] && break
    sleep 0.01
  done
  [[ "$(<"$exec_ready")" == "$exec_pid" ]]
  _write_pidfile exec-child "$exec_pid"
  : > "$exec_go"
  for _ in {1..20}; do
    exec_comm="$(ps -p "$exec_pid" -o comm= 2>/dev/null)"
    exec_comm="${exec_comm//[[:space:]]/}"
    [[ "$exec_comm" == sleep ]] && break
    sleep 0.01
  done
  if [[ "$(_tracked_pid exec-child 2>/dev/null || true)" != "$exec_pid" ]]; then
    echo "self-test lost process identity after exec" >&2
    return 1
  fi
  _stop_pidfile exec-child
  wait "$exec_pid" 2>/dev/null || true
  printf '%s|invalid start time\n' "$$" > "$RUNDIR/stale.pid"
  _stop_pidfile stale
  [[ ! -e "$RUNDIR/stale.pid" ]]
  if FASTI_PORT=0 bash "$0" --status >/dev/null 2>&1; then
    echo "self-test accepted invalid FASTI_PORT" >&2
    return 1
  fi
  local status_output
  status_output="$(FASTI_LISTEN=127.0.0.1:18420 bash "$0" --status)"
  [[ "$status_output" == *"http://127.0.0.1:18420"* ]]
  if FASTI_LISTEN=0.0.0.0:18420 _listener_fell_back 0.0.0.0:18420; then
    echo "self-test treated the requested wildcard listener as a fallback" >&2
    return 1
  fi
  if ! FASTI_LISTEN=0.0.0.0:18420 _listener_fell_back 127.0.0.1:18420; then
    echo "self-test missed a changed listener" >&2
    return 1
  fi
  status_output="$(FASTI_LISTEN=127.0.0.1:18420 FASTI_API_URL=http://localhost:18421 bash "$0" --status)"
  [[ "$status_output" == *"http://localhost:18421"* ]]
  if FASTI_LISTEN='[0:0:0:0:0:0:0:1]:18420' _listener_fell_back '[::1]:18420'; then
    echo "self-test treated equivalent IPv6 listener text as a fallback" >&2
    return 1
  fi
  if ! FASTI_LISTEN='[::1]:18420' _listener_fell_back '[::1]:18421'; then
    echo "self-test missed an IPv6 listener fallback" >&2
    return 1
  fi
  if FASTI_API_URL='http://userinfo-marker@127.0.0.1:18421?query-marker' bash "$0" --status >/dev/null 2>&1; then
    echo "self-test accepted credentials or a query in FASTI_API_URL" >&2
    return 1
  fi
  if FASTI_API_URL=http://127.0.0.1:70000 bash "$0" --status >/dev/null 2>&1; then
    echo "self-test accepted an out-of-range FASTI_API_URL port" >&2
    return 1
  fi
  if FASTI_API_URL=http://127.0.0.1:not-a-port bash "$0" --status >/dev/null 2>&1; then
    echo "self-test accepted a nonnumeric FASTI_API_URL port" >&2
    return 1
  fi
  status_output="$(FASTI_API_URL=http://127.0.0.1:18421/ bash "$0" --status)"
  [[ "$status_output" == *"http://127.0.0.1:18421"* ]]
  if FASTI_CONTAINER_RUNTIME=podman FASTI_LISTEN=0.0.0.0:18420 FASTI_PORT=18420 _start_container >/dev/null 2>&1; then
    echo "self-test accepted an unsupported container listener" >&2
    return 1
  fi
  id() { printf '0\n'; }
  if FASTI_CONTAINER_RUNTIME=podman _run_container 127.0.0.1:18420:8420 >/dev/null 2>&1; then
    echo "self-test accepted a root container process" >&2
    return 1
  fi
  unset -f id
  podman() { return 1; }
  if FASTI_CONTAINER_RUNTIME=podman _require_container_image >/dev/null 2>&1; then
    echo "self-test accepted a missing container image" >&2
    return 1
  fi
  unset -f podman
  _validate_origin_url FASTI_PUBLIC_URL https://fasti.internal
  _validate_origin_url FASTI_API_URL http://localhost:8420
  if _validate_origin_url FASTI_PUBLIC_URL http://fasti.internal >/dev/null 2>&1; then
    echo "self-test accepted non-loopback HTTP" >&2
    return 1
  fi
  if _validate_origin_url FASTI_API_URL 'https://userinfo-marker@fasti.internal' >/dev/null 2>&1; then
    echo "self-test accepted URL user information" >&2
    return 1
  fi
  if _validate_origin_url FASTI_API_URL 'https://fasti.internal:0' >/dev/null 2>&1; then
    echo "self-test accepted URL port zero" >&2
    return 1
  fi
  if _validate_origin_url FASTI_API_URL 'http://127.0.0.1:' >/dev/null 2>&1; then
    echo "self-test accepted an empty URL port" >&2
    return 1
  fi
  [[ "$(_resolve_data_root .custom-data)" == "$PROJECT_ROOT/.custom-data" ]]
  [[ "$(_resolve_data_root /tmp/fasti-custom-data)" == "/tmp/fasti-custom-data" ]]
  local reset_root
  reset_root="$(_validated_reset_root "$PROJECT_ROOT/.dev-data")"
  [[ "$reset_root" == "$PROJECT_ROOT/.dev-data" ]]
  if _validated_reset_root "$PROJECT_ROOT/data" >/dev/null 2>&1; then
    echo "self-test accepted a non-development reset root" >&2
    return 1
  fi
  mkdir -p "$RUNDIR/reset-outside"
  ln -s "$RUNDIR/reset-outside" "$RUNDIR/reset-link"
  if _validated_reset_root "$RUNDIR/reset-link" >/dev/null 2>&1; then
    echo "self-test accepted a symlink-escaped reset root" >&2
    return 1
  fi
  rm -f "$RUNDIR/reset-link"
  rmdir "$RUNDIR/reset-outside"
  if _confirm_full_dev_root_reset "$reset_root" <<<"RESET something-else" >/dev/null 2>&1; then
    echo "self-test accepted an inexact reset confirmation" >&2
    return 1
  fi
  _confirm_full_dev_root_reset "$reset_root" <<<"RESET $reset_root" >/dev/null
  local reset_lock_root="$RUNDIR/reset-lock-root"
  mkdir -p "$reset_lock_root"
  : > "$reset_lock_root/fasti.lock"
  python3 - "$reset_lock_root/fasti.lock" <<'PY' &
import fcntl
import sys
import time

with open(sys.argv[1], "r+", encoding="utf-8") as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    time.sleep(30)
PY
  local reset_lock_pid=$!
  for _ in {1..20}; do
    _data_root_is_active "$reset_lock_root" && break
    sleep 0.01
  done
  if ! _data_root_is_active "$reset_lock_root"; then
    echo "self-test missed an active development-root lock" >&2
    kill "$reset_lock_pid" 2>/dev/null || true
    wait "$reset_lock_pid" 2>/dev/null || true
    return 1
  fi
  kill "$reset_lock_pid" 2>/dev/null || true
  wait "$reset_lock_pid" 2>/dev/null || true
  if _data_root_is_active "$reset_lock_root"; then
    echo "self-test retained an inactive development-root lock" >&2
    return 1
  fi
  rm -f "$reset_lock_root/fasti.lock"
  rmdir "$reset_lock_root"
  FASTI_DATA_ROOT_EXPLICIT=0
  if _start_desktop >/dev/null 2>&1; then
    echo "self-test accepted inferred Desktop data root" >&2
    return 1
  fi
  pnpm() { printf 'pnpm:%s\n' "$*" >> "$desktop_calls"; }
  cargo() { printf 'cargo:%s|data:%s\n' "$*" "$FASTI_DATA_ROOT" >> "$desktop_calls"; }
  FASTI_DATA_ROOT_EXPLICIT=1
  DATADIR="$RUNDIR/desktop-data"
  _start_desktop
  unset -f pnpm cargo
  mapfile -t desktop_invocations < "$desktop_calls"
  [[ "${desktop_invocations[0]}" == "pnpm:--dir $PROJECT_ROOT run build" ]]
  [[ "${desktop_invocations[1]}" == "cargo:run --locked --manifest-path $PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml --bin fasti-desktop|data:$DATADIR" ]]
  DATADIR="$old_datadir"
  FASTI_DATA_ROOT_EXPLICIT="$old_data_root_explicit"
  _validate_open_target 'https://fasti.internal/path?view=settings'
  if _validate_open_target 'https://userinfo-marker@fasti.internal/path' >/dev/null 2>&1; then
    echo "self-test accepted browser target credentials" >&2
    return 1
  fi
  if _validate_open_target $'https://fasti.internal/path\nnext' >/dev/null 2>&1; then
    echo "self-test accepted browser target control characters" >&2
    return 1
  fi
  systemd-run() { return 0; }
  _configure_native_scope
  unset -f systemd-run
  ceiling_mib="$(_memory_ceiling_mib)"
  [[ "${NATIVE_SCOPE_RUNNER[*]}" == *"MemoryMax=$((ceiling_mib * 1024 * 1024))"* ]]
  local help_output
  help_output="$(bash "$0" --help)"
  [[ "$help_output" == *"Fasti Local Development Launcher"* ]]
  [[ "$help_output" == *"--desktop"* ]]
  [[ "$help_output" != *$'\n  fasti '* ]]
  [[ "$(bash "$0" trailbase --help)" == *"Prepare first: ./scripts/dev.sh --prepare-offline"* ]]
  rmdir "$RUNDIR/desktop-data"
  rm -f "$desktop_calls"
  rm -f "$leader_file" "$exec_ready" "$exec_go"
  rmdir "$RUNDIR"
  RUNDIR="$old_rundir"
  trap - EXIT
  echo "dev launcher self-test passed"
}

_help() {
  cat <<'EOF'
Fasti Local Development Launcher

Usage:
  ./scripts/dev.sh [OPTIONS]

Commands / Options:
  (no args), start      Start the native Fasti daemon and web workbench
  --open, open [TARGET] Open the web workbench (or custom URL/domain/port) in the browser
  --status, status      Check this worktree's daemon, web, container, and API health
  --update, update      Pull latest dev, fetch Cargo/pnpm deps, and rebuild packages
  --stop, stop          Stop running daemon, web, and container processes
  --reset-access [--full-dev-root]
                        Validate and reset this worktree's development Access root
  --prepare-offline     Fetch locked Rust, pnpm, and exact TrailBase runtime/test inputs
  trailbase initialize  Initialize TrailBase and rotate its first administrator secret
  trailbase start [--podman|--docker]
                        Start the pinned private native or OCI TrailBase process
  trailbase status      Verify TrailBase process, root, and route boundaries
  trailbase stop        Stop the tracked TrailBase process
  trailbase backup      Create a stopped, complete, digest-bound depot backup
  trailbase restore BACKUP ISOLATED_TARGET
                        Verify and restore a backup without replacing current data
  --podman              Start Fasti in a scoped Podman container
  --docker              Start Fasti in a scoped Docker container
  --container           Start Fasti in a container using the configured runtime
  --desktop, desktop    Build and run the trusted Desktop review host in the foreground
  --self-test           Run dev launcher verification and invariant self-tests
  --help, -h, help      Print this help message

Environment Variables:
  FASTI_PORT                Port for Fasti daemon (default: 8420)
  FASTI_LISTEN              Bind address (default: 127.0.0.1:$FASTI_PORT)
  FASTI_PORT_FALLBACK       Port conflict strategy: auto | fail (default: fail)
  FASTI_CONTAINER_RUNTIME   Container runtime: podman | docker (default: podman)
  FASTI_IMAGE               Container image name (default: fasti:b0)
  FASTI_DATA_ROOT           Data storage path (default: .dev-data; required explicitly for --desktop)
  FASTI_PUBLIC_URL          Public reverse-proxy HTTPS URL (optional)

Examples:
  ./scripts/dev.sh                   # Build and launch daemon and web UI
  ./scripts/dev.sh open              # Open http://127.0.0.1:5173
  ./scripts/dev.sh open 5173         # Open a custom localhost port
  ./scripts/dev.sh open fasti.local  # Open a custom domain
  ./scripts/dev.sh update            # Pull dev and rebuild the workspace
  ./scripts/dev.sh status            # Check services and health
  ./scripts/dev.sh stop              # Stop this worktree's services
  ./scripts/dev.sh --reset-access    # Report the unavailable Access-only reset
  ./scripts/dev.sh --reset-access --full-dev-root
                          # Confirm a full .dev-data reset, including Chronicle
  ./scripts/dev.sh --prepare-offline # Prepare locked inputs before network denial
  ./scripts/dev.sh trailbase initialize
                          # One-time terminal-only administrator bootstrap
  ./scripts/dev.sh trailbase start   # Start private TrailBase without session exchange
  FASTI_DATA_ROOT=/private/path ./scripts/dev.sh desktop
EOF
}

case "${1:-}" in
  --help|-h|help) _help ;;
  --update|update) _update ;;
  --prepare-offline|prepare-offline) _prepare_offline ;;
  trailbase) shift; _trailbase_command "$@" ;;
  --stop|stop) _stop ;;
  --reset-access|reset-access)
    shift
    if (($# > 1)); then
      echo "--reset-access accepts at most one --full-dev-root argument" >&2
      exit 1
    fi
    _reset_access "${1:-}"
    ;;
  --status|status) _status ;;
  --open|open) shift 1 2>/dev/null || true; _open "$@" ;;
  --podman) FASTI_CONTAINER_RUNTIME=podman; _start_container ;;
  --docker) FASTI_CONTAINER_RUNTIME=docker; _start_container ;;
  --container|container) _start_container ;;
  --desktop|desktop) _start_desktop ;;
  --self-test|self-test|selftest) _self_test ;;
  --start|start) _start_native ;;
  "") _start_native ;;
  *)
    echo "Unknown command or option: $1" >&2
    echo "Run './scripts/dev.sh --help' for usage." >&2
    exit 1
    ;;
esac
