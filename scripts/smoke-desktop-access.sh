#!/usr/bin/env bash
# Prove the Linux release desktop serves embedded C1 routes and starts Wry processes.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$root/apps/desktop/src-tauri/Cargo.toml"
release_binary="$root/apps/desktop/src-tauri/target/release/fasti-desktop"

source_status="$(git -C "$root" status --porcelain=v1 --untracked-files=all)"
[[ -z "$source_status" ]] || {
  echo "C1 desktop release-host smoke requires a clean Git worktree" >&2
  exit 1
}
git_commit="$(git -C "$root" rev-parse HEAD)"
git_tree="$(git -C "$root" rev-parse HEAD^{tree})"

for command in cargo curl git ip jq pnpm ps sha256sum uname unshare; do
  command -v "$command" >/dev/null || {
    echo "Missing release-host smoke prerequisite: $command" >&2
    exit 1
  }
done

[[ "$(uname -s)" == Linux ]] || {
  echo "C1 desktop release-host smoke is Linux-only" >&2
  exit 1
}
architecture="$(uname -m)"

if [[ -z "${WAYLAND_DISPLAY:-}" && -z "${DISPLAY:-}" ]]; then
  echo "C1 desktop release-host smoke requires an active local Wayland or X11 session" >&2
  exit 1
fi
if ! unshare --user --map-root-user --net -- true 2>/dev/null; then
  echo "C1 desktop release-host smoke requires an unprivileged isolated network namespace" >&2
  exit 1
fi

pnpm --dir "$root" --filter @fasti/web build
cargo build \
  --manifest-path "$manifest" \
  --release \
  --locked \
  --offline

[[ -x "$release_binary" ]] || {
  echo "The locked release desktop binary is missing" >&2
  exit 1
}

work_root="$(mktemp -d "${TMPDIR:-/tmp}/fasti-c1-release-host-smoke.XXXXXX")"
chmod 700 "$work_root"
cleanup() {
  rm -rf -- "$work_root"
}
trap cleanup EXIT

installed_binary="$work_root/installation/fasti-desktop"
mkdir -m 700 "$work_root/installation" "$work_root/data"
cp -- "$release_binary" "$installed_binary"
chmod 700 "$installed_binary"

release_sha256="$(sha256sum "$release_binary" | awk '{print $1}')"
installed_sha256="$(sha256sum "$installed_binary" | awk '{print $1}')"
[[ "$installed_sha256" == "$release_sha256" ]] || {
  echo "The installed desktop bytes differ from the locked release artifact" >&2
  exit 1
}
artifact_size="$(stat -c %s "$installed_binary")"

