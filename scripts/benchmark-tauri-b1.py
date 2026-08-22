#!/usr/bin/env python3
"""Capture a benchmark-only empty Tauri shell in a Linux cgroup-v2 scope.

This is packaging evidence, not a Fasti product surface. Qualifying capture is
allowed only from an exact clean commit and builds the locked fixture locally.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import platform
import re
import shlex
import shutil
import stat
import statistics
import subprocess
import sys
import tempfile
import time
import tomllib
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "benchmarks" / "b1" / "tauri-shell"
MANIFEST = FIXTURE / "src-tauri" / "Cargo.toml"
LOCKFILE = FIXTURE / "src-tauri" / "Cargo.lock"
BINARY = FIXTURE / "src-tauri" / "target" / "release" / "fasti-b1-tauri-shell"
VALIDATOR = FIXTURE / "validate-evidence.mjs"
PERFORMANCE_HARNESS = ROOT / "scripts" / "benchmark-b1.py"
TAURI_CONFIG = FIXTURE / "src-tauri" / "tauri.conf.json"
INDEX_HTML = FIXTURE / "dist" / "index.html"
FIXTURE_POLICY = FIXTURE / "fixture-policy.json"
FIXTURE_POLICY_SCHEMA = FIXTURE / "fixture-policy.schema.json"
EVIDENCE_ROOT = FIXTURE / "evidence"
ARTIFACT_ROOT = EVIDENCE_ROOT / "artifacts"
HARNESS_VERSION = "fasti-b1-tauri-shell.v1"
ABSOLUTE_CEILING_BYTES = 192 * 1024 * 1024
LOW_RAM_TARGET_BYTES = 96 * 1024 * 1024
PLACEHOLDERS = {"runner", "runner-id", "self-test", "todo", "tbd", "unknown"}


class CaptureError(RuntimeError):
    """A missing prerequisite or ambiguous observation invalidated capture."""


def command_text(parts: list[str | Path]) -> str:
    return shlex.join(str(part) for part in parts)


def run_checked(parts: list[str | Path], *, timeout: float = 1200) -> str:
    result = subprocess.run(
        [str(part) for part in parts],
        cwd=ROOT,
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
    )
    if result.returncode != 0:
        diagnostic = result.stderr.strip() or result.stdout.strip() or "no diagnostic"
        raise CaptureError(f"command failed ({command_text(parts)}): {diagnostic}")
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


def read_regular_file_once(path: Path) -> bytes:
    """Read an exact regular file through a no-follow descriptor."""
    nofollow = getattr(os, "O_NOFOLLOW", None)
    if nofollow is None:
        raise CaptureError("qualifying artifact capture requires O_NOFOLLOW")
    try:
        descriptor = os.open(path, os.O_RDONLY | nofollow)
    except OSError as error:
        raise CaptureError(f"artifact cannot be opened without following links: {path}") from error
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode):
            raise CaptureError(f"artifact is not a regular file: {path}")
        with os.fdopen(os.dup(descriptor), "rb") as handle:
            return handle.read()
    finally:
        os.close(descriptor)


def fingerprint_regular_file(path: Path) -> dict[str, Any]:
    """Hash a retained large file through one stable no-follow descriptor."""
    nofollow = getattr(os, "O_NOFOLLOW", None)
    if nofollow is None:
        raise CaptureError("qualifying retained-image capture requires O_NOFOLLOW")
    absolute = Path(os.path.abspath(path))
    try:
        descriptor = os.open(absolute, os.O_RDONLY | nofollow)
    except OSError as error:
        raise CaptureError(
            f"retained OS image cannot be opened without following links: {absolute}"
        ) from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_size < 1:
            raise CaptureError("retained OS image must be a nonempty regular file")
        digest = hashlib.sha256()
        with os.fdopen(os.dup(descriptor), "rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        after = os.fstat(descriptor)
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
        ):
            raise CaptureError("retained OS image changed while it was fingerprinted")
        if not absolute.name or "/" in absolute.name:
            raise CaptureError("retained OS image filename is invalid")
        return {
            "file_name": absolute.name,
            "size_bytes": before.st_size,
            "sha256": digest.hexdigest(),
        }
    finally:
        os.close(descriptor)


def require_private_evidence_path(output: Path) -> Path:
    """Constrain qualifying receipts to the private benchmark evidence package."""
    absolute = Path(os.path.abspath(output))
    if absolute.parent != EVIDENCE_ROOT or not re.fullmatch(
        r"[a-z0-9][a-z0-9._-]{0,126}\.json", absolute.name
    ):
        raise CaptureError(
            "qualifying Tauri evidence output must be a safe JSON filename directly under "
            "benchmarks/b1/tauri-shell/evidence"
        )
    for directory in (EVIDENCE_ROOT, ARTIFACT_ROOT):
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        if directory.is_symlink() or not directory.is_dir():
            raise CaptureError(f"private evidence directory is unsafe: {directory}")
    return absolute


def retain_artifact(binary_bytes: bytes) -> tuple[Path, bool]:
    """Publish exact measured bytes once under their content digest."""
    digest = hashlib.sha256(binary_bytes).hexdigest()
    artifact = ARTIFACT_ROOT / f"sha256-{digest}-fasti-b1-tauri-shell"
    nofollow = getattr(os, "O_NOFOLLOW", None)
    if nofollow is None:
        raise CaptureError("qualifying artifact retention requires O_NOFOLLOW")
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(dir=ARTIFACT_ROOT, delete=False) as handle:
            temporary = Path(handle.name)
            handle.write(binary_bytes)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary, 0o600, follow_symlinks=False)
        try:
            os.link(temporary, artifact, follow_symlinks=False)
        except FileExistsError:
            if read_regular_file_once(artifact) != binary_bytes:
                raise CaptureError("content-addressed Tauri artifact contains different bytes")
            return artifact, False
        return artifact, True
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def remove_new_artifact(path: Path, created: bool) -> None:
    if not created:
        return
    try:
        if path.parent == ARTIFACT_ROOT and not path.is_symlink() and path.is_file():
            path.unlink()
    except FileNotFoundError:
        pass


def validate_runner_id(value: str) -> str:
    normalized = value.strip().casefold()
    if normalized in PLACEHOLDERS or not re.fullmatch(r"[a-z0-9][a-z0-9_-]{2,63}", normalized):
        raise CaptureError(
            "runner ID must be a stable 3-64 character lowercase lab identifier, not a placeholder"
        )
    return normalized


def clean_source_identity() -> dict[str, str]:
    if run_checked(["git", "status", "--porcelain=v1", "--untracked-files=all"]):
        raise CaptureError("Tauri evidence requires a clean source tree")
    return {
        "git_commit": run_checked(["git", "rev-parse", "HEAD"]),
        "git_tree": run_checked(["git", "rev-parse", "HEAD^{tree}"]),
        "fixture_tree": run_checked(
            ["git", "rev-parse", "HEAD:benchmarks/b1/tauri-shell"]
        ),
    }


def validate_fixture_policy(
    config: dict[str, Any],
    manifest: dict[str, Any],
    html: str,
    *,
    main_source: str | None = None,
    require_tracked: bool = False,
) -> dict[str, Any]:
    run_checked(["node", str(VALIDATOR.relative_to(ROOT)), "--policy"], timeout=30)
    policy = json.loads(FIXTURE_POLICY.read_text(encoding="utf-8"))
    for relative, expected_digest in policy["tracked_inputs"].items():
        source = FIXTURE / relative
        if not source.is_file() or sha256_file(source) != expected_digest:
            raise CaptureError(
                f"benchmark fixture input differs from canonical policy: {relative}"
            )
        if require_tracked:
            run_checked(
                ["git", "cat-file", "-e", f"HEAD:{source.relative_to(ROOT)}"],
                timeout=10,
            )
    if require_tracked:
        for source in [FIXTURE_POLICY, FIXTURE_POLICY_SCHEMA]:
            run_checked(
                ["git", "cat-file", "-e", f"HEAD:{source.relative_to(ROOT)}"],
                timeout=10,
            )
    windows = config.get("app", {}).get("windows")
    if not isinstance(windows, list) or len(windows) != 1:
        raise CaptureError("benchmark fixture must define exactly one hidden window")
    window = windows[0]
    if window.get("visible") is not False:
        raise CaptureError("benchmark fixture window must remain hidden")
    if window.get("fullscreen") is not False or window.get("resizable") is not False:
        raise CaptureError("benchmark fixture window must remain inert")
    if config.get("bundle", {}).get("active") is not False:
        raise CaptureError("benchmark fixture bundling must remain disabled")
    if config.get("build", {}).get("frontendDist") != "../dist":
        raise CaptureError("benchmark fixture must load only its local static asset")
    package = manifest.get("package", {})
    if package.get("name") != "fasti-b1-tauri-shell" or package.get("publish") is not False:
        raise CaptureError("benchmark fixture must remain isolated and non-publishable")
    if "workspace" not in manifest or manifest.get("dependencies") != {
        "tauri": {
            "version": "=2.11.5",
            "default-features": False,
            "features": ["wry"],
        }
    }:
        raise CaptureError("benchmark fixture dependency boundary changed")
    if manifest.get("build-dependencies") != {"tauri-build": "=2.6.3"}:
        raise CaptureError("benchmark fixture build dependency boundary changed")
    if re.search(r"<(script|a|button|form|input|img)\b", html, re.IGNORECASE):
        raise CaptureError("benchmark fixture static asset gained an interactive/product surface")
    body = re.search(r"<body[^>]*>(.*?)</body>", html, re.IGNORECASE | re.DOTALL)
    if body is None or body.group(1).strip():
        raise CaptureError("benchmark fixture body must remain empty")
    main_source = main_source or (FIXTURE / "src-tauri" / "src" / "main.rs").read_text(
        encoding="utf-8"
    )
    if re.search(
        r"(?:#\s*\[\s*tauri::command|generate_handler!|invoke_handler|\.plugin\s*\()",
        main_source,
    ):
        raise CaptureError("benchmark fixture gained a command, invoke handler, or plugin")
    return {
        "benchmark_only": True,
        "product_surface": False,
        "window_visible": window["visible"],
        "served_web": "not_applicable",
        "design_review": "not_applicable_non_product_hidden_benchmark_fixture",
        "qualifying_runner": "governed_linux_desktop_cgroup_v2",
        "measurement_boundary": "dedicated_transient_user_scope",
        "fixture_policy_sha256": sha256_file(FIXTURE_POLICY),
    }


def fixture_scope(*, require_tracked: bool = False) -> dict[str, Any]:
    return validate_fixture_policy(
        json.loads(TAURI_CONFIG.read_text(encoding="utf-8")),
        tomllib.loads(MANIFEST.read_text(encoding="utf-8")),
        INDEX_HTML.read_text(encoding="utf-8"),
        require_tracked=require_tracked,
    )


def performance_environment_module() -> Any:
    spec = importlib.util.spec_from_file_location(
        "fasti_b1_performance_environment", PERFORMANCE_HARNESS
    )
    if spec is None or spec.loader is None:
        raise CaptureError("could not load the canonical B1 environment collectors")
    module = importlib.util.module_from_spec(spec)
    previous = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


def collect_environment(os_image_path: Path) -> tuple[dict[str, Any], Any]:
    retained_image = fingerprint_regular_file(os_image_path)
    collector = performance_environment_module()
    os_image = collector.parse_os_image()
    storage, _ = collector.parse_storage_identity()
    webkit_version = run_checked(
        ["pkg-config", "--modversion", "webkit2gtk-4.1"], timeout=10
    )
    environment = {
        "os_image": {
            **os_image,
            "claim_scope": "runtime_os_release_fields_only",
            "retained_image": retained_image,
            "approval": "retained_digest_recorded_no_profile_allowlist",
        },
        "firmware": collector.parse_firmware_identity("packaging_reference"),
        "root_filesystem": {
            "source": storage["root_source"],
            "type": storage["root_filesystem_type"],
            "mount_options": storage["root_mount_options"],
        },
        "storage": storage,
        "cpu_governor": collector.parse_cpu_governors(),
        "temperature": {
            "preflight": collector.parse_temperature(),
            "post_capture": None,
        },
        "container_runtime": {
            "status": "not_applicable",
            "reason": "The benchmark fixture is a native Tauri process, not an OCI subject.",
        },
        "cgroup": {
            "version": "v2",
            "manager": "systemd_user_transient_scope",
            "controller": "memory",
        },
        "corpus": {
            "status": "not_applicable",
            "seed": None,
            "digest": None,
            "reason": "The empty shell has no generated records or workload corpus.",
        },
        "webkit_runtime": {
            "package": "webkit2gtk-4.1",
            "version": webkit_version,
        },
        "fingerprint_commands": [
            "read /etc/os-release",
            "read /sys/class/dmi/id/{bios_vendor,bios_version,bios_date}",
            "read /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor",
            "read /sys/class/thermal/thermal_zone*/{type,temp}",
            command_text(["findmnt", "-n", "-o", "SOURCE,FSTYPE,OPTIONS", "-T", str(ROOT)]),
            "read lsblk root-device parent chain and hash stable identifiers",
            command_text(["pkg-config", "--modversion", "webkit2gtk-4.1"]),
            "read /sys/fs/cgroup/cgroup.controllers and transient scope memory files",
        ],
    }
    return environment, collector


def validate_display_evidence(
    *,
    session_id: str,
    session_type: str,
    session_remote: str,
    session_class: str,
    session_state: str,
    seat: str,
    connected_drm_connectors: list[str],
    process_inventory: str,
    wayland: bool,
    x11: bool,
) -> dict[str, Any]:
    if not re.fullmatch(r"[A-Za-z0-9._-]+", session_id):
        raise CaptureError("qualifying Tauri capture requires a safe XDG_SESSION_ID")
    if session_type not in {"wayland", "x11"}:
        raise CaptureError(
            f"unsupported_display_stack: login session type is {session_type!r}, not Wayland or X11"
        )
    if session_remote != "no" or session_class != "user" or session_state != "active":
        raise CaptureError(
            "qualifying Tauri capture requires an active local graphical user session"
        )
    if not seat:
        raise CaptureError("qualifying Tauri capture requires a local display seat")
    if not connected_drm_connectors:
        raise CaptureError(
            "qualifying Tauri capture requires at least one connected physical DRM display connector"
        )
    simulated = re.compile(
        r"(?im)(?:^|[/ ])(?:Xvfb|Xdummy)(?:\s|$)|"
        r"\bweston\b[^\n]*(?:--backend(?:=|\s+)headless|headless-backend\.so)"
    )
    if simulated.search(process_inventory):
        raise CaptureError(
            "unsupported_display_stack: simulated Xvfb/Xdummy/headless-Weston process detected"
        )
    if session_type == "wayland" and not wayland:
        raise CaptureError("login session reports Wayland but WAYLAND_DISPLAY is unavailable")
    if session_type == "x11" and (not x11 or wayland):
        raise CaptureError("login session reports X11 but its display environment is inconsistent")
    display_server = "wayland_and_x11" if wayland and x11 else session_type
    return {
        "session_id": session_id,
        "session_type": session_type,
        "session_class": session_class,
        "session_remote": False,
        "session_state": session_state,
        "seat": seat,
        "connected_drm_connectors": sorted(connected_drm_connectors),
        "simulation_scan": "none_detected",
        "display_server": display_server,
    }


def display_session() -> dict[str, Any]:
    if platform.system() != "Linux":
        raise CaptureError(
            "qualifying Tauri capture requires a governed Linux desktop cgroup-v2 "
            "runner; macOS WebKit XPC helpers cannot be attributed by PPID"
        )
    session_id = os.environ.get("XDG_SESSION_ID", "")
    wayland = bool(os.environ.get("WAYLAND_DISPLAY") and os.environ.get("XDG_RUNTIME_DIR"))
    x11 = bool(os.environ.get("DISPLAY"))
    if not wayland and not x11:
        raise CaptureError(
            "unsupported_display_stack: qualifying Tauri capture requires a real "
            "Wayland or X11 desktop and refuses simulated Xvfb/Weston evidence"
        )
    if not re.fullmatch(r"[A-Za-z0-9._-]+", session_id):
        raise CaptureError("qualifying Tauri capture requires a safe XDG_SESSION_ID")
    for command in ["loginctl", "ps"]:
        require_command(command)
    session = {
        key: run_checked(
            ["loginctl", "show-session", session_id, f"--property={property_name}", "--value"],
            timeout=10,
        )
        for key, property_name in [
            ("session_type", "Type"),
            ("session_remote", "Remote"),
            ("session_class", "Class"),
            ("session_state", "State"),
            ("seat", "Seat"),
        ]
    }
    connectors = [
        path.parent.name
        for path in sorted(Path("/sys/class/drm").glob("*/status"))
        if path.read_text(encoding="ascii").strip() == "connected"
    ]
    processes = run_checked(["ps", "-eo", "comm=,args="], timeout=10)
    return validate_display_evidence(
        session_id=session_id,
        connected_drm_connectors=connectors,
        process_inventory=processes,
        wayland=wayland,
        x11=x11,
        **session,
    )


def runner_fingerprint(runner_id: str, display_evidence: dict[str, Any]) -> dict[str, Any]:
    release = Path("/etc/os-release")
    if not release.is_file():
        raise CaptureError("Linux runner lacks /etc/os-release")
    fields = {}
    for line in release.read_text(encoding="utf-8").splitlines():
        if "=" in line and not line.startswith("#"):
            key, value = line.split("=", 1)
            fields[key] = value.strip().strip('"')
    os_version = fields.get("PRETTY_NAME", "")
    if not os_version:
        raise CaptureError("Linux PRETTY_NAME is unavailable")
    cpu_model = ""
    for line in Path("/proc/cpuinfo").read_text(
        encoding="utf-8", errors="replace"
    ).splitlines():
        key, separator, value = line.partition(":")
        if separator and key.strip().casefold() in {"model name", "model", "hardware"}:
            cpu_model = value.strip()
            if cpu_model:
                break
    if not cpu_model:
        raise CaptureError("Linux CPU model is unavailable")
    total_memory = 0
    for line in Path("/proc/meminfo").read_text(encoding="utf-8").splitlines():
        if line.startswith("MemTotal:"):
            total_memory = int(line.split()[1]) * 1024
            break
    if not total_memory:
        raise CaptureError("Linux total memory is unavailable")
    return {
        "runner_id": runner_id,
        "os": "linux",
        "os_version": os_version,
        "kernel": platform.release(),
        "architecture": platform.machine(),
        "cpu_model": cpu_model,
        "logical_cpu_count": os.cpu_count() or 1,
        "total_memory_bytes": total_memory,
        "display_server": display_evidence["display_server"],
        "display_evidence": {
            key: value
            for key, value in display_evidence.items()
            if key != "display_server"
        },
        "systemd_user_scope": True,
    }


def validate_control_group(control_group: str, unit: str) -> Path:
    if not control_group.startswith("/") or ".." in Path(control_group).parts:
        raise CaptureError(f"systemd returned an unsafe cgroup path: {control_group!r}")
    if Path(control_group).name != unit:
        raise CaptureError(
            f"systemd cgroup {control_group!r} does not belong to expected unit {unit!r}"
        )
    cgroup_root = Path("/sys/fs/cgroup").resolve()
    candidate = (cgroup_root / control_group.lstrip("/")).resolve()
    if candidate != cgroup_root and cgroup_root not in candidate.parents:
        raise CaptureError("systemd cgroup escaped the cgroup-v2 filesystem")
    return candidate


def resolve_scope_cgroup(unit: str) -> Path:
    control_group = run_checked(
        ["systemctl", "--user", "show", unit, "--property=ControlGroup", "--value"],
        timeout=10,
    )
    if not control_group:
        raise CaptureError(f"systemd user scope {unit} has no control group")
    path = validate_control_group(control_group, unit)
    for filename in ["memory.current", "memory.peak", "cgroup.procs"]:
        if not (path / filename).is_file():
            raise CaptureError(f"cgroup-v2 scope lacks {filename}: {path}")
    return path


def cgroup_usage(path: Path) -> tuple[int, int, int]:
    try:
        current = int((path / "memory.current").read_text(encoding="ascii").strip())
        peak = int((path / "memory.peak").read_text(encoding="ascii").strip())
        processes = [
            line
            for line in (path / "cgroup.procs").read_text(encoding="ascii").splitlines()
            if line.strip()
        ]
    except (FileNotFoundError, PermissionError, ValueError) as error:
        raise CaptureError(f"could not read cgroup-v2 memory evidence from {path}") from error
    if current < 1 or peak < current or not processes:
        raise CaptureError(
            f"invalid cgroup-v2 observation current={current}, peak={peak}, "
            f"processes={len(processes)}"
        )
    return current, peak, len(processes)


def stop_scope(
    unit: str, process: subprocess.Popen[bytes], cgroup_path: Path | None
) -> None:
    subprocess.run(
        ["systemctl", "--user", "kill", "--signal=TERM", "--kill-whom=all", unit],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    subprocess.run(
        ["systemctl", "--user", "stop", unit],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if process.poll() is None:
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            subprocess.run(
                ["systemctl", "--user", "kill", "--signal=KILL", "--kill-whom=all", unit],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )
            process.kill()
            process.wait(timeout=5)
    state = subprocess.run(
        ["systemctl", "--user", "is-active", unit],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    ).stdout.strip()
    if state in {"active", "activating", "deactivating"}:
        raise CaptureError(f"Tauri benchmark scope did not stop cleanly: {unit} is {state}")
    if cgroup_path is not None and cgroup_path.exists():
        remaining = (cgroup_path / "cgroup.procs").read_text(encoding="ascii").strip()
        if remaining:
            raise CaptureError(
                f"Tauri benchmark scope retained processes after cleanup: {remaining}"
            )


def capture_once(
    run: int,
    *,
    steady_window_seconds: float,
    sample_interval_ms: int,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="fasti-tauri-b1-") as temporary:
        ready_path = Path(temporary) / "ready"
        environment = os.environ.copy()
        environment["FASTI_TAURI_BENCHMARK_READY_FILE"] = str(ready_path)
        unit = f"fasti-b1-tauri-{uuid.uuid4().hex}.scope"
        run_command = [
            "systemd-run",
            "--user",
            "--scope",
            "--quiet",
            f"--unit={unit}",
            "--property=MemoryAccounting=yes",
            "unshare",
            "--user",
            "--map-root-user",
            "--net",
            "--",
            str(BINARY),
        ]
        started = time.monotonic()
        with tempfile.TemporaryFile() as diagnostic_file:
            process = subprocess.Popen(
                run_command,
                cwd=ROOT,
                env=environment,
                stdout=subprocess.DEVNULL,
                stderr=diagnostic_file,
            )
            observations: list[tuple[float, int, int, int]] = []
            ready_at: float | None = None
            cgroup_path: Path | None = None
            try:
                deadline = started + 20
                while ready_at is None:
                    if process.poll() is not None:
                        diagnostic_file.seek(0)
                        diagnostic = diagnostic_file.read().decode(
                            "utf-8", errors="replace"
                        )
                        raise CaptureError(
                            f"Tauri run {run} exited before readiness: {diagnostic.strip()}"
                        )
                    now = time.monotonic()
                    if now > deadline:
                        raise CaptureError(
                            f"Tauri run {run} did not become ready within 20 seconds"
                        )
                    if cgroup_path is None:
                        try:
                            cgroup_path = resolve_scope_cgroup(unit)
                        except CaptureError:
                            time.sleep(sample_interval_ms / 1000)
                            continue
                    current, peak, count = cgroup_usage(cgroup_path)
                    observations.append((now, current, peak, count))
                    if (
                        ready_path.is_file()
                        and ready_path.read_text(encoding="utf-8") == "ready\n"
                    ):
                        ready_at = now
                        break
                    time.sleep(sample_interval_ms / 1000)
                steady_deadline = ready_at + steady_window_seconds
                while time.monotonic() < steady_deadline:
                    now = time.monotonic()
                    current, peak, count = cgroup_usage(cgroup_path)
                    observations.append((now, current, peak, count))
                    time.sleep(sample_interval_ms / 1000)
            finally:
                stop_scope(unit, process, cgroup_path)
        steady = [observation for observation in observations if observation[0] >= ready_at]
        if not steady:
            raise CaptureError(f"Tauri run {run} produced no steady-state observations")
        return {
            "run": run,
            "systemd_unit": unit,
            "ready_file": str(ready_path),
            "argv": run_command,
            "command": command_text(
                [f"FASTI_TAURI_BENCHMARK_READY_FILE={ready_path}", *run_command]
            ),
            "startup_ms": round((ready_at - started) * 1000, 3),
            "steady_cgroup_memory_bytes": max(item[1] for item in steady),
            "peak_cgroup_memory_bytes": max(item[2] for item in observations),
            "process_count_peak": max(item[3] for item in observations),
        }


def metric_summary(values: list[int | float]) -> dict[str, int | float]:
    return {
        "minimum": min(values),
        "median": statistics.median(values),
        "maximum": max(values),
    }


def capture(args: argparse.Namespace) -> int:
    runner_id = validate_runner_id(args.runner_id)
    display_evidence = display_session()
    for command in [
        "cargo",
        "findmnt",
        "git",
        "ip",
        "lsblk",
        "node",
        "pkg-config",
        "systemctl",
        "systemd-run",
        "unshare",
    ]:
        require_command(command)
    run_checked(["systemctl", "--user", "show-environment"], timeout=10)
    controllers = Path("/sys/fs/cgroup/cgroup.controllers")
    if not controllers.is_file() or "memory" not in controllers.read_text(encoding="ascii").split():
        raise CaptureError("qualifying Tauri capture requires the cgroup-v2 memory controller")
    if run_checked(
        ["unshare", "--user", "--map-root-user", "--net", "--", "ip", "route", "show"],
        timeout=10,
    ):
        raise CaptureError("route-less Tauri namespace preflight unexpectedly has an IP route")
    output = require_private_evidence_path(args.output)
    if output.exists():
        raise CaptureError(f"refusing to overwrite existing evidence: {output}")
    if args.repetitions < 5:
        raise CaptureError("qualifying short benchmarks require at least five repetitions")
    if args.steady_window_seconds < 3:
        raise CaptureError("steady window must be at least three seconds")
    if not 10 <= args.sample_interval_ms <= 250:
        raise CaptureError("sample interval must be between 10 and 250 milliseconds")

    source = clean_source_identity()
    scope = fixture_scope(require_tracked=True)
    environment, environment_collector = collect_environment(args.os_image_path)
    environment["fingerprint_commands"].extend(
        [
            "loginctl show-session $XDG_SESSION_ID --property={Type,Remote,Class,State,Seat} --value",
            "read /sys/class/drm/*/status and require a connected physical connector",
            "ps -eo comm=,args= and reject Xvfb, Xdummy, or headless Weston",
        ]
    )
    build_command = [
        "cargo",
        "build",
        "--manifest-path",
        str(MANIFEST.relative_to(ROOT)),
        "--release",
        "--locked",
        "--offline",
    ]
    run_checked(build_command)
    if not BINARY.is_file():
        raise CaptureError(f"locked build did not produce {BINARY.relative_to(ROOT)}")
    artifact_bytes = read_regular_file_once(BINARY)
    artifact_sha256 = hashlib.sha256(artifact_bytes).hexdigest()
    samples = [
        capture_once(
            run,
            steady_window_seconds=args.steady_window_seconds,
            sample_interval_ms=args.sample_interval_ms,
        )
        for run in range(1, args.repetitions + 1)
    ]
    environment["temperature"]["post_capture"] = environment_collector.parse_temperature()
    if clean_source_identity() != source:
        raise CaptureError("source identity changed during Tauri capture")
    if read_regular_file_once(BINARY) != artifact_bytes:
        raise CaptureError("Tauri artifact bytes changed during capture")
    retained_artifact, retained_artifact_created = retain_artifact(artifact_bytes)

    fields = [
        "startup_ms",
        "steady_cgroup_memory_bytes",
        "peak_cgroup_memory_bytes",
        "process_count_peak",
    ]
    summary = {field: metric_summary([sample[field] for sample in samples]) for field in fields}
    measured = int(summary["peak_cgroup_memory_bytes"]["maximum"])
    receipt = {
        "$schema": "https://fasti.scrobble.dev/schemas/benchmarks/b1/tauri-shell-evidence.schema.json",
        "schema_version": "fasti.b1.tauri-shell-evidence.v1",
        "body": "B1",
        "status": "complete",
        "captured_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "scope": scope,
        "source": {
            **source,
            "tree_state": "clean",
            "cargo_lock_sha256": sha256_file(LOCKFILE),
            "harness_script_sha256": sha256_file(Path(__file__).resolve()),
        },
        "runner": runner_fingerprint(runner_id, display_evidence),
        "environment": environment,
        "harness": {
            "version": HARNESS_VERSION,
            "repetitions": args.repetitions,
            "steady_window_seconds": args.steady_window_seconds,
            "sample_interval_ms": args.sample_interval_ms,
            "measurement_backend": "linux_cgroup_v2_memory_controller",
            "network_isolation": "route_less_user_network_namespace",
            "commands": [
                command_text(build_command),
                "per-run exact systemd-run commands are recorded with each raw sample",
            ],
        },
        "artifact": {
            "measurement_path": str(BINARY.relative_to(ROOT)),
            "path": str(retained_artifact.relative_to(ROOT)),
            "sha256": artifact_sha256,
            "size_bytes": len(artifact_bytes),
        },
        "samples": samples,
        "summary": summary,
        "verdict": {
            "budget": "absolute_ceiling",
            "low_ram_target_bytes": LOW_RAM_TARGET_BYTES,
            "low_ram_target_status": "pass" if measured <= LOW_RAM_TARGET_BYTES else "fail",
            "limit_bytes": ABSOLUTE_CEILING_BYTES,
            "measured_bytes": measured,
            "status": "pass" if measured <= ABSOLUTE_CEILING_BYTES else "fail",
            "disposition": (
                "within_low_ram_target"
                if measured <= LOW_RAM_TARGET_BYTES
                else "target_miss_requires_b8_tuning"
                if measured <= ABSOLUTE_CEILING_BYTES
                else "absolute_breach_blocks_b8"
            ),
            "effect": "failure_blocks_b8_packaging_not_b1_contracts",
        },
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w", encoding="utf-8", dir=output.parent, delete=False
    ) as handle:
        json.dump(receipt, handle, indent=2)
        handle.write("\n")
        temporary_output = Path(handle.name)
    published = False
    try:
        run_checked(["node", str(VALIDATOR.relative_to(ROOT)), str(temporary_output)])
        temporary_output.replace(output)
        published = True
    finally:
        if temporary_output.exists():
            temporary_output.unlink()
        if not published:
            remove_new_artifact(retained_artifact, retained_artifact_created)
    print(f"PASS: wrote Tauri cgroup-v2 evidence {output}")
    return 0 if receipt["verdict"]["status"] == "pass" else 1


def self_test() -> None:
    require_command("node")
    run_checked(["node", str(VALIDATOR.relative_to(ROOT)), "--self-test"])
    fixture_scope()
    assert validate_control_group(
        "/user.slice/user-1000.slice/user@1000.service/app.slice/"
        "fasti-b1-tauri-" + "1" * 32 + ".scope",
        "fasti-b1-tauri-" + "1" * 32 + ".scope",
    ).name == "fasti-b1-tauri-" + "1" * 32 + ".scope"
    assert metric_summary([3, 1, 2, 5, 4]) == {
        "minimum": 1,
        "median": 3,
        "maximum": 5,
    }
    try:
        validate_runner_id("runner-id")
    except CaptureError:
        pass
    else:
        raise AssertionError("placeholder runner ID passed")
    config = json.loads(TAURI_CONFIG.read_text(encoding="utf-8"))
    config["app"]["windows"][0]["visible"] = True
    try:
        validate_fixture_policy(
            config,
            tomllib.loads(MANIFEST.read_text(encoding="utf-8")),
            INDEX_HTML.read_text(encoding="utf-8"),
        )
    except CaptureError:
        pass
    else:
        raise AssertionError("visible benchmark window passed fixture policy")
    print("PASS: Tauri capture harness self-test")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("self-test", help="run portable harness sentinels")
    subparsers.add_parser(
        "policy-check", help="verify the canonical hidden fixture policy and tracked inputs"
    )
    capture_parser = subparsers.add_parser("capture", help="capture a clean local receipt")
    capture_parser.add_argument("--runner-id", required=True)
    capture_parser.add_argument("--os-image-path", required=True, type=Path)
    capture_parser.add_argument("--output", required=True, type=Path)
    capture_parser.add_argument("--repetitions", type=int, default=5)
    capture_parser.add_argument("--steady-window-seconds", type=float, default=3)
    capture_parser.add_argument("--sample-interval-ms", type=int, default=25)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "self-test":
            self_test()
            return 0
        if args.command == "policy-check":
            scope = fixture_scope(require_tracked=True)
            print(json.dumps({"status": "pass", "scope": scope}, indent=2))
            return 0
        return capture(args)
    except (CaptureError, OSError, subprocess.TimeoutExpired) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
