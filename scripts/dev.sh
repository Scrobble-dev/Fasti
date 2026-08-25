#!/usr/bin/env bash
# Fasti Local Development Launcher
#
# Usage:
#   ./scripts/dev.sh             # Start the native daemon
#   ./scripts/dev.sh --podman    # Start Fasti in a scoped Podman container
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
FASTI_API_URL="${FASTI_API_URL:-http://127.0.0.1:$FASTI_PORT}"
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

_wait_for_health() {
  local pid="${1:-}"
  for _ in {1..10}; do
    [[ -z "$pid" ]] || kill -0 "$pid" 2>/dev/null || return 1
    curl --connect-timeout 2 --max-time 5 --silent --fail "$FASTI_API_URL/api/v1/health" >/dev/null 2>&1 && return 0
    sleep 0.5
  done
  return 1
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
    wait "$pid" 2>/dev/null || true
    for _ in {1..10}; do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.1
    done
    if kill -0 "$pid" 2>/dev/null; then
      kill -KILL -- "-$pid" 2>/dev/null || kill -KILL "$pid" 2>/dev/null || true
    fi
  fi
  rm -f "$RUNDIR/$name.pid"
}

_stop_processes() {
  _stop_pidfile daemon
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

_status() {
  echo "=== Fasti Dev Status ($DEV_SCOPE) ==="
  _status_line "Daemon (fastid)" daemon
  echo ""
  if curl --connect-timeout 2 --max-time 5 --silent --fail "$FASTI_API_URL/api/v1/health" >/dev/null 2>&1; then
    echo "  API Probe: HEALTHY ($(curl --connect-timeout 2 --max-time 5 -s "$FASTI_API_URL/api/v1/health"))"
  else
    echo "  API Probe: NOT REACHABLE ($FASTI_API_URL)"
  fi
}

_stop() {
  echo "Stopping Fasti dev scope $DEV_SCOPE..."
  _stop_processes
  podman stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
  echo "Stopped Fasti dev scope $DEV_SCOPE."
}

_start_podman() {
  echo "=== Launching Fasti Podman Container ($CONTAINER_NAME) ==="
  mkdir -p "$DATADIR"
  if ! podman run -d --name "$CONTAINER_NAME" --rm \
    --publish "127.0.0.1:$FASTI_PORT:8420" \
    -v "$DATADIR:/data:Z" \
    -e FASTI_DATA_ROOT=/data \
    localhost/fasti:test; then
    echo "Failed to start Podman container $CONTAINER_NAME" >&2
    return 1
  fi

  if _wait_for_health; then
    echo "Fasti Podman container is healthy on $FASTI_API_URL"
  else
    echo "Fasti Podman container failed its health probe; run: podman logs $CONTAINER_NAME" >&2
    podman stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
    return 1
  fi
}

_start_native() {
  trap _cleanup EXIT
  trap '_cleanup; exit 130' INT
  trap '_cleanup; exit 143' TERM
  set -m
  mkdir -p "$LOGDIR" "$DATADIR" "$RUNDIR"

  echo "=== 1. Compiling and starting Fasti daemon ==="
  cargo build --locked --bin fastid
  export FASTI_LISTEN FASTI_API_URL
  export FASTI_DATA_ROOT="$DATADIR"

  "$PROJECT_ROOT/target/debug/fastid" > "$LOGDIR/fastid.log" 2>&1 &
  local daemon_pid=$!
  _write_pidfile daemon "$daemon_pid"
  echo "Fasti daemon started (PID: $daemon_pid, log: .dev-logs/fastid.log)"

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
  RUNDIR="$(mktemp -d)"
  trap '_stop_pidfile child; rm -f "$RUNDIR/stale.pid"; rmdir "$RUNDIR" 2>/dev/null || true' EXIT
  set -m
  bash -c 'sleep 30 & wait' &
  local leader=$!
  _write_pidfile child "$leader"
  [[ "$(_tracked_pid child)" == "$leader" ]]
  _stop_pidfile child
  ! kill -0 "$leader" 2>/dev/null
  printf '%s|invalid start time\n' "$$" > "$RUNDIR/stale.pid"
  _stop_pidfile stale
  [[ ! -e "$RUNDIR/stale.pid" ]]
  ! FASTI_PORT=0 "$0" --status >/dev/null 2>&1
  rmdir "$RUNDIR"
  RUNDIR="$old_rundir"
  trap - EXIT
  echo "dev launcher self-test passed"
}

case "${1:-}" in
  --stop) _stop ;;
  --status) _status ;;
  --podman|--container) _start_podman ;;
  --self-test) _self_test ;;
  *) _start_native ;;
esac
