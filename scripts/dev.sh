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
FASTI_PORT="${FASTI_PORT:-8420}"
FASTI_WEB_PORT="${FASTI_WEB_PORT:-5173}"
FASTI_LISTEN="${FASTI_LISTEN:-127.0.0.1:${FASTI_PORT}}"
FASTI_API_URL="${FASTI_API_URL:-http://127.0.0.1:${FASTI_PORT}}"
FASTI_DEV_NAME="${FASTI_DEV_NAME:-fasti-dev-$(basename "$PROJECT_ROOT" | tr -cd '[:alnum:]_.-')}"

_pid_running() {
  local pid_file="$1"
  [[ -f "$pid_file" ]] && kill -0 "$(<"$pid_file")" 2>/dev/null
}

_wait_for_health() {
  for _ in {1..10}; do
    curl --silent --fail "$FASTI_API_URL/api/v1/health" >/dev/null 2>&1 && return 0
    sleep 0.5
  done
  return 1
}

_status() {
  echo "=== Fasti Dev Status ==="
  if _pid_running "$LOGDIR/fastid.pid"; then
    echo "  Daemon (fastid):  RUNNING (PID: $(<"$LOGDIR/fastid.pid"))"
  else
    echo "  Daemon (fastid):  NOT RUNNING"
  fi

  if _pid_running "$LOGDIR/vite.pid"; then
    echo "  Web Shell (Vite): RUNNING (PID: $(<"$LOGDIR/vite.pid"))"
  else
    echo "  Web Shell (Vite): NOT RUNNING"
  fi

  echo ""
  if curl --silent --fail "$FASTI_API_URL/api/v1/health" >/dev/null 2>&1; then
    echo "  API Probe:        HEALTHY ($(curl -s "$FASTI_API_URL/api/v1/health"))"
  else
    echo "  API Probe:        NOT REACHABLE ($FASTI_API_URL)"
  fi
}

_stop() {
  echo "Stopping this worktree's Fasti dev processes..."
  for pid_file in "$LOGDIR/fastid.pid" "$LOGDIR/vite.pid"; do
    if _pid_running "$pid_file"; then
      kill "$(<"$pid_file")"
    fi
  done
  rm -f "$LOGDIR/fastid.pid" "$LOGDIR/vite.pid"
  podman stop "$FASTI_DEV_NAME" 2>/dev/null || true
  echo "This worktree's Fasti dev processes stopped."
}

_start_podman() {
  echo "=== Launching Fasti Podman Container ==="
  mkdir -p "$DATADIR"
  if podman container exists "$FASTI_DEV_NAME"; then
    podman start "$FASTI_DEV_NAME" >/dev/null
  else
    podman run -d --name "$FASTI_DEV_NAME" --rm \
      --publish "127.0.0.1:${FASTI_PORT}:8420" \
      -v "$DATADIR:/data:Z" \
      -e FASTI_DATA_ROOT=/data \
      localhost/fasti:test >/dev/null
  fi

  if _wait_for_health; then
    echo "Fasti Podman container is healthy on $FASTI_API_URL"
  else
    echo "Fasti Podman container did not become healthy. Check podman logs $FASTI_DEV_NAME."
    return 1
  fi
}

_start_desktop() {
  echo "=== Launching Fasti Desktop (Tauri v2) ==="
  cd "$PROJECT_ROOT/apps/desktop/src-tauri"
  PKG_CONFIG=/usr/bin/pkg-config cargo run
}

_start_native() {
  mkdir -p "$LOGDIR" "$DATADIR"
  if _pid_running "$LOGDIR/fastid.pid" || _pid_running "$LOGDIR/vite.pid"; then
    echo "This worktree already has Fasti development processes. Run ./scripts/dev.sh --stop first."
    _status
    return 1
  fi

  echo "=== 1. Compiling & Starting Fasti Daemon ==="
  cargo build --locked --bin fastid
  export FASTI_LISTEN FASTI_API_URL FASTI_WEB_PORT
  export FASTI_DATA_ROOT="$DATADIR"
  
  "$PROJECT_ROOT/target/debug/fastid" > "$LOGDIR/fastid.log" 2>&1 &
  local daemon_pid=$!
  printf '%s\n' "$daemon_pid" > "$LOGDIR/fastid.pid"
  trap _stop EXIT
  trap '_stop; trap - EXIT; exit 130' INT TERM
  echo "Fasti daemon started (PID: $daemon_pid, log: .dev-logs/fastid.log)"

  echo "Waiting for daemon health probe..."
  if _wait_for_health; then
    echo "Fasti daemon is healthy on $FASTI_API_URL"
  else
    echo "Daemon did not respond in time. Check .dev-logs/fastid.log."
    return 1
  fi

  echo ""
  echo "=== 2. Starting Fasti Web Workbench (Vite + Svelte 5) ==="
  if [[ -d "$PROJECT_ROOT/apps/web" ]]; then
    cd "$PROJECT_ROOT/apps/web"
    pnpm run dev --host 127.0.0.1 --port "$FASTI_WEB_PORT" > "$LOGDIR/vite.log" 2>&1 &
    local web_pid=$!
    printf '%s\n' "$web_pid" > "$LOGDIR/vite.pid"
    echo "Web Workbench started (PID: $web_pid, log: .dev-logs/vite.log)"
    echo ""
    echo "┌─────────────────────────────────────────────────────────────┐"
    echo "│ Fasti Workbench is live!                                    │"
    echo "│                                                             │"
    echo "│ Web Interface:  http://127.0.0.1:$FASTI_WEB_PORT"
    echo "│ Local Daemon:   $FASTI_API_URL"
    echo "│ Health Probe:   $FASTI_API_URL/api/v1/health"
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
