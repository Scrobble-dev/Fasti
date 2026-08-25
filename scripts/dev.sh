#!/usr/bin/env bash
# Fasti Local Development Launcher
#
# Usage:
#   ./scripts/dev.sh             # Start the native daemon
#   ./scripts/dev.sh --podman    # Start Fasti in a scoped Podman container
#   ./scripts/dev.sh --docker    # Start Fasti in a scoped Docker container
#   ./scripts/dev.sh --status    # Check this worktree's daemon and API health
#   ./scripts/dev.sh --stop      # Stop this worktree's daemon or container
#   ./scripts/dev.sh --self-test # Verify scoped process cleanup
#
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOGDIR="$PROJECT_ROOT/.dev-logs"
DATADIR="$PROJECT_ROOT/.dev-data"
RUNDIR="$PROJECT_ROOT/.dev-run"
FASTI_PORT="${FASTI_PORT:-8420}"
FASTI_LISTEN="${FASTI_LISTEN:-127.0.0.1:$FASTI_PORT}"
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

_api_url_for_addr() {
  case "$1" in
    0.0.0.0:*) printf 'http://127.0.0.1:%s\n' "${1##*:}" ;;
    \[::\]:*) printf 'http://[::1]:%s\n' "${1##*:}" ;;
    *) printf 'http://%s\n' "$1" ;;
  esac
}

_preferred_addr() {
  case "$FASTI_LISTEN" in
    0.0.0.0:*) printf '127.0.0.1:%s\n' "${FASTI_LISTEN##*:}" ;;
    \[::\]:*) printf '[::1]:%s\n' "${FASTI_LISTEN##*:}" ;;
    *) printf '%s\n' "$FASTI_LISTEN" ;;
  esac
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
import sys

entry = next(line for line in Path("/proc/self/cgroup").read_text().splitlines() if line.startswith("0::"))
root = Path("/sys/fs/cgroup") / entry[3:].lstrip("/")
valid = (root / "memory.max").read_text().strip() == sys.argv[1]
valid = valid and (root / "memory.swap.max").read_text().strip() == "0"
raise SystemExit(0 if valid else 1)
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
  ps -p "$1" -o lstart= -o args= 2>/dev/null | sed 's/^ *//'
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

