#!/usr/bin/env bash
# Local dev-loop launcher: fastid (daemon) + apps/web (Vite), if present.
#
# apps/web only exists on worktrees checked out at a branch that carries it
# (today: codex/b4-durable-bootstrap). Worktrees come and go, so this script
# detects apps/web at runtime instead of hardcoding a worktree path.
#
# fastid's port is host-wide, not worktree-scoped: other things on this
# machine (a canary/monitoring container, another worktree's dev session)
# can legitimately hold 8420 at the same time this script wants it. Rather
# than fail outright, cmd_start walks forward to the next free port and says
# so -- see resolve_fastid_port. Pin one explicitly with FASTI_DEV_PORT.
#
# Usage: scripts/dev.sh [start|stop|status|logs|open|help]
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

log_dir=".dev-logs"
fastid_pid="$log_dir/fastid.pid"
fastid_log="$log_dir/fastid.log"
fastid_port_file="$log_dir/fastid.port"
web_pid="$log_dir/web.pid"
web_log="$log_dir/web.log"
base_fastid_port="${FASTI_DEV_PORT:-8420}"
web_url="http://127.0.0.1:5173"

has_setsid() {
  command -v setsid >/dev/null 2>&1
}

has_web() {
  [[ -f "apps/web/package.json" ]]
}

pid_alive() {
  local pid_file="$1"
  [[ -f "$pid_file" ]] && kill -0 "$(cat "$pid_file")" 2>/dev/null
}

port_in_use() {
  local port="$1"
  if command -v ss >/dev/null 2>&1; then
    ss -ltn "( sport = :$port )" 2>/dev/null | grep -q ":$port"
  else
    curl --silent --fail --max-time 1 "http://127.0.0.1:$port" >/dev/null 2>&1
  fi
}

