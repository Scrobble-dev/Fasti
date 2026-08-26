#!/usr/bin/env bash
# Floppy local dev — starts all services and opens the browser.
#
# Usage:
#   ./scripts/dev.sh          # start everything
#   ./scripts/dev.sh --stop   # kill all Floppy dev processes
#   ./scripts/dev.sh --status # show what's running
#
# Prerequisites:
#   - Redis running on localhost:6379 (docker run -d --name redis -p 6379:6379 --restart unless-stopped redis:8-alpine)
#   - uv installed (brew install uv)
#   - .env with at least SECRET=<something> and DEBUG=True
#   - Dependencies installed (uv sync --locked)

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOGDIR="$PROJECT_ROOT/.dev-logs"

_status() {
  echo "=== Floppy dev processes ==="
  pgrep -af "manage.py runserver"       2>/dev/null && echo "" || echo "  Django:             not running"
  pgrep -af "celery -A config.*interactive" 2>/dev/null && echo "" || echo "  Celery interactive: not running"
  pgrep -af "celery -A config.*celery.*beat" 2>/dev/null && echo "" || echo "  Celery background:  not running"
  pgrep -af "tailwindcss.*input.css"    2>/dev/null && echo "" || echo "  Tailwind:           not running"
  echo ""
  redis-cli ping 2>/dev/null && echo "  Redis:              PONG" || echo "  Redis:              not reachable"
}

_stop() {
  echo "Stopping Floppy dev processes..."
  pkill -f "manage.py runserver"              2>/dev/null || true
  pkill -f "celery -A config"                 2>/dev/null || true
  pkill -f "tailwindcss.*input.css"           2>/dev/null || true
  sleep 0.5
  echo "Done."
}

_start() {
  # Ensure log directory
  mkdir -p "$LOGDIR"

  # Check Redis
  if ! redis-cli ping &>/dev/null; then
    echo "Redis not running. Starting via Docker..."
    docker run -d --name redis -p 6379:6379 --restart unless-stopped redis:8-alpine 2>/dev/null \
      || docker start redis 2>/dev/null \
      || { echo "ERROR: Cannot start Redis. Start it manually."; exit 1; }
    sleep 1
  fi

  # Sync deps
  echo "Syncing dependencies..."
  uv sync --locked 2>&1 | tail -5

  # Migrate
  echo "Running migrations..."
  uv run --no-sync python "$PROJECT_ROOT/src/manage.py" migrate 2>&1 | grep -E "(Applying|No migrations)" | head -20
  echo ""

  # Kill any existing Floppy processes
  _stop 2>/dev/null || true

  # Start Celery interactive worker
  echo "Starting Celery interactive worker..."
  PYTHONPATH="$PROJECT_ROOT/src" uv run --no-sync celery -A config worker \
    --queues interactive --hostname celery-interactive@%h --loglevel INFO \
    > "$LOGDIR/celery-interactive.log" 2>&1 &

  # Start Celery background worker + beat
  echo "Starting Celery background worker + beat..."
  PYTHONPATH="$PROJECT_ROOT/src" uv run --no-sync celery -A config worker \
    --queues celery --beat --scheduler django --hostname celery@%h --loglevel INFO \
    > "$LOGDIR/celery-background.log" 2>&1 &

  # Start Tailwind watcher
  echo "Starting Tailwind watcher..."
  npx @tailwindcss/cli -i "$PROJECT_ROOT/src/static/css/input.css" \
    -o "$PROJECT_ROOT/src/static/css/main.css" --watch \
    > "$LOGDIR/tailwind.log" 2>&1 &

  # Wait a beat for workers to init
  sleep 2

  # Start Django dev server
  echo "Starting Django dev server..."
  uv run --no-sync python "$PROJECT_ROOT/src/manage.py" runserver \
    > "$LOGDIR/django.log" 2>&1 &

  sleep 2

  # Open browser
  echo "Opening http://localhost:8000 ..."
  xdg-open http://localhost:8000 2>/dev/null || open http://localhost:8000 2>/dev/null || true

  echo ""
  echo "=== Floppy dev environment running ==="
  echo "  Django:   http://localhost:8000  (log: $LOGDIR/django.log)"
  echo "  Celery:   2 workers             (logs: $LOGDIR/celery-*.log)"
  echo "  Tailwind: watching              (log: $LOGDIR/tailwind.log)"
  echo "  Redis:    localhost:6379"
  echo ""
  echo "  Demo login: demo / demodemo"
  echo "  Stop all:   ./scripts/dev.sh --stop"
  echo "  Status:     ./scripts/dev.sh --status"
  echo "  Tail logs:  tail -f $LOGDIR/*.log"
}

case "${1:-start}" in
  --stop|stop)   _stop ;;
  --status|status) _status ;;
  *)             _start ;;
esac
