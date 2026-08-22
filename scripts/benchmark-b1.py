#!/usr/bin/env python3
"""Capture honest B1 native and OCI performance evidence on Linux.

The capture path deliberately refuses partial or inferred results. Native
subjects run inside a route-less Linux network namespace. OCI subjects run
with Docker's `--network none` and require cgroup v2 counters.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shlex
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
B1_DIR = ROOT / "benchmarks" / "b1"
BUDGETS_PATH = B1_DIR / "budgets.json"
VALIDATOR_PATH = B1_DIR / "validate-evidence.mjs"
HARNESS_VERSION = "fasti-b1-benchmark.v1"
SCENARIO_IDS = (
    "native_empty_process",
    "native_fastid_idle",
    "oci_empty_process",
    "oci_fastid_idle",
    "oci_fasti_cli_guard",
)


class CaptureError(RuntimeError):
    """A missing prerequisite or unsupported measurement invalidated capture."""


def command_text(args: list[str]) -> str:
    return shlex.join(str(part) for part in args)


def run_checked(
    args: list[str],
    *,
    cwd: Path = ROOT,
    timeout: float = 30,
    input_text: str | None = None,
) -> str:
    result = subprocess.run(
        args,
        cwd=cwd,
        input=input_text,
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "no diagnostic"
        raise CaptureError(f"command failed ({command_text(args)}): {detail}")
    return result.stdout.strip()


def require_command(name: str) -> None:
    if shutil.which(name) is None:
        raise CaptureError(f"required command is unavailable: {name}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_os_release() -> str:
    path = Path("/etc/os-release")
    if not path.is_file():
        raise CaptureError("/etc/os-release is required for the runner fingerprint")
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" not in line or line.startswith("#"):
            continue
        key, value = line.split("=", 1)
        values[key] = value.strip().strip('"')
    description = values.get("PRETTY_NAME")
    if not description:
        raise CaptureError("PRETTY_NAME is missing from /etc/os-release")
    return description


def parse_cpu_model() -> str:
    cpuinfo = Path("/proc/cpuinfo")
    if not cpuinfo.is_file():
        raise CaptureError("/proc/cpuinfo is required for the runner fingerprint")
    for line in cpuinfo.read_text(encoding="utf-8", errors="replace").splitlines():
        key, separator, value = line.partition(":")
        if separator and key.strip().lower() in {"model name", "model", "hardware"}:
            model = value.strip()
            if model:
                return model
    raise CaptureError("no CPU model was found in /proc/cpuinfo")


def parse_total_memory_bytes() -> int:
    meminfo = Path("/proc/meminfo")
    if not meminfo.is_file():
        raise CaptureError("/proc/meminfo is required for the runner fingerprint")
    for line in meminfo.read_text(encoding="utf-8").splitlines():
        if line.startswith("MemTotal:"):
            parts = line.split()
            if len(parts) == 3 and parts[2] == "kB":
                return int(parts[1]) * 1024
    raise CaptureError("MemTotal is missing or unsupported in /proc/meminfo")


def ensure_clean_tree() -> tuple[str, str]:
    status = run_checked(["git", "status", "--porcelain=v1", "--untracked-files=all"])
    if status:
        raise CaptureError(
            "performance evidence requires a clean source tree; commit or remove every tracked and untracked change first"
        )
    commit = run_checked(["git", "rev-parse", "HEAD"])
    tree = run_checked(["git", "rev-parse", "HEAD^{tree}"])
    if len(commit) != 40 or len(tree) != 40:
        raise CaptureError("Git did not return full commit and tree object IDs")
    return commit, tree


def preflight(args: argparse.Namespace) -> dict[str, Any]:
    if platform.system() != "Linux":
        raise CaptureError(
            f"B1 performance capture is Linux-only; {platform.system()} cannot provide the required /proc, netns, and cgroup-v2 evidence"
        )

    for command in ["curl", "docker", "git", "ip", "node", "unshare"]:
        require_command(command)

    if not args.native_binary.is_file() or not os.access(args.native_binary, os.X_OK):
        raise CaptureError(f"native fastid binary is missing or not executable: {args.native_binary}")
    if args.output.exists():
        raise CaptureError(f"refusing to overwrite existing evidence: {args.output}")

    commit, tree = ensure_clean_tree()
    if not Path("/sys/fs/cgroup/cgroup.controllers").is_file():
        raise CaptureError("cgroup v2 is required; /sys/fs/cgroup/cgroup.controllers is absent")

    unshare_test = [
        "unshare",
        "--user",
        "--map-root-user",
        "--net",
        "/bin/sh",
        "-c",
        'ip link set lo up && test -z "$(ip route show)"',
    ]
    run_checked(unshare_test)

    docker_cgroup = run_checked(["docker", "info", "--format", "{{.CgroupVersion}}"])
    if docker_cgroup != "2":
        raise CaptureError(f"Docker must use cgroup v2, reported {docker_cgroup!r}")
    docker_version = run_checked(["docker", "version", "--format", "{{.Server.Version}}"])
    image_id = run_checked(["docker", "image", "inspect", "--format", "{{.Id}}", args.image])
    if not image_id:
        raise CaptureError(f"Docker image has no immutable ID: {args.image}")

    fingerprint_commands = [
        "uname -srmo",
        "read /etc/os-release:PRETTY_NAME",
        "read /proc/cpuinfo:first(model name|model|hardware)",
        "read /proc/meminfo:MemTotal",
        command_text(["docker", "version", "--format", "{{.Server.Version}}"]),
        command_text(["docker", "info", "--format", "{{.CgroupVersion}}"]),
        command_text(unshare_test),
    ]

    return {
        "runner": {
            "runner_id": args.runner_id,
            "hardware_profile": args.hardware_profile,
            "custodian": args.custodian,
            "os_release": parse_os_release(),
            "kernel_release": platform.release(),
            "architecture": platform.machine(),
            "cpu_model": parse_cpu_model(),
            "logical_cpu_count": os.cpu_count() or 1,
            "total_memory_bytes": parse_total_memory_bytes(),
            "cgroup_version": "v2",
            "container_engine": {"name": "docker", "version": docker_version},
        },
        "source": {
            "git_commit": commit,
            "git_tree": tree,
            "tree_state": "clean",
            "native_fastid_sha256": sha256_file(args.native_binary),
            "oci_image_ref": args.image,
            "oci_image_id": image_id,
            "contract_ref": args.contract_ref,
        },
        "fingerprint_commands": fingerprint_commands,
    }


def process_tree(root_pid: int) -> list[int]:
    pending = [root_pid]
    seen: set[int] = set()
    while pending:
        pid = pending.pop()
        if pid in seen:
            continue
        seen.add(pid)
        children_path = Path(f"/proc/{pid}/task/{pid}/children")
        try:
            children = children_path.read_text(encoding="ascii").split()
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue
        for child in children:
            try:
                pending.append(int(child))
            except ValueError:
                continue
    return sorted(pid for pid in seen if Path(f"/proc/{pid}").exists())


def process_rss_bytes(pid: int) -> int:
    try:
        lines = Path(f"/proc/{pid}/status").read_text(encoding="ascii").splitlines()
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        return 0
    for line in lines:
        if line.startswith("VmRSS:"):
            fields = line.split()
            if len(fields) == 3 and fields[2] == "kB":
                return int(fields[1]) * 1024
    return 0


def process_cpu_ticks(pid: int) -> int:
    try:
        value = Path(f"/proc/{pid}/stat").read_text(encoding="ascii")
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        return 0
    close = value.rfind(")")
    if close < 0:
        return 0
    fields = value[close + 2 :].split()
    if len(fields) < 13:
        return 0
    return int(fields[11]) + int(fields[12])


def cgroup_path_for_pid(pid: int) -> Path:
    try:
        lines = Path(f"/proc/{pid}/cgroup").read_text(encoding="ascii").splitlines()
    except (FileNotFoundError, PermissionError) as error:
        raise CaptureError(f"cannot read cgroup membership for PID {pid}: {error}") from error
    relative = None
    for line in lines:
        if line.startswith("0::"):
            relative = line[3:]
            break
    if relative is None:
        raise CaptureError(f"PID {pid} has no cgroup-v2 membership")
    path = Path("/sys/fs/cgroup") / relative.lstrip("/")
    for required in ["memory.current", "memory.peak", "cpu.stat"]:
        if not (path / required).is_file():
            raise CaptureError(f"container cgroup lacks {required}: {path}")
    return path


def cgroup_cpu_seconds(path: Path) -> float:
    for line in (path / "cpu.stat").read_text(encoding="ascii").splitlines():
        key, value = line.split(maxsplit=1)
        if key == "usage_usec":
            return int(value) / 1_000_000
    raise CaptureError(f"usage_usec is missing from {path / 'cpu.stat'}")


def read_cgroup(path: Path) -> tuple[int, int, float]:
    current = int((path / "memory.current").read_text(encoding="ascii").strip())
    peak_text = (path / "memory.peak").read_text(encoding="ascii").strip()
    if peak_text == "max":
        raise CaptureError(f"unsupported unbounded memory.peak value in {path}")
    return current, int(peak_text), cgroup_cpu_seconds(path)


class Sampler:
    def __init__(self, root_pid: int, interval_ms: int, cgroup_path: Path | None = None):
        self.root_pid = root_pid
        self.interval_seconds = interval_ms / 1000
        self.cgroup_path = cgroup_path
        self.records: list[dict[str, float | int]] = []
        self.error: Exception | None = None
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name="fasti-b1-sampler", daemon=True)
        self._previous_ticks: dict[int, int] = {}
        self._accumulated_ticks = 0

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        self._thread.join(timeout=5)
        if self._thread.is_alive():
            raise CaptureError("measurement sampler did not stop")
        if self.error is not None:
            raise CaptureError(f"measurement sampler failed: {self.error}") from self.error

    def _sample(self) -> None:
        pids = process_tree(self.root_pid)
        if not pids:
            return
        ticks: dict[int, int] = {}
        for pid in pids:
            ticks[pid] = process_cpu_ticks(pid)
            previous = self._previous_ticks.get(pid)
            if previous is None:
                self._accumulated_ticks += ticks[pid]
            elif ticks[pid] >= previous:
                self._accumulated_ticks += ticks[pid] - previous
        self._previous_ticks = ticks

        record: dict[str, float | int] = {
            "at": time.monotonic(),
            "rss_bytes": sum(process_rss_bytes(pid) for pid in pids),
            "cpu_ticks": self._accumulated_ticks,
            "process_count": len(pids),
        }
        if self.cgroup_path is not None:
            current, peak, cpu_seconds = read_cgroup(self.cgroup_path)
            record.update(
                {
                    "cgroup_current_bytes": current,
                    "cgroup_peak_bytes": peak,
                    "cgroup_cpu_seconds": cpu_seconds,
                }
            )
        self.records.append(record)

    def _run(self) -> None:
        try:
            while not self._stop.is_set():
                self._sample()
                self._stop.wait(self.interval_seconds)
            self._sample()
        except Exception as error:  # surfaced synchronously by stop()
            self.error = error


def wait_for_file(path: Path, process: subprocess.Popen[Any], timeout: float, log_path: Path) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            return
        code = process.poll()
        if code is not None:
            diagnostic = log_path.read_text(encoding="utf-8", errors="replace")[-4000:]
            raise CaptureError(f"subject exited before readiness with code {code}: {diagnostic}")
        time.sleep(0.01)
    raise CaptureError(f"subject did not become ready within {timeout} seconds")


def wait_steady(process_alive: callable, seconds: float) -> None:
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if not process_alive():
            raise CaptureError("subject exited during the steady measurement window")
        time.sleep(min(0.1, deadline - time.monotonic()))


def stop_process_group(process: subprocess.Popen[Any]) -> None:
    if process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=3)
    except (ProcessLookupError, subprocess.TimeoutExpired):
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired as error:
            raise CaptureError(f"could not stop process group {process.pid}") from error


def metrics_from_records(
    records: list[dict[str, float | int]],
    *,
    started_at: float,
    ready_at: float,
    finished_at: float,
    clock_ticks: int,
    with_cgroup: bool,
) -> dict[str, Any]:
    if not records:
        raise CaptureError("no process-tree measurements were captured")
    steady = [record for record in records if float(record["at"]) >= ready_at]
    if len(steady) < 2:
        raise CaptureError("fewer than two steady-state samples were captured")
    if any(int(record["rss_bytes"]) <= 0 for record in steady):
        raise CaptureError("a steady-state process-tree RSS sample was missing")

    elapsed = max(finished_at - started_at, 0.000001)
    cpu_seconds = int(records[-1]["cpu_ticks"]) / clock_ticks
    result: dict[str, Any] = {
        "startup_ms": round((ready_at - started_at) * 1000, 3),
        "steady_process_tree_rss_bytes": round(
            statistics.median(int(record["rss_bytes"]) for record in steady)
        ),
        "peak_process_tree_rss_bytes": max(int(record["rss_bytes"]) for record in records),
        "process_tree_cpu_seconds": round(cpu_seconds, 6),
        "process_tree_cpu_percent": round((cpu_seconds / elapsed) * 100, 6),
        "process_count_peak": max(int(record["process_count"]) for record in records),
        "cgroup": None,
    }

    if with_cgroup:
        required = {"cgroup_current_bytes", "cgroup_peak_bytes", "cgroup_cpu_seconds"}
        if any(not required.issubset(record) for record in steady):
            raise CaptureError("one or more cgroup-v2 measurements were missing")
        cgroup_cpu = float(records[-1]["cgroup_cpu_seconds"])
        result["cgroup"] = {
            "steady_memory_current_bytes": round(
                statistics.median(int(record["cgroup_current_bytes"]) for record in steady)
            ),
            "peak_memory_bytes": max(int(record["cgroup_peak_bytes"]) for record in records),
            "cpu_seconds": round(cgroup_cpu, 6),
            "cpu_percent": round((cgroup_cpu / elapsed) * 100, 6),
        }
    return result


def run_native_once(
    scenario_id: str,
    run_number: int,
    args: argparse.Namespace,
) -> tuple[dict[str, Any], list[str]]:
    with tempfile.TemporaryDirectory(prefix=f"fasti-b1-{scenario_id}-") as temp_name:
        temp = Path(temp_name)
        ready = temp / "ready"
        routes = temp / "routes"
        health = temp / "health.json"
        log = temp / "subject.log"

        if scenario_id == "native_empty_process":
            script = """