_stop_processes() {
  _stop_pidfile daemon
  rm -f "$BOUND_ADDR_FILE"
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
  elif ((!FASTI_API_URL_EXPLICIT)) && [[ -s "$BOUND_ADDR_FILE" ]]; then
    FASTI_API_URL="$(_api_url_for_addr "$(<"$BOUND_ADDR_FILE")")"
  fi
  echo "=== Fasti Dev Status ($DEV_SCOPE) ==="
  _status_line "Daemon (fastid)" daemon
  if [[ -n "$container_runtime" ]]; then
    printf '  %-19s RUNNING (%s)\n' "Container:" "$container_runtime"
  else
    printf '  %-19s NOT RUNNING\n' "Container:"
  fi
  echo ""
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

_stop() {
  local runtime=""
  echo "Stopping Fasti dev scope $DEV_SCOPE..."
  _stop_processes
  for runtime in podman docker; do
    command -v "$runtime" >/dev/null 2>&1 || continue
    "$runtime" stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
  done
  rm -f "$BOUND_ADDR_FILE"
  echo "Stopped Fasti dev scope $DEV_SCOPE."
}

_require_container_image() {
  if "$FASTI_CONTAINER_RUNTIME" image inspect "$FASTI_IMAGE" >/dev/null 2>&1; then
    return 0
  fi
  echo "Container image $FASTI_IMAGE is not available. Build it with: $FASTI_CONTAINER_RUNTIME build --tag $FASTI_IMAGE ." >&2
  return 1
}

_port_in_use() {
  command -v ss >/dev/null 2>&1 || return 1
  [[ -n "$(ss -H -ltn "sport = :$1" 2>/dev/null)" ]]
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
  ceiling_mib="$(_memory_ceiling_mib)"
  "$FASTI_CONTAINER_RUNTIME" run -d --name "$CONTAINER_NAME" --rm \
    --memory "${ceiling_mib}m" --memory-swap "${ceiling_mib}m" \
    --publish "$1" \
    -v "$DATADIR:/data:Z" \
    -e FASTI_DATA_ROOT=/data \
    "$FASTI_IMAGE"
}

_start_container() {
  local publish="127.0.0.1:$FASTI_PORT:8420"
  local used_fallback=0
  local actual_port="$FASTI_PORT"
  if [[ "$FASTI_LISTEN" != "127.0.0.1:$FASTI_PORT" ]]; then
    echo "Container mode supports FASTI_LISTEN=127.0.0.1:FASTI_PORT only; the container still listens on 0.0.0.0:8420 internally" >&2
    return 1
  fi
  if _port_in_use "$FASTI_PORT"; then
    if [[ "$FASTI_PORT_FALLBACK" == fail ]]; then
      echo "FASTI_PORT $FASTI_PORT is already in use" >&2
      return 1
    fi
    publish="127.0.0.1::8420"
    used_fallback=1
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
    if ((FASTI_API_URL_EXPLICIT)) || [[ -n "$FASTI_PUBLIC_URL" ]]; then
      echo "The preferred port is occupied. Automatic fallback is unsafe with FASTI_API_URL or FASTI_PUBLIC_URL configured." >&2
      "$FASTI_CONTAINER_RUNTIME" stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
      return 1
    fi
    FASTI_API_URL="http://127.0.0.1:$actual_port"
    echo "Preferred port $FASTI_PORT was occupied; using $actual_port on 127.0.0.1."
  fi
  _write_bound_addr "127.0.0.1:$actual_port"

  if _wait_for_health; then
    echo "Fasti $FASTI_CONTAINER_RUNTIME container is healthy on $FASTI_API_URL"
  else
    echo "Fasti $FASTI_CONTAINER_RUNTIME container failed its health probe; run: $FASTI_CONTAINER_RUNTIME logs $CONTAINER_NAME" >&2
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
  if [[ "$actual_addr" != "$(_preferred_addr)" ]]; then
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
  if _wait_for_health "$daemon_pid"; then
    echo "Fasti daemon is healthy on $FASTI_API_URL"
  else
    echo "Fasti daemon failed to start; see .dev-logs/fastid.log" >&2
    return 1
  fi

  echo "Press Ctrl+C or run ./scripts/dev.sh --stop to shut down."
  wait "$daemon_pid"
}

_self_test() {
  local old_rundir="$RUNDIR"
  local ceiling_mib=""
  local leader_file=""
  RUNDIR="$(mktemp -d)"
  leader_file="$RUNDIR/leader"
  trap '_stop_pidfile child; rm -f "$RUNDIR/stale.pid" "$RUNDIR/leader"; rmdir "$RUNDIR" 2>/dev/null || true' EXIT
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
  printf '%s|invalid start time\n' "$$" > "$RUNDIR/stale.pid"
  _stop_pidfile stale
  [[ ! -e "$RUNDIR/stale.pid" ]]
  if FASTI_PORT=0 "$0" --status >/dev/null 2>&1; then
    echo "self-test accepted invalid FASTI_PORT" >&2
    return 1
  fi
  local status_output
  status_output="$(FASTI_LISTEN=127.0.0.1:18420 "$0" --status)"
  [[ "$status_output" == *"http://127.0.0.1:18420"* ]]
  status_output="$(FASTI_LISTEN=127.0.0.1:18420 FASTI_API_URL=http://localhost:18421 "$0" --status)"
  [[ "$status_output" == *"http://localhost:18421"* ]]
  if FASTI_API_URL='http://userinfo-marker@127.0.0.1:18421?query-marker' "$0" --status >/dev/null 2>&1; then
    echo "self-test accepted credentials or a query in FASTI_API_URL" >&2
    return 1
  fi
  if FASTI_API_URL=http://127.0.0.1:70000 "$0" --status >/dev/null 2>&1; then
    echo "self-test accepted an out-of-range FASTI_API_URL port" >&2
    return 1
  fi
  if FASTI_API_URL=http://127.0.0.1:not-a-port "$0" --status >/dev/null 2>&1; then
    echo "self-test accepted a nonnumeric FASTI_API_URL port" >&2
    return 1
  fi
  status_output="$(FASTI_API_URL=http://127.0.0.1:18421/ "$0" --status)"
  [[ "$status_output" == *"http://127.0.0.1:18421"* ]]
  if FASTI_CONTAINER_RUNTIME=podman FASTI_LISTEN=0.0.0.0:18420 FASTI_PORT=18420 _start_container >/dev/null 2>&1; then
    echo "self-test accepted an unsupported container listener" >&2
    return 1
  fi
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
  systemd-run() { return 0; }
  _configure_native_scope
  unset -f systemd-run
  ceiling_mib="$(_memory_ceiling_mib)"
  [[ "${NATIVE_SCOPE_RUNNER[*]}" == *"MemoryMax=$((ceiling_mib * 1024 * 1024))"* ]]
  [[ "${NATIVE_SCOPE_RUNNER[*]}" == *"MemorySwapMax=0"* ]]
  rm -f "$leader_file"
  rmdir "$RUNDIR"
  RUNDIR="$old_rundir"
  trap - EXIT
  echo "dev launcher self-test passed"
}

case "${1:-}" in
  --stop) _stop ;;
  --status) _status ;;
  --podman) FASTI_CONTAINER_RUNTIME=podman; _start_container ;;
  --docker) FASTI_CONTAINER_RUNTIME=docker; _start_container ;;
  --container) _start_container ;;
  --self-test) _self_test ;;
  *) _start_native ;;
esac
