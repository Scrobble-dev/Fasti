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

# _status reports Fasti daemon and web process status and probes API health on ports 8420 and 4000.
_status() {
  echo "=== Fasti Dev Status ==="
  local daemon_pid
  local web_pid
  daemon_pid=$(pgrep -f "target/.*/fastid" 2>/dev/null || true)
  web_pid=$(pgrep -f "vite.*apps/web" 2>/dev/null || true)

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
  if curl --connect-timeout 2 --max-time 5 --silent --fail http://127.0.0.1:8420/api/v1/health >/dev/null 2>&1; then
    echo "  API Probe (8420): HEALTHY ($(curl --connect-timeout 2 --max-time 5 -s http://127.0.0.1:8420/api/v1/health))"
  elif curl --connect-timeout 2 --max-time 5 --silent --fail http://127.0.0.1:4000/api/v1/health >/dev/null 2>&1; then
    echo "  API Probe (4000): HEALTHY ($(curl --connect-timeout 2 --max-time 5 -s http://127.0.0.1:4000/api/v1/health))"
  else
    echo "  API Probe:        NOT REACHABLE"
  fi
}

# _stop stops Fasti development processes and the `fasti-dev` Podman container.
_stop() {
  echo "Stopping Fasti dev processes..."
  pkill -f "target/.*/fastid" 2>/dev/null || true
  pkill -f "vite.*apps/web" 2>/dev/null || true
  podman stop fasti-dev 2>/dev/null || true
  sleep 0.5
  echo "All Fasti dev processes stopped."
}

# _start_podman launches the Fasti development container with persistent data and reports its API health.
_start_podman() {
  echo "=== Launching Fasti Podman Container ==="
  mkdir -p "$DATADIR"
  if ! podman run -d --name fasti-dev --rm \
    --publish 8420:8420 \
    -v "$DATADIR:/data:Z" \
    -e FASTI_DATA_ROOT=/data \
    localhost/fasti:test 2>/dev/null; then
    if ! podman restart fasti-dev 2>/dev/null; then
      echo "Failed to start or restart Podman container fasti-dev"
      return 1
    fi
  fi

  sleep 1
  echo "Fasti Podman container running on http://127.0.0.1:8420"
  echo "API Health: $(curl --connect-timeout 2 --max-time 5 -s http://127.0.0.1:8420/api/v1/health || echo 'starting...')"
}

# _start_desktop launches the Fasti Tauri desktop application from its source directory.
_start_desktop() {
  echo "=== Launching Fasti Desktop (Tauri v2) ==="
  cd "$PROJECT_ROOT/apps/desktop/src-tauri"
  PKG_CONFIG=/usr/bin/pkg-config cargo run
}

# _start_native builds and starts the local Fasti daemon and, when available, the web workbench.
_start_native() {
  trap _stop EXIT INT TERM
  mkdir -p "$LOGDIR" "$DATADIR"

  echo "=== 1. Compiling & Starting Fasti Daemon ==="
  cargo build --locked --bin fastid
  export FASTI_LISTEN=127.0.0.1:8420
  export FASTI_DATA_ROOT="$DATADIR"
  
  "$PROJECT_ROOT/target/debug/fastid" > "$LOGDIR/fastid.log" 2>&1 &
  local daemon_pid=$!
  echo "Fasti daemon started (PID: $daemon_pid, log: .dev-logs/fastid.log)"

  echo "Waiting for daemon health probe..."
  for _ in $(seq 1 10); do
    if curl --connect-timeout 2 --max-time 5 --silent --fail http://127.0.0.1:8420/api/v1/health >/dev/null 2>&1; then
      break
    fi
    sleep 0.5
  done

  if curl --connect-timeout 2 --max-time 5 --silent --fail http://127.0.0.1:8420/api/v1/health >/dev/null 2>&1; then
    echo "✓ Fasti daemon is healthy on http://127.0.0.1:8420"
  else
    echo "⚠️ Daemon did not respond in time, check .dev-logs/fastid.log"
  fi

  echo ""
  echo "=== 2. Starting Fasti Web Workbench (Vite + Svelte 5) ==="
  if [[ -d "$PROJECT_ROOT/apps/web" ]]; then
    cd "$PROJECT_ROOT/apps/web"
    pnpm run dev --host 127.0.0.1 --port 5173 > "$LOGDIR/vite.log" 2>&1 &
    local web_pid=$!
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
