#!/usr/bin/env bash
# Fasti Local Development Launcher
#
# Usage:
#   ./scripts/dev.sh           # Start daemon + web frontend
#   ./scripts/dev.sh --podman  # Start Fasti in Podman container
#   ./scripts/dev.sh --desktop # Launch Tauri desktop app
#   ./scripts/dev.sh --status  # Check running processes and health
#   ./scripts/dev.sh --stop    # Stop all Fasti dev processes
#
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOGDIR="$PROJECT_ROOT/.dev-logs"
DATADIR="$PROJECT_ROOT/.dev-data"
DAEMON_PID_FILE="$LOGDIR/fastid.pid"
WEB_PID_FILE="$LOGDIR/web.pid"

_tracked_pid() {
  local pid_file="$1"
  local marker="$2"
  local pid

  [[ -f "$pid_file" ]] || return 1
  read -r pid < "$pid_file"
  if [[ ! "$pid" =~ ^[0-9]+$ ]] || ((pid <= 1)) || ! kill -0 "$pid" 2>/dev/null; then
    rm -f "$pid_file"
    return 1
  fi
  if ! ps -p "$pid" -o args= 2>/dev/null | grep -Eq -- "$marker"; then
    rm -f "$pid_file"
    return 1
  fi
  printf '%s' "$pid"
}

_stop_pid_file() {
  local pid_file="$1"
  local marker="$2"
  local label="$3"
  local pid
  local pgid

  if pid=$(_tracked_pid "$pid_file" "$marker"); then
    pgid=$(ps -p "$pid" -o pgid= 2>/dev/null | tr -d ' ')
    if [[ "$pgid" == "$pid" ]]; then
      kill -- "-$pid" 2>/dev/null || true
    else
      kill "$pid" 2>/dev/null || true
    fi
    echo "Stopped $label (PID: $pid)"
  fi
  rm -f "$pid_file"
}

_cleanup_native() {
  trap - EXIT INT TERM
  _stop_pid_file "$WEB_PID_FILE" 'vite|pnpm' "Web Shell"
  _stop_pid_file "$DAEMON_PID_FILE" 'target/debug/fastid' "Daemon"
}

_port_in_use() {
  local port="$1"
  if command -v ss >/dev/null 2>&1; then
    ss -ltn "sport = :$port" 2>/dev/null | grep -q ":$port"
  elif command -v lsof >/dev/null 2>&1; then
    lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1
  elif command -v nc >/dev/null 2>&1; then
    nc -z 127.0.0.1 "$port" >/dev/null 2>&1
  else
    echo "Cannot inspect TCP port $port: install ss, lsof, or nc." >&2
    return 0
  fi
}

_status() {
  echo "=== Fasti Dev Status ==="
  local daemon_pid
  local web_pid
  daemon_pid=$(_tracked_pid "$DAEMON_PID_FILE" 'target/debug/fastid' || true)
  web_pid=$(_tracked_pid "$WEB_PID_FILE" 'vite|pnpm' || true)

  if [[ -n "$daemon_pid" ]]; then
    echo "  Daemon (fastid):  RUNNING (PID: $daemon_pid)"
  else
    echo "  Daemon (fastid):  NOT RUNNING"
  fi

  if [[ -n "$web_pid" ]]; then
    echo "  Web Shell (Vite): RUNNING (PID: $web_pid)"
  else
    echo "  Web Shell (Vite): NOT RUNNING"
  fi

  echo ""
  if curl --silent --fail http://127.0.0.1:8420/api/v1/health >/dev/null 2>&1; then
    echo "  API Probe (8420): HEALTHY ($(curl -s http://127.0.0.1:8420/api/v1/health))"
  elif curl --silent --fail http://127.0.0.1:4000/api/v1/health >/dev/null 2>&1; then
    echo "  API Probe (4000): HEALTHY ($(curl -s http://127.0.0.1:4000/api/v1/health))"
  else
    echo "  API Probe:        NOT REACHABLE"
  fi
}

_stop() {
  echo "Stopping Fasti dev processes..."
  _stop_pid_file "$WEB_PID_FILE" 'vite|pnpm' "Web Shell"
  _stop_pid_file "$DAEMON_PID_FILE" 'target/debug/fastid' "Daemon"
  podman stop fasti-dev 2>/dev/null || true
  echo "Tracked Fasti dev processes stopped."
}