set -eu
ip link set lo up
ip route show > "$1"
test ! -s "$1"
: > "$2"
exec /bin/sleep 3600
""".strip()
            command = [
                "unshare",
                "--user",
                "--map-root-user",
                "--net",
                "/bin/sh",
                "-c",
                script,
                "fasti-native-empty",
                str(routes),
                str(ready),
            ]
        elif scenario_id == "native_fastid_idle":
            script = """
set -eu
ip link set lo up
ip route show > "$1"
test ! -s "$1"
FASTI_LISTEN=127.0.0.1:8420 "$4" &
daemon_pid=$!
trap 'kill "$daemon_pid" 2>/dev/null || true' EXIT INT TERM
attempt=0
while [ "$attempt" -lt 500 ]; do
  if curl --fail --silent --max-time 1 http://127.0.0.1:8420/api/v1/health > "$3"; then
    : > "$2"
    wait "$daemon_pid"
    exit $?
  fi
  kill -0 "$daemon_pid" 2>/dev/null || wait "$daemon_pid"
  attempt=$((attempt + 1))
  sleep 0.01
done
echo "native fastid health probe timed out" >&2
exit 92
""".strip()
            command = [
                "unshare",
                "--user",
                "--map-root-user",
                "--net",
                "/bin/sh",
                "-c",
                script,
                "fasti-native-fastid",
                str(routes),
                str(ready),
                str(health),
                str(args.native_binary),
            ]
        else:
            raise CaptureError(f"unknown native scenario: {scenario_id}")

        started_at = time.monotonic()
        with log.open("wb") as log_handle:
            process = subprocess.Popen(
                command,
                cwd=ROOT,
                stdout=log_handle,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            sampler = Sampler(process.pid, args.sample_interval_ms)
            sampler.start()
            try:
                wait_for_file(ready, process, args.startup_timeout_seconds, log)
                ready_at = time.monotonic()
                if routes.read_text(encoding="utf-8").strip():
                    raise CaptureError(f"{scenario_id} network namespace unexpectedly has an IP route")
                if scenario_id == "native_fastid_idle":
                    payload = json.loads(health.read_text(encoding="utf-8"))
                    if payload.get("status") != "healthy" or not payload.get("version"):
                        raise CaptureError(f"native health response is invalid: {payload!r}")
                wait_steady(lambda: process.poll() is None, args.steady_window_seconds)
                finished_at = time.monotonic()
            finally:
                try:
                    sampler.stop()
                finally:
                    stop_process_group(process)

        metrics = metrics_from_records(
            sampler.records,
            started_at=started_at,
            ready_at=ready_at,
            finished_at=finished_at,
            clock_ticks=os.sysconf("SC_CLK_TCK"),
            with_cgroup=False,
        )
        metrics["run"] = run_number
        return metrics, [command_text(command)]


def docker_container_pid(name: str, timeout: float) -> int:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = run_checked(
            ["docker", "inspect", "--format", "{{.State.Pid}}", name],
            timeout=5,
        )
        pid = int(value)
        if pid > 0:
            return pid
        time.sleep(0.01)
    raise CaptureError(f"Docker container {name} did not expose a host PID")


def docker_running(name: str) -> bool:
    result = subprocess.run(
        ["docker", "inspect", "--format", "{{.State.Running}}", name],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    return result.returncode == 0 and result.stdout.strip() == "true"


def docker_logs(name: str) -> str:
    result = subprocess.run(
        ["docker", "logs", name],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise CaptureError(f"could not read logs for {name}: {result.stderr.strip()}")
    return result.stdout + result.stderr


def run_oci_once(
    scenario_id: str,
    run_number: int,
    args: argparse.Namespace,
) -> tuple[dict[str, Any], list[str], int | None]:
    suffix = uuid.uuid4().hex[:10]
    name = f"fasti-b1-{scenario_id.replace('_', '-')}-{os.getpid()}-{run_number}-{suffix}"
    commands: list[str] = []
    observed_exit: int | None = None

    if scenario_id == "oci_empty_process":
        script = "printf 'FASTI_EMPTY_READY\\n'; exec /bin/sleep 3600"
    elif scenario_id == "oci_fastid_idle":
        script = "exec /usr/local/bin/fastid"
    elif scenario_id == "oci_fasti_cli_guard":
        script = """