# Names whatever's actually holding a port, so the user isn't left guessing
# "something else" -- a podman container's name if one is publishing it,
# otherwise the owning process from `ss -p`.
describe_port_owner() {
  local port="$1"
  local owner
  if command -v podman >/dev/null 2>&1; then
    owner="$(podman ps --format '{{.Names}} {{.Ports}}' 2>/dev/null \
      | awk -v needle=":${port}->" '$0 ~ needle {print $1; exit}')"
    if [[ -n "$owner" ]]; then
      echo "podman container '$owner'"
      return
    fi
  fi
  if command -v ss >/dev/null 2>&1; then
    owner="$(ss -ltnp "( sport = :$port )" 2>/dev/null \
      | sed -n 's/.*users:(("\([^"]*\)".*/\1/p' | head -1)"
    if [[ -n "$owner" ]]; then
      echo "process '$owner'"
      return
    fi
  fi
  echo "an unidentified process"
}

# Walks forward from base_fastid_port until it finds a free one, printing
# what it skipped past along the way. Gives up after 5 tries.
resolve_fastid_port() {
  local port="$base_fastid_port"
  for _ in 1 2 3 4 5; do
    if ! port_in_use "$port"; then
      echo "$port"
      return 0
    fi
    echo "Port $port is already in use by $(describe_port_owner "$port"). Trying $((port + 1))... (pin one with FASTI_DEV_PORT)" >&2
    port=$((port + 1))
  done
  echo "Could not find a free port for fastid in ${base_fastid_port}..$port. Free one of them or set FASTI_DEV_PORT." >&2
  return 1
}

current_fastid_port() {
  if [[ -f "$fastid_port_file" ]]; then
    cat "$fastid_port_file"
  else
    echo "$base_fastid_port"
  fi
}

fastid_health_url() {
  echo "http://127.0.0.1:$(current_fastid_port)/api/v1/health"
}

no_web_message() {
  local branch
  branch="$(git branch --show-current 2>/dev/null || echo "detached HEAD")"
  cat >&2 <<EOF
apps/web not found on this worktree (branch: $branch).
The web workbench lives on whichever worktree/branch carries apps/web
(check with: git worktree list, then look for apps/web in each).
Starting fastid only.
EOF
}

cmd_start() {
  mkdir -p "$log_dir"

  if pid_alive "$fastid_pid"; then
    echo "fastid already running (pid $(cat "$fastid_pid"), port $(current_fastid_port))"
  else
    local port
    port="$(resolve_fastid_port)" || exit 1
    echo "$port" >"$fastid_port_file"
    echo "Starting fastid on port $port..."
    # setsid makes the tracked pid its own process group leader, so cmd_stop's
    # `kill -- -$pid` reaches cargo's actual fastid child too, not just the
    # cargo wrapper. Not every platform has setsid (e.g. macOS without
    # coreutils); cmd_stop falls back to a direct kill there.
    if has_setsid; then
      FASTI_LISTEN="127.0.0.1:$port" setsid cargo run --locked -p fastid >"$fastid_log" 2>&1 &
    else
      FASTI_LISTEN="127.0.0.1:$port" cargo run --locked -p fastid >"$fastid_log" 2>&1 &
    fi
    echo $! >"$fastid_pid"

    local health_url
    health_url="$(fastid_health_url)"
    for _ in $(seq 1 30); do
      curl --silent --fail "$health_url" >/dev/null 2>&1 && break
      sleep 1
    done
    if curl --silent --fail "$health_url" >/dev/null 2>&1; then
      echo "fastid ready: $health_url"
    else
      echo "fastid did not become healthy within 30s; check $fastid_log" >&2
    fi
  fi

  if ! has_web; then
    no_web_message
    return
  fi

  if pid_alive "$web_pid"; then
    echo "web already running (pid $(cat "$web_pid"))"
  else
    if port_in_use 5173; then
      echo "Port 5173 is already in use by $(describe_port_owner 5173). Stop it first, or run 'scripts/dev.sh stop'." >&2
      exit 1
    fi
    # apps/web imports @fasti/tokens and @fasti/sdk as built workspace
    # packages, not raw TS sources -- a fresh worktree/checkout won't have
    # their dist/ output yet, and vite fails to resolve them without it.
    echo "Building @fasti/tokens and @fasti/sdk (apps/web dependencies)..."
    pnpm run build >"$web_log" 2>&1

    echo "Starting apps/web (Vite)..."
    # FASTI_QA_PROXY_TARGET: apps/web's vite.config.ts hardcodes its /api proxy
    # to 127.0.0.1:8420 unless this is set. If fastid landed on a fallback port
    # (see resolve_fastid_port), the web UI's API calls need to follow it there.
    local proxy_target
    proxy_target="$(fastid_health_url | sed 's#/api/v1/health##')"
    if has_setsid; then
      FASTI_QA_PROXY_TARGET="$proxy_target" setsid pnpm --filter @fasti/web run dev >>"$web_log" 2>&1 &
    else
      FASTI_QA_PROXY_TARGET="$proxy_target" pnpm --filter @fasti/web run dev >>"$web_log" 2>&1 &
    fi
    echo $! >"$web_pid"
    echo "web starting: $web_url (see $web_log for Vite's own ready message)"
  fi
}

cmd_stop() {
  for pid_file in "$fastid_pid" "$web_pid"; do
    if [[ -f "$pid_file" ]]; then
      local pid
      pid="$(cat "$pid_file")"
      # Negative pid = kill the whole process group, reaching cargo/pnpm's
      # actual child too (see has_setsid above). Falls back to a direct kill
      # for platforms without setsid, where $pid is not a process-group leader.
      if kill -- "-$pid" 2>/dev/null || kill "$pid" 2>/dev/null; then
        echo "Stopped $pid_file (pid $pid)"
      else
        echo "$pid_file already stopped"
      fi
      rm -f "$pid_file"
    fi
  done
  rm -f "$fastid_port_file"
  if command -v podman >/dev/null 2>&1 && podman container exists fasti-devloop 2>/dev/null; then
    podman stop fasti-devloop >/dev/null 2>&1 && echo "Stopped podman container fasti-devloop"
  fi
}

cmd_status() {
  local port
  port="$(current_fastid_port)"
  if [[ -d "$log_dir" ]] && pid_alive "$fastid_pid"; then
    echo "fastid: running (pid $(cat "$fastid_pid"), port $port)"
  elif port_in_use "$port"; then
    echo "fastid: not running via this script, but port $port is held by $(describe_port_owner "$port")"
  else
    echo "fastid: not running"
  fi
  if [[ -d "$log_dir" ]] && pid_alive "$web_pid"; then
    echo "web: running (pid $(cat "$web_pid"))"
  else
    echo "web: not running"
  fi
}

cmd_logs() {
  if [[ ! -d "$log_dir" ]]; then
    echo "No logs yet -- nothing has been started in this worktree."
    return
  fi
  tail -F "$fastid_log" "$web_log" 2>/dev/null
}

cmd_open() {
  local target
  target="$(fastid_health_url)"
  if pid_alive "$web_pid"; then
    target="$web_url"
  else
    # fastid is an API daemon with no home page of its own (see README.md
    # Current status: "Web UI: Not implemented"). Say so explicitly instead
    # of silently opening a JSON health check and leaving "where's the UI?"
    # unanswered.
    if has_web; then
      echo "web isn't running in this worktree yet -- run 'scripts/dev.sh start' first. Opening the health check instead." >&2
    else
      no_web_message
      echo "fastid has no home page of its own -- opening its health check instead." >&2
    fi
  fi
  if command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$target" >/dev/null 2>&1 &
  else
    echo "$target"
  fi
}

# Runs the release container image (see docs/dev-loop.md), not the native
# dev loop above. Build the image first: podman build --tag fasti:b0 .
# Named fasti-devloop, not fasti-dev, to stay clear of whatever /canary
# monitoring creates on this host (observed: fasti-dev + fasti-canary
# containers both publishing to fastid's default port).
cmd_podman() {
  local image="${FASTI_DEV_IMAGE:-fasti:b0}"
  local data_dir="$log_dir/podman-data"
  mkdir -p "$data_dir"
  echo "Starting $image in podman (container name: fasti-devloop)..."
  podman run -d --name fasti-devloop --rm \
    --publish "127.0.0.1:${base_fastid_port}:8420" \
    -v "$data_dir:/data:Z" \
    -e FASTI_DATA_ROOT=/data \
    "$image" 2>/dev/null || podman restart fasti-devloop

  local health_url="http://127.0.0.1:${base_fastid_port}/api/v1/health"
  for _ in $(seq 1 30); do
    curl --silent --fail "$health_url" >/dev/null 2>&1 && break
    sleep 1
  done
  if curl --silent --fail "$health_url" >/dev/null 2>&1; then
    echo "fasti-devloop ready: $health_url"
  else
    echo "fasti-devloop did not become healthy within 30s; check: podman logs fasti-devloop" >&2
  fi
}

cmd_desktop() {
  if [[ ! -d "apps/desktop/src-tauri" ]]; then
    echo "apps/desktop/src-tauri not found. Desktop packaging is not implemented yet (see README.md Current status: Desktop packaging owned by B8)." >&2
    exit 1
  fi
  (cd apps/desktop/src-tauri && cargo run)
}

cmd_help() {
  cat <<EOF
Usage: scripts/dev.sh [start|stop|status|logs|open|podman|desktop|help]

  start    Start fastid (and apps/web, if present in this worktree). Default.
           If fastid's port is taken by something else, tries the next one
           and says so. Pin a port with FASTI_DEV_PORT=<port>.
  stop     Stop whatever this script started.
  status   Show what's running, and what's holding fastid's port if this
           script isn't the one holding it.
  logs     Tail the daemon/web log files.
  open     Open the running web UI, or the health endpoint if web isn't up.
  podman   Run the release container image (podman run, not the dev loop).
  desktop  Run the Tauri desktop shell, if this worktree has one.
  help     Show this message.
EOF
}

case "${1:-start}" in
  start) cmd_start ;;
  stop) cmd_stop ;;
  status) cmd_status ;;
  logs) cmd_logs ;;
  open) cmd_open ;;
  podman) cmd_podman ;;
  desktop) cmd_desktop ;;
  help|-h|--help) cmd_help ;;
  *) echo "Unknown command: $1" >&2; cmd_help; exit 1 ;;
esac