_start_podman() {
  echo "=== Launching Fasti Podman Container ==="
  mkdir -p "$DATADIR"
  podman run -d --name fasti-dev --rm \
    --publish 8420:8420 \
    -v "$DATADIR:/data:Z" \
    -e FASTI_DATA_ROOT=/data \
    localhost/fasti:test 2>/dev/null || podman restart fasti-dev 2>/dev/null || true

  sleep 1
  echo "Fasti Podman container running on http://127.0.0.1:8420"
  echo "API Health: $(curl -s http://127.0.0.1:8420/api/v1/health || echo 'starting...')"
}

_start_desktop() {
  echo "=== Launching Fasti Desktop (Tauri v2) ==="
  cd "$PROJECT_ROOT/apps/desktop/src-tauri"
  PKG_CONFIG=/usr/bin/pkg-config cargo run
}

_start_native() {
  mkdir -p "$LOGDIR" "$DATADIR"

  if _tracked_pid "$DAEMON_PID_FILE" 'target/debug/fastid' >/dev/null; then
    echo "Fasti daemon is already running for this worktree." >&2
    exit 1
  fi
  if _tracked_pid "$WEB_PID_FILE" 'vite|pnpm' >/dev/null; then
    echo "Fasti web shell is already running for this worktree." >&2
    exit 1
  fi
  if _port_in_use 8420; then
    echo "Port 8420 is already in use by a process this worktree did not start." >&2
    exit 1
  fi
  if _port_in_use 5173; then
    echo "Port 5173 is already in use by a process this worktree did not start." >&2
    exit 1
  fi

  trap _cleanup_native EXIT INT TERM

  echo "=== 1. Compiling & Starting Fasti Daemon ==="
  cargo build --locked --bin fastid
  export FASTI_LISTEN=127.0.0.1:8420
  export FASTI_DATA_ROOT="$DATADIR"
  
  setsid "$PROJECT_ROOT/target/debug/fastid" > "$LOGDIR/fastid.log" 2>&1 &
  local daemon_pid=$!
  echo "$daemon_pid" > "$DAEMON_PID_FILE"
  echo "Fasti daemon started (PID: $daemon_pid, log: .dev-logs/fastid.log)"

  echo "Waiting for daemon health probe..."
  for _ in $(seq 1 10); do
    if ! kill -0 "$daemon_pid" 2>/dev/null; then
      echo "Fasti daemon exited during startup; check .dev-logs/fastid.log" >&2
      exit 1
    fi
    if curl --silent --fail http://127.0.0.1:8420/api/v1/health >/dev/null 2>&1; then
      break
    fi
    sleep 0.5
  done

  if curl --silent --fail http://127.0.0.1:8420/api/v1/health >/dev/null 2>&1; then
    echo "✓ Fasti daemon is healthy on http://127.0.0.1:8420"
  else
    echo "⚠️ Daemon did not respond in time, check .dev-logs/fastid.log"
  fi

  echo ""
  echo "=== 2. Starting Fasti Web Workbench (Vite + Svelte 5) ==="
  if [[ -d "$PROJECT_ROOT/apps/web" ]]; then
    cd "$PROJECT_ROOT/apps/web"
    setsid pnpm run dev --host 127.0.0.1 --port 5173 > "$LOGDIR/vite.log" 2>&1 &
    local web_pid=$!
    echo "$web_pid" > "$WEB_PID_FILE"
    echo "Web Workbench started (PID: $web_pid, log: .dev-logs/vite.log)"
    echo ""
    echo "┌─────────────────────────────────────────────────────────────┐"
    echo "│ Fasti Workbench is live!                                    │"
    echo "│                                                             │"
    echo "│ • Web Interface:  http://127.0.0.1:5173                     │"
    echo "│ • Local Daemon:   http://127.0.0.1:8420                     │"
    echo "│ • Health Probe:   http://127.0.0.1:8420/api/v1/health       │"
    echo "│ • Data Directory: $DATADIR                                  │"
    echo "└─────────────────────────────────────────────────────────────┘"
    echo ""
    echo "Press Ctrl+C or run ./scripts/dev.sh --stop to shutdown."
    wait "$web_pid"
  else
    echo "apps/web not found in current directory."
    wait "$daemon_pid"
  fi
}

case "${1:-}" in
  --stop)
    _stop
    ;;
  --status)
    _status
    ;;
  --podman|--container)
    _start_podman
    ;;
  --desktop)
    _start_desktop
    ;;
  *)
    _start_native
    ;;
esac
