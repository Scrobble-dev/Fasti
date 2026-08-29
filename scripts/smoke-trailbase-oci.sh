#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
scope="${FASTI_DEV_SCOPE:-$(basename "$repo_root")}"
scope="${scope//[^A-Za-z0-9_.-]/-}"
container="trailbase-dev-$scope"
inspect_file="$(mktemp -p "$repo_root/target" fasti-trailbase-oci-inspect.XXXXXX)"

cleanup() {
  "$repo_root/scripts/dev.sh" trailbase stop >/dev/null 2>&1 || true
  rm -f -- "$inspect_file"
}
trap cleanup EXIT INT TERM

command -v podman >/dev/null || { echo "Podman is required for the OCI conformance gate." >&2; exit 1; }
if podman inspect "$container" --format '{{.State.Running}}' 2>/dev/null | grep -qx true; then
  echo "Stop the scoped TrailBase container before the OCI conformance gate." >&2
  exit 1
fi

"$repo_root/scripts/dev.sh" trailbase start --podman
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

"$repo_root/scripts/dev.sh" trailbase stop >/dev/null
if podman inspect "$container" >/dev/null 2>&1; then
  echo "Stopped OCI container was not removed." >&2
  exit 1
fi
echo "PASS: exact TrailBase OCI lifecycle, isolation, and active-backup guard"