/usr/local/bin/fasti verify >/tmp/fasti-cli.stdout 2>/tmp/fasti-cli.stderr
code=$?
printf 'FASTI_CLI_EXIT=%s\\n' "$code"
exec /bin/sleep 3600
""".strip()
    else:
        raise CaptureError(f"unknown OCI scenario: {scenario_id}")

    run_command = [
        "docker",
        "run",
        "--detach",
        "--name",
        name,
        "--network",
        "none",
        "--entrypoint",
        "/bin/sh",
        args.image,
        "-c",
        script,
    ]
    commands.append(command_text(run_command))
    started_at = time.monotonic()
    try:
        container_id = run_checked(run_command, timeout=args.startup_timeout_seconds)
        if not container_id:
            raise CaptureError(f"Docker returned no container ID for {scenario_id}")

        network_mode_command = ["docker", "inspect", "--format", "{{.HostConfig.NetworkMode}}", name]
        networks_command = ["docker", "inspect", "--format", "{{json .NetworkSettings.Networks}}", name]
        commands.extend([command_text(network_mode_command), command_text(networks_command)])
        if run_checked(network_mode_command) != "none":
            raise CaptureError(f"{scenario_id} was not created with Docker network mode none")
        networks = json.loads(run_checked(networks_command))
        if networks:
            raise CaptureError(f"{scenario_id} unexpectedly has Docker networks: {networks!r}")

        pid = docker_container_pid(name, args.startup_timeout_seconds)
        cgroup_path = cgroup_path_for_pid(pid)
        commands.extend(
            [
                command_text(["docker", "inspect", "--format", "{{.State.Pid}}", name]),
                f"read {cgroup_path / 'memory.current'} {cgroup_path / 'memory.peak'} {cgroup_path / 'cpu.stat'}",
                f"read /proc/{pid}/task/{pid}/children and descendant /proc/<pid>/status,/proc/<pid>/stat",
            ]
        )
        sampler = Sampler(pid, args.sample_interval_ms, cgroup_path)
        sampler.start()
        try:
            deadline = time.monotonic() + args.startup_timeout_seconds
            if scenario_id == "oci_empty_process":
                commands.append(command_text(["docker", "logs", name]))
                while time.monotonic() < deadline:
                    if "FASTI_EMPTY_READY" in docker_logs(name):
                        break
                    if not docker_running(name):
                        raise CaptureError("OCI empty process exited before readiness")
                    time.sleep(0.01)
                else:
                    raise CaptureError("OCI empty process did not become ready")
            elif scenario_id == "oci_fastid_idle":
                health_command = [
                    "docker",
                    "exec",
                    name,
                    "wget",
                    "-q",
                    "-O",
                    "-",
                    "http://127.0.0.1:8420/api/v1/health",
                ]
                commands.append(command_text(health_command))
                payload = None
                while time.monotonic() < deadline:
                    probe = subprocess.run(
                        health_command,
                        cwd=ROOT,
                        text=True,
                        capture_output=True,
                        check=False,
                    )
                    if probe.returncode == 0:
                        payload = json.loads(probe.stdout)
                        break
                    if not docker_running(name):
                        raise CaptureError(f"OCI fastid exited before health: {docker_logs(name)[-4000:]}")
                    time.sleep(0.01)
                if payload is None:
                    raise CaptureError("OCI fastid did not become healthy")
                if payload.get("status") != "healthy" or not payload.get("version"):
                    raise CaptureError(f"OCI health response is invalid: {payload!r}")
            else:
                commands.append(command_text(["docker", "logs", name]))
                marker = None
                while time.monotonic() < deadline:
                    for line in docker_logs(name).splitlines():
                        if line.startswith("FASTI_CLI_EXIT="):
                            marker = line
                            break
                    if marker is not None:
                        break
                    if not docker_running(name):
                        raise CaptureError(f"OCI CLI wrapper exited before its marker: {docker_logs(name)[-4000:]}")
                    time.sleep(0.005)
                if marker is None:
                    raise CaptureError("OCI CLI did not record its exit before the startup timeout")
                observed_exit = int(marker.split("=", 1)[1])
                if observed_exit == 0:
                    raise CaptureError("guarded OCI fasti verify command unexpectedly succeeded")
                with tempfile.TemporaryDirectory(prefix="fasti-b1-cli-output-") as output_dir:
                    output_path = Path(output_dir)
                    stdout_path = output_path / "stdout"
                    stderr_path = output_path / "stderr"
                    copy_stdout = [
                        "docker",
                        "cp",
                        f"{name}:/tmp/fasti-cli.stdout",
                        str(stdout_path),
                    ]
                    copy_stderr = [
                        "docker",
                        "cp",
                        f"{name}:/tmp/fasti-cli.stderr",
                        str(stderr_path),
                    ]
                    commands.extend([command_text(copy_stdout), command_text(copy_stderr)])
                    run_checked(copy_stdout)
                    run_checked(copy_stderr)
                    stderr = stderr_path.read_text(encoding="utf-8", errors="replace")
                    if stdout_path.stat().st_size != 0 or not all(
                        phrase in stderr for phrase in ["is not available", "No data was changed"]
                    ):
                        raise CaptureError("guarded OCI CLI output did not match the explicit unavailable contract")

            ready_at = time.monotonic()
            wait_steady(lambda: docker_running(name), args.steady_window_seconds)
            finished_at = time.monotonic()
        finally:
            sampler.stop()

        metrics = metrics_from_records(
            sampler.records,
            started_at=started_at,
            ready_at=ready_at,
            finished_at=finished_at,
            clock_ticks=os.sysconf("SC_CLK_TCK"),
            with_cgroup=True,
        )
        metrics["run"] = run_number
        return metrics, commands, observed_exit
    finally:
        subprocess.run(
            ["docker", "rm", "--force", name],
            cwd=ROOT,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )


def summarize(values: list[int | float]) -> dict[str, int | float]:
    return {
        "minimum": min(values),
        "median": statistics.median(values),
        "maximum": max(values),
    }


def scenario_summary(samples: list[dict[str, Any]], with_cgroup: bool) -> dict[str, Any]:
    result = {
        field: summarize([sample[field] for sample in samples])
        for field in [
            "startup_ms",
            "steady_process_tree_rss_bytes",
            "peak_process_tree_rss_bytes",
            "process_tree_cpu_seconds",
            "process_tree_cpu_percent",
            "process_count_peak",
        ]
    }
    mapping = {
        "steady_cgroup_memory_current_bytes": "steady_memory_current_bytes",
        "peak_cgroup_memory_bytes": "peak_memory_bytes",
        "cgroup_cpu_seconds": "cpu_seconds",
        "cgroup_cpu_percent": "cpu_percent",
    }
    for summary_name, sample_name in mapping.items():
        result[summary_name] = (
            summarize([sample["cgroup"][sample_name] for sample in samples]) if with_cgroup else None
        )
    return result


def unique_in_order(values: list[str]) -> list[str]:
    seen: set[str] = set()
    result: list[str] = []
    for value in values:
        if value not in seen:
            seen.add(value)
            result.append(value)
    return result


def capture_scenario(scenario_id: str, args: argparse.Namespace) -> dict[str, Any]:
    samples: list[dict[str, Any]] = []
    commands: list[str] = []
    exits: list[int | None] = []
    with_cgroup = scenario_id.startswith("oci_")
    for run_number in range(1, args.repetitions + 1):
        if with_cgroup:
            sample, run_commands, observed_exit = run_oci_once(scenario_id, run_number, args)
            exits.append(observed_exit)
        else:
            sample, run_commands = run_native_once(scenario_id, run_number, args)
            exits.append(None)
        samples.append(sample)
        commands.extend(run_commands)

    if scenario_id == "oci_fasti_cli_guard":
        observed_codes = {code for code in exits if code is not None}
        if len(observed_codes) != 1:
            raise CaptureError(f"OCI CLI guard exit codes were inconsistent: {sorted(observed_codes)}")
        workload_exit = {
            "expectation": "guarded_nonzero",
            "observed_code": observed_codes.pop(),
            "matched": True,
        }
    else:
        workload_exit = {
            "expectation": "running_until_harness_stop",
            "observed_code": None,
            "matched": True,
        }

    subjects = {
        "native_empty_process": "route-less native /bin/sleep process-tree baseline",
        "native_fastid_idle": "route-less native fastid idle process tree",
        "oci_empty_process": "network-none OCI shell and sleep cgroup baseline",
        "oci_fastid_idle": "network-none OCI fastid idle process tree and cgroup",
        "oci_fasti_cli_guard": "network-none OCI guarded fasti CLI launch peak plus retained wrapper baseline",
    }
    proof = (
        "Each repetition ran in a fresh Linux user and network namespace; ip route show was empty. "
        "The fastid case was probed only through loopback inside that namespace."
        if not with_cgroup
        else "Each repetition used Docker --network none; HostConfig.NetworkMode was none and NetworkSettings.Networks was empty."
    )
    return {
        "id": scenario_id,
        "subject": subjects[scenario_id],
        "measurement_scope": "oci_process_tree_and_cgroup_v2" if with_cgroup else "native_process_tree",
        "status": "measured",
        "network_denied": {
            "required": True,
            "observed": True,
            "mechanism": "docker_network_none" if with_cgroup else "linux_network_namespace_without_routes",
            "proof": proof,
        },
        "commands": unique_in_order(commands),
        "workload_exit": workload_exit,
        "samples": samples,
        "summary": scenario_summary(samples, with_cgroup),
    }


def artifact_sizes(args: argparse.Namespace) -> tuple[dict[str, int], list[str]]:
    image_size_command = ["docker", "image", "inspect", "--format", "{{.Size}}", args.image]
    binary_size_command = [
        "docker",
        "run",
        "--rm",
        "--network",
        "none",
        "--entrypoint",
        "/bin/sh",
        args.image,
        "-c",
        "stat -c '%s %s' /usr/local/bin/fastid /usr/local/bin/fasti",
    ]
    binary_values = run_checked(binary_size_command).split()
    if len(binary_values) != 2:
        raise CaptureError(f"unexpected OCI binary size output: {binary_values!r}")
    sizes = {
        "native_fastid_binary_bytes": args.native_binary.stat().st_size,
        "oci_fastid_binary_bytes": int(binary_values[0]),
        "oci_fasti_cli_binary_bytes": int(binary_values[1]),
        "oci_image_bytes": int(run_checked(image_size_command)),
    }
    if any(value <= 0 for value in sizes.values()):
        raise CaptureError(f"one or more artifact sizes are invalid: {sizes!r}")
    commands = [
        command_text(["stat", "-c", "%s", str(args.native_binary)]),
        command_text(binary_size_command),
        command_text(image_size_command),
    ]
    return sizes, commands


def budget_verdicts(scenarios: list[dict[str, Any]], budgets: dict[str, Any]) -> list[dict[str, Any]]:
    by_id = {scenario["id"]: scenario for scenario in scenarios}
    idle_measured = max(
        by_id["native_fastid_idle"]["summary"]["steady_process_tree_rss_bytes"]["maximum"],
        by_id["oci_fastid_idle"]["summary"]["steady_process_tree_rss_bytes"]["maximum"],
        by_id["oci_fastid_idle"]["summary"]["steady_cgroup_memory_current_bytes"]["maximum"],
    )
    absolute_measured = max(
        by_id["native_fastid_idle"]["summary"]["peak_process_tree_rss_bytes"]["maximum"],
        by_id["oci_fastid_idle"]["summary"]["peak_process_tree_rss_bytes"]["maximum"],
        by_id["oci_fastid_idle"]["summary"]["peak_cgroup_memory_bytes"]["maximum"],
        by_id["oci_fasti_cli_guard"]["summary"]["peak_process_tree_rss_bytes"]["maximum"],
        by_id["oci_fasti_cli_guard"]["summary"]["peak_cgroup_memory_bytes"]["maximum"],
    )

    def measured(budget: str, value: int, reason: str) -> dict[str, Any]:
        limit = budgets[budget]
        return {
            "budget": budget,
            "limit_bytes": limit,
            "measured_bytes": value,
            "status": "pass" if value <= limit else "fail",
            "reason": reason,
        }

    return [
        measured(
            "idle_target",
            idle_measured,
            "Worst native process-tree or OCI process-tree/cgroup steady idle maximum across repetitions.",
        ),
        {
            "budget": "normal_target",
            "limit_bytes": budgets["normal_target"],
            "measured_bytes": None,
            "status": "not_applicable",
            "reason": "B1 has no implemented normal-operation workload; no result is claimed.",
        },
        {
            "budget": "heavy_target",
            "limit_bytes": budgets["heavy_target"],
            "measured_bytes": None,
            "status": "not_applicable",
            "reason": "B1 has no implemented heavy-operation workload; no result is claimed.",
        },
        measured(
            "absolute_ceiling",
            absolute_measured,
            "Worst native process-tree or OCI process-tree/cgroup Fasti peak across idle daemon and guarded CLI repetitions.",
        ),
    ]


def capture(args: argparse.Namespace) -> None:
    context = preflight(args)
    budgets_bytes = BUDGETS_PATH.read_bytes()
    budgets_document = json.loads(budgets_bytes)
    memory_budgets = budgets_document["memory_bytes"]

    sizes, size_commands = artifact_sizes(args)
    scenarios = [capture_scenario(scenario_id, args) for scenario_id in SCENARIO_IDS]
    evidence = {
        "$schema": "https://fasti.scrobble.dev/schemas/benchmarks/b1/evidence.schema.json",
        "schema_version": "fasti.b1.performance-evidence.v1",
        "body": "B1",
        "status": "complete",
        "captured_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "runner": context["runner"],
        "source": context["source"],
        "budget_snapshot": {
            "source": "benchmarks/b1/budgets.json",
            "sha256": hashlib.sha256(budgets_bytes).hexdigest(),
            "memory_bytes": memory_budgets,
        },
        "harness": {
            "version": HARNESS_VERSION,
            "repetitions": args.repetitions,
            "steady_window_seconds": args.steady_window_seconds,
            "sample_interval_ms": args.sample_interval_ms,
            "baseline_subtraction": False,
            "fingerprint_commands": context["fingerprint_commands"],
            "artifact_size_commands": size_commands,
        },
        "scenarios": scenarios,
        "artifact_sizes": sizes,
        "budget_verdicts": budget_verdicts(scenarios, memory_budgets),
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.parent / f".{args.output.name}.{uuid.uuid4().hex}.tmp"
    try:
        temporary.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        run_checked(["node", str(VALIDATOR_PATH), str(temporary)], timeout=30)
        try:
            os.link(temporary, args.output)
        except FileExistsError as error:
            raise CaptureError(f"refusing to overwrite existing evidence: {args.output}") from error
    finally:
        temporary.unlink(missing_ok=True)

    failures = [verdict["budget"] for verdict in evidence["budget_verdicts"] if verdict["status"] == "fail"]
    print(f"PASS: validated B1 performance evidence written to {args.output}")
    if failures:
        print(f"BUDGET_FAILURES: {', '.join(failures)}")
        raise SystemExit(1)


def self_test() -> None:
    run_checked(["node", str(VALIDATOR_PATH), "--self-test"])
    print("PASS: B1 benchmark harness validator self-test")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(
        description="Capture B1 Fasti native process-tree and OCI cgroup-v2 performance evidence."
    )
    subcommands = root.add_subparsers(dest="command", required=True)
    subcommands.add_parser("self-test", help="run portable schema and negative-sentinel tests")

    capture_parser = subcommands.add_parser(
        "capture",
        help="capture complete evidence; Linux, route-less netns, Docker network-none, and cgroup v2 are mandatory",
    )
    capture_parser.add_argument("--native-binary", type=Path, required=True)
    capture_parser.add_argument("--image", required=True)
    capture_parser.add_argument(
        "--hardware-profile",
        required=True,
        choices=[
            "raspberry_pi_5_champion",
            "j4125_calibrated",
            "ugoos_am6b_plus",
            "xiaomi_box_m3",
            "nvidia_shield",
            "representative_tv",
        ],
    )
    capture_parser.add_argument("--runner-id", required=True, help="stable non-secret label for this exact runner")
    capture_parser.add_argument("--custodian", required=True, help="person or team accountable for the physical run")
    capture_parser.add_argument("--contract-ref", required=True, help="immutable digest or Git object for the contract set")
    capture_parser.add_argument("--output", type=Path, required=True)
    capture_parser.add_argument("--repetitions", type=int, default=5)
    capture_parser.add_argument("--steady-window-seconds", type=float, default=5.0)
    capture_parser.add_argument("--sample-interval-ms", type=int, default=10)
    capture_parser.add_argument("--startup-timeout-seconds", type=float, default=15.0)
    return root


def validate_arguments(args: argparse.Namespace) -> None:
    if args.command != "capture":
        return
    args.native_binary = args.native_binary.resolve()
    args.output = args.output.resolve()
    if args.repetitions < 3:
        raise CaptureError("at least three repetitions are required")
    if args.steady_window_seconds < 3:
        raise CaptureError("steady measurement window must be at least three seconds")
    if not 1 <= args.sample_interval_ms <= 250:
        raise CaptureError("sample interval must be between 1 and 250 milliseconds")
    if args.startup_timeout_seconds <= 0:
        raise CaptureError("startup timeout must be positive")
    for label, value in [
        ("runner ID", args.runner_id),
        ("custodian", args.custodian),
        ("contract reference", args.contract_ref),
        ("image reference", args.image),
    ]:
        if not value.strip():
            raise CaptureError(f"{label} must not be empty")
    if re.fullmatch(r"(?:[0-9a-f]{40}|sha256:[0-9a-f]{64})", args.contract_ref) is None:
        raise CaptureError("contract reference must be a full 40-hex Git object or sha256:<64 lowercase hex>")


def main() -> None:
    args = parser().parse_args()
    try:
        validate_arguments(args)
        if args.command == "self-test":
            self_test()
        else:
            capture(args)
    except CaptureError as error:
        print(f"REFUSED: {error}", file=sys.stderr)
        raise SystemExit(2) from error


if __name__ == "__main__":
    main()
