#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
scope="${FASTI_DEV_SCOPE:-$(basename "$repo_root")}"
scope="${scope//[^A-Za-z0-9_.-]/-}"
container="trailbase-dev-$scope"
inspect_file="$(mktemp -p "$repo_root/target" fasti-trailbase-oci-inspect.XXXXXX)"
owned_container_id=""

cleanup() {
  if [[ -n "$owned_container_id" ]] &&
    [[ "$(podman inspect "$container" --format '{{.Id}}' 2>/dev/null || true)" == "$owned_container_id" ]]; then
    podman stop "$owned_container_id" >/dev/null 2>&1 || true
  fi
  rm -f -- "$inspect_file"
}
trap cleanup EXIT INT TERM

command -v podman >/dev/null || { echo "Podman is required for the OCI conformance gate." >&2; exit 1; }
if podman inspect "$container" --format '{{.State.Running}}' 2>/dev/null | grep -qx true; then
  echo "Stop the scoped TrailBase container before the OCI conformance gate." >&2
  exit 1
fi
stopped_status="$("$repo_root/scripts/dev.sh" trailbase status)"
grep -q 'Process: STOPPED' <<<"$stopped_status" || {
  echo "Stop the scoped native TrailBase runtime before the OCI conformance gate." >&2
  exit 1
}

start_output="$("$repo_root/scripts/dev.sh" trailbase start --podman)"
if grep -q 'is already running' <<<"$start_output"; then
  echo "The OCI conformance gate did not create its TrailBase container." >&2
  exit 1
fi
printf '%s\n' "$start_output"
owned_container_id="$(sed -n 's/^Container ID: //p' <<<"$start_output")"
[[ "$(stat -c '%a' "$repo_root/.dev-trailbase/runtime.lock")" == 600 ]] || {
  echo "OCI launcher did not create an owner-only runtime lock." >&2
  exit 1
}
if [[ ! "$owned_container_id" =~ ^[0-9a-f]{64}$ ]] ||
  [[ "$(podman inspect "$container" --format '{{.Id}}')" != "$owned_container_id" ]]; then
  echo "The OCI launcher did not return the exact container it created." >&2
  exit 1
fi
status="$("$repo_root/scripts/dev.sh" trailbase status)"
grep -q 'Process: RUNNING (podman container:' <<<"$status"
grep -q 'Evidence: exact OCI identity; liveness healthy; admin not published; no Record API configured' <<<"$status"

if "$repo_root/scripts/dev.sh" trailbase backup >/dev/null 2>&1; then
  echo "Active OCI depot backup did not fail closed." >&2
  exit 1
fi

podman inspect "$container" >"$inspect_file"
python3 -B - "$inspect_file" "$repo_root/third_party/trailbase/release.json" <<'PY'
import json
import sys

inspection = json.load(open(sys.argv[1], encoding="utf-8"))
release = json.load(open(sys.argv[2], encoding="utf-8"))
if not isinstance(inspection, list) or len(inspection) != 1:
    raise SystemExit("OCI inspection did not return one container")
container = inspection[0]
host = container["HostConfig"]
config = container["Config"]
expected_image = f'{release["oci"]["repository"]}@{release["oci"]["index_digest"]}'
expected = {
    "Memory": 192 * 1024 * 1024,
    "MemorySwap": 192 * 1024 * 1024,
    "NanoCpus": 1_000_000_000,
    "PidsLimit": 128,
    "ReadonlyRootfs": True,
}
if any(host.get(key) != value for key, value in expected.items()):
    raise SystemExit("OCI resource or read-only policy drifted")
if "no-new-privileges" not in host.get("SecurityOpt", []):
    raise SystemExit("OCI no-new-privileges policy is missing")
if host.get("CapAdd") not in (None, []):
    raise SystemExit("OCI added Linux capabilities")
if host.get("LogConfig", {}).get("Type") != "none":
    raise SystemExit("OCI log driver is not disabled")
if host.get("PortBindings") != {"4000/tcp": [{"HostIp": "127.0.0.1", "HostPort": "4000"}]}:
    raise SystemExit("OCI public port boundary drifted")
if config.get("Image") != expected_image:
    raise SystemExit("OCI image is not the exact release digest")
if config.get("User", "").split(":", 1)[0] in ("", "0", "root"):
    raise SystemExit("OCI runtime user is root")
command = config.get("Cmd", [])
for pair in (["--admin-address", "127.0.0.1:4001"], ["--runtime-threads", "1"]):
    if not any(command[index:index + 2] == pair for index in range(len(command) - 1)):
        raise SystemExit(f"OCI command omits {' '.join(pair)}")
PY

[[ "$(podman top "$container" capeff | tail -n 1)" == none ]] || {
  echo "OCI process retained an effective Linux capability." >&2
  exit 1
}

podman stop "$owned_container_id" >/dev/null
owned_container_id=""
if podman inspect "$container" >/dev/null 2>&1; then
  echo "Stopped OCI container was not removed." >&2
  exit 1
fi
reference="$(python3 -B "$repo_root/scripts/trailbase_runtime.py" prepare-oci \
  "$repo_root/.dev-trailbase" --runtime podman --offline)"
owned_container_id="$(podman run -d --name "$container" --rm --pull never \
  --userns keep-id --user "$(id -u):$(id -g)" \
  --volume "$repo_root/.dev-trailbase:/app/trailroot:Z" \
  "$reference" \
  /app/trail --depot /app/trailroot/depot --public-url http://127.0.0.1:4000 \
  run --address 0.0.0.0:4000 --admin-address 127.0.0.1:4001 \
  --cors-allowed-origins http://127.0.0.1:4000 --runtime-threads 1)"
if "$repo_root/scripts/dev.sh" trailbase start --podman >/dev/null 2>&1; then
  echo "Launcher accepted a scoped container with a drifted isolation policy." >&2
  exit 1
fi
podman stop "$owned_container_id" >/dev/null
owned_container_id=""
echo "PASS: exact TrailBase OCI lifecycle, isolation, and active-backup guard"