unshare --user --map-root-user --net -- /usr/bin/env bash -euo pipefail -c '
  work_root="$1"
  binary="$2"
  ip link set lo up
  export HOME="$work_root/home"
  export XDG_DATA_HOME="$work_root/xdg/data"
  export XDG_CONFIG_HOME="$work_root/xdg/config"
  export XDG_CACHE_HOME="$work_root/xdg/cache"
  export XDG_STATE_HOME="$work_root/xdg/state"
  export FASTI_DATA_ROOT="$work_root/data"
  unset FASTI_TRAILBASE_ROOT

  mkdir -m 700 -p \
    "$HOME" "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_STATE_HOME"

  "$binary" >"$work_root/desktop.log" 2>&1 &
  desktop_pid=$!
  cleanup_processes() {
    kill "$desktop_pid" 2>/dev/null || true
    wait "$desktop_pid" 2>/dev/null || true
  }
  trap cleanup_processes EXIT INT TERM

  ready=0
  for _ in $(seq 1 80); do
    if curl --fail --silent --max-time 2 \
      http://127.0.0.1:8420/ >"$work_root/index.html" 2>/dev/null; then
      ready=1
      break
    fi
    if ! kill -0 "$desktop_pid" 2>/dev/null; then
      cat "$work_root/desktop.log" >&2
      echo "The installed desktop exited before its fixed listener was ready" >&2
      exit 1
    fi
    sleep 0.25
  done
  [[ "$ready" == 1 ]] || {
    cat "$work_root/desktop.log" >&2
    echo "The installed desktop did not bind 127.0.0.1:8420" >&2
    exit 1
  }

  root_status="$(curl --silent --show-error --output "$work_root/index.html" \
    --write-out "%{http_code}" --max-time 3 http://127.0.0.1:8420/)"
  first_run_status="$(curl --silent --show-error --output "$work_root/first-run.html" \
    --write-out "%{http_code}" --max-time 3 http://127.0.0.1:8420/first-run)"
  health_status="$(curl --silent --show-error --output "$work_root/health.json" \
    --write-out "%{http_code}" --max-time 3 http://127.0.0.1:8420/api/v1/health)"
  projection_status="$(curl --silent --show-error --output "$work_root/projection.json" \
    --write-out "%{http_code}" --max-time 3 http://127.0.0.1:8420/api/access/v1/projection)"

  [[ "$root_status" == 200 && "$first_run_status" == 200 && "$health_status" == 200 ]] || {
    echo "The installed desktop did not serve its embedded routes" >&2
    exit 1
  }
  for page in "$work_root/index.html" "$work_root/first-run.html"; do
    grep -Fq "<title>Fasti · Living Chronicle</title>" "$page" \
      && grep -Fq '\''<div id="app"></div>'\'' "$page" || {
        echo "The embedded application shell is absent from a desktop route" >&2
        exit 1
      }
  done
  jq -e '\''(.status == "healthy") and (.version | type == "string" and length > 0)'\'' \
    "$work_root/health.json" >/dev/null
  [[ "$projection_status" == 401 ]] || {
    echo "The signed-out Access projection was not mounted fail-closed" >&2
    exit 1
  }
  jq -e '\''.type == "https://fasti.scrobble.dev/v1/problems/browser-session-revoked"'\'' \
    "$work_root/projection.json" >/dev/null

  process_rows="$(ps -eo pid=,ppid=,args=)"
  printf "%s\n" "$process_rows" | awk -v parent="$desktop_pid" \
    '\''$2 == parent && /WebKitNetworkProcess/ { found=1 } END { exit !found }'\'' || {
      echo "The installed desktop did not start WebKitNetworkProcess" >&2
      exit 1
    }
  printf "%s\n" "$process_rows" | awk -v parent="$desktop_pid" \
    '\''$2 == parent && /WebKitWebProcess/ { found=1 } END { exit !found }'\'' || {
      echo "The installed desktop did not start WebKitWebProcess" >&2
      exit 1
    }

  printf "%s\n" "$desktop_pid" >"$work_root/desktop.pid"
' bash "$work_root" "$installed_binary"

post_run_sha256="$(sha256sum "$installed_binary" | awk '{print $1}')"
[[ "$post_run_sha256" == "$installed_sha256" ]] || {
  echo "The installed desktop artifact changed while it ran" >&2
  exit 1
}

[[ -z "$(git -C "$root" status --porcelain=v1 --untracked-files=all)" ]] || {
  echo "The desktop release-host smoke changed the Git worktree" >&2
  exit 1
}
[[ "$(git -C "$root" rev-parse HEAD)" == "$git_commit" \
  && "$(git -C "$root" rev-parse HEAD^{tree})" == "$git_tree" ]] || {
  echo "The source identity changed while the desktop release-host smoke ran" >&2
  exit 1
}
jq -cn \
  --arg git_commit "$git_commit" \
  --arg git_tree "$git_tree" \
  --arg sha256 "$installed_sha256" \
  --arg architecture "$architecture" \
  --argjson size_bytes "$artifact_size" \
  '{
    status: "PASS",
    gate: "access.desktop-release-host-smoke",
    platform: {os: "linux", architecture: $architecture},
    source: {git_commit: $git_commit, git_tree: $git_tree},
    artifact: {kind: "copied-release-binary", sha256: $sha256, size_bytes: $size_bytes},
    routes: {root: 200, first_run: 200, health: 200, signed_out_access_projection: 401},
    webview: {network_process: true, web_process: true},
    development_server: false,
    network_isolation: "route-less user network namespace"
  }'
