#!/usr/bin/env python3
"""Verify Fasti's exact TrailBase release inputs.

This module is an internal helper for ``scripts/dev.sh`` and xtask. It does not
own process supervision or expose a second developer launcher.
"""

from __future__ import annotations

import argparse
import atexit
import contextlib
import copy
import datetime
import fcntl
import hashlib
import ipaddress
import json
import os
import platform
import re
import secrets
import shutil
import signal
import stat
import subprocess  # nosec B404 -- the governed launcher must execute exact verified binaries.
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
RELEASE_PATH = ROOT / "third_party" / "trailbase" / "release.json"
SHA256 = re.compile(r"^[0-9a-f]{64}$")
OCI_DIGEST = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
VERSION = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
BOOTSTRAP_PASSWORD = re.compile(r"^\s*password:\s*'([^']+)'\s*$")
TRAILBASE_LICENSE_SHA256 = "be4741d827008446e5e8bf9ee42f9e57b57245b6aed260fbdaf00ffebe958fb7"
TRAILBASE_REPOSITORY_URL = "https://github.com/trailbaseio/trailbase"
_MANAGED_PROCESS_GROUPS: dict[int, subprocess.Popen[Any]] = {}
_STARTING_PROCESS_GROUPS = 0
_PENDING_TERMINATION_SIGNAL: int | None = None


class ReleaseError(ValueError):
    """The checked-in release lock or a supplied artifact is invalid."""


def start_managed_process_group(
    command: list[str | Path],
    *,
    environment: dict[str, str],
    stdout: int | None,
    stderr: int | None,
    text: bool = False,
) -> subprocess.Popen[Any]:
    """Start and register one child group before cancellation can be delivered."""
    global _STARTING_PROCESS_GROUPS
    _STARTING_PROCESS_GROUPS += 1
    try:
        process = subprocess.Popen(  # nosec -- nosemgrep -- callers supply digest-verified executables and fixed argv.
            command,  # nosemgrep -- governed local command vectors only; no shell.
            env=environment,
            stdout=stdout,
            stderr=stderr,
            text=text,
            start_new_session=True,
        )
        _MANAGED_PROCESS_GROUPS[process.pid] = process
    finally:
        _STARTING_PROCESS_GROUPS -= 1
        if _STARTING_PROCESS_GROUPS == 0 and _PENDING_TERMINATION_SIGNAL is not None:
            _terminate_managed_process_groups(_PENDING_TERMINATION_SIGNAL, None)
    return process


def _process_group_exists(process_group: int) -> bool:
    try:
        os.killpg(process_group, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def stop_managed_process_group(process: subprocess.Popen[Any]) -> None:
    try:
        if _process_group_exists(process.pid):
            with contextlib.suppress(ProcessLookupError):
                os.killpg(process.pid, signal.SIGTERM)
        if process.poll() is None:
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                pass
        if _process_group_exists(process.pid):
            with contextlib.suppress(ProcessLookupError):
                os.killpg(process.pid, signal.SIGKILL)
            if process.poll() is None:
                process.wait(timeout=5)
            deadline = time.monotonic() + 5
            while _process_group_exists(process.pid) and time.monotonic() < deadline:
                time.sleep(0.05)
        if _process_group_exists(process.pid):
            raise ReleaseError(f"could not stop managed process group {process.pid}")
        if process.poll() is None:
            raise ReleaseError(f"could not reap managed process {process.pid}")
    finally:
        if not _process_group_exists(process.pid):
            _MANAGED_PROCESS_GROUPS.pop(process.pid, None)


def _cleanup_managed_process_groups() -> None:
    for process in tuple(_MANAGED_PROCESS_GROUPS.values()):
        try:
            stop_managed_process_group(process)
        except (OSError, ReleaseError, subprocess.SubprocessError):
            continue


def _terminate_managed_process_groups(signum: int, _frame: Any) -> None:
    global _PENDING_TERMINATION_SIGNAL
    _PENDING_TERMINATION_SIGNAL = signum
    if _STARTING_PROCESS_GROUPS:
        return
    _PENDING_TERMINATION_SIGNAL = None
    raise SystemExit(128 + signum)


def install_termination_cleanup() -> None:
    atexit.unregister(_cleanup_managed_process_groups)
    atexit.register(_cleanup_managed_process_groups)
    signal.signal(signal.SIGINT, _terminate_managed_process_groups)
    signal.signal(signal.SIGTERM, _terminate_managed_process_groups)


def _open_safe_regular_file(path: Path) -> int:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        metadata = os.fstat(descriptor)
        if stat.S_ISREG(metadata.st_mode):
            return descriptor
    except OSError:
        os.close(descriptor)
        raise
    os.close(descriptor)
    raise ReleaseError(f"not a regular file: {path}")


def _read_regular_file(path: Path, maximum_bytes: int) -> bytes:
    descriptor = _open_safe_regular_file(path)
    try:
        metadata = os.fstat(descriptor)
        if metadata.st_size > maximum_bytes:
            raise ReleaseError(f"file exceeds {maximum_bytes} bytes: {path}")
        with os.fdopen(os.dup(descriptor), "rb") as source:
            return source.read()
    finally:
        os.close(descriptor)


def load_release(path: Path = RELEASE_PATH) -> dict[str, Any]:
    try:
        release = json.loads(_read_regular_file(path, 64 * 1024))
    except (OSError, json.JSONDecodeError) as error:
        raise ReleaseError(f"cannot read TrailBase release lock: {error}") from error
    validate_release(release)
    return release


def _require_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        raise ReleaseError(
            f"{label} fields differ: missing={sorted(expected - actual)}, "
            f"unexpected={sorted(actual - expected)}"
        )


def _require_https(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.startswith("https://"):
        raise ReleaseError(f"{label} must be an HTTPS URL")
    if any(character.isspace() for character in value) or "@" in value.split("//", 1)[1]:
        raise ReleaseError(f"{label} must not contain credentials or whitespace")
    return value


def validate_release(release: Any) -> None:
    if not isinstance(release, dict):
        raise ReleaseError("release lock must be a JSON object")
    _require_keys(
        release,
        {
            "schema_version",
            "version",
            "tag",
            "tag_commit",
            "release_url",
            "source_url",
            "license",
            "expected_version_line",
            "upgrade_fixture",
            "native",
            "oci",
        },
        "release",
    )
    if release["schema_version"] != "fasti.trailbase-release.v1":
        raise ReleaseError("unsupported TrailBase release-lock schema")
    version = release["version"]
    if not isinstance(version, str) or not VERSION.fullmatch(version):
        raise ReleaseError("version must contain three numeric components")
    if release["tag"] != f"v{version}":
        raise ReleaseError("tag must equal v plus the exact version")
    if not isinstance(release["tag_commit"], str) or not COMMIT.fullmatch(release["tag_commit"]):
        raise ReleaseError("tag_commit must be a lowercase 40-character commit")
    release_url = _require_https(release["release_url"], "release_url")
    source_url = _require_https(release["source_url"], "source_url")
    if release_url != f"{TRAILBASE_REPOSITORY_URL}/releases/tag/v{version}":
        raise ReleaseError("release_url does not name the exact version")
    if source_url != f"{TRAILBASE_REPOSITORY_URL}/tree/v{version}":
        raise ReleaseError("source_url does not name the exact version")

    license_info = release["license"]
    if not isinstance(license_info, dict):
        raise ReleaseError("license must be an object")
    _require_keys(
        license_info,
        {"spdx", "file_sha256", "source_url", "integration"},
        "license",
    )
    if license_info["spdx"] != "OSL-3.0":
        raise ReleaseError("TrailBase v0.33.5 must retain its OSL-3.0 disposition")
    if license_info["integration"] != "separate-unmodified-process":
        raise ReleaseError("TrailBase must remain a separate, unmodified process")
    if license_info["file_sha256"] != TRAILBASE_LICENSE_SHA256:
        raise ReleaseError("TrailBase licence differs from the reviewed v0.33.5 text")
    if _require_https(license_info["source_url"], "license.source_url") != (
        f"{TRAILBASE_REPOSITORY_URL}/blob/v{version}/LICENSE"
    ):
        raise ReleaseError("license.source_url does not name the reviewed release licence")

    expected_version = release["expected_version_line"]
    if (
        not isinstance(expected_version, str)
        or not expected_version.startswith(f"trail v{version}-")
        or f"-g{release['tag_commit'][:8]} " not in expected_version
    ):
        raise ReleaseError("expected_version_line does not identify the exact release")

    _validate_native_release(release, "release")

    upgrade_fixture = release["upgrade_fixture"]
    if not isinstance(upgrade_fixture, dict):
        raise ReleaseError("upgrade_fixture must be an object")
    _require_keys(
        upgrade_fixture,
        {
            "version",
            "tag",
            "tag_commit",
            "release_url",
            "source_url",
            "expected_version_line",
            "native",
        },
        "upgrade_fixture",
    )
    fixture_version = upgrade_fixture["version"]
    if not isinstance(fixture_version, str) or not VERSION.fullmatch(fixture_version):
        raise ReleaseError("upgrade_fixture.version must contain three numeric components")
    if tuple(map(int, fixture_version.split("."))) >= tuple(map(int, version.split("."))):
        raise ReleaseError("upgrade fixture must be older than the selected release")
    if upgrade_fixture["tag"] != f"v{fixture_version}":
        raise ReleaseError("upgrade_fixture.tag must equal v plus its exact version")
    if not isinstance(upgrade_fixture["tag_commit"], str) or not COMMIT.fullmatch(
        upgrade_fixture["tag_commit"]
    ):
        raise ReleaseError("upgrade_fixture.tag_commit must be a lowercase 40-character commit")
    if _require_https(
        upgrade_fixture["release_url"], "upgrade_fixture.release_url"
    ) != f"{TRAILBASE_REPOSITORY_URL}/releases/tag/v{fixture_version}":
        raise ReleaseError("upgrade_fixture.release_url does not name the exact version")
    if _require_https(
        upgrade_fixture["source_url"], "upgrade_fixture.source_url"
    ) != f"{TRAILBASE_REPOSITORY_URL}/tree/v{fixture_version}":
        raise ReleaseError("upgrade_fixture.source_url does not name the exact version")
    if (
        not isinstance(upgrade_fixture["expected_version_line"], str)
        or not upgrade_fixture["expected_version_line"].startswith(f"trail v{fixture_version}-")
        or f"-g{upgrade_fixture['tag_commit'][:8]} "
        not in upgrade_fixture["expected_version_line"]
    ):
        raise ReleaseError("upgrade_fixture.expected_version_line does not identify its release")
    _validate_native_release(upgrade_fixture, "upgrade_fixture")

    oci = release["oci"]
    if not isinstance(oci, dict):
        raise ReleaseError("oci must be an object")
    _require_keys(
        oci,
        {"repository", "index_digest", "attestation_manifests", "platform_manifests"},
        "oci",
    )
    if oci["repository"] != "docker.io/trailbase/trailbase":
        raise ReleaseError("OCI repository is not the approved TrailBase repository")
    if not isinstance(oci["index_digest"], str) or not OCI_DIGEST.fullmatch(oci["index_digest"]):
        raise ReleaseError("OCI index digest is invalid")
    manifests = oci["platform_manifests"]
    if not isinstance(manifests, dict) or set(manifests) != {"linux-amd64", "linux-arm64"}:
        raise ReleaseError("OCI manifests must contain exactly Linux amd64 and arm64")
    attestations = oci["attestation_manifests"]
    if not isinstance(attestations, dict) or set(attestations) != set(manifests):
        raise ReleaseError("OCI attestation manifests must match the runtime platforms")
    if any(not isinstance(value, str) or not OCI_DIGEST.fullmatch(value) for value in attestations.values()):
        raise ReleaseError("OCI attestation manifest digest is invalid")
    for platform_name, manifest in manifests.items():
        if not isinstance(manifest, dict):
            raise ReleaseError(f"OCI {platform_name} manifest must be an object")
        _require_keys(manifest, {"manifest_digest", "config_digest", "layers"}, platform_name)
        if (
            not isinstance(manifest["manifest_digest"], str)
            or not OCI_DIGEST.fullmatch(manifest["manifest_digest"])
            or not isinstance(manifest["config_digest"], str)
            or not OCI_DIGEST.fullmatch(manifest["config_digest"])
        ):
            raise ReleaseError(f"OCI {platform_name} manifest or config digest is invalid")
        layers = manifest["layers"]
        if not isinstance(layers, list) or not layers:
            raise ReleaseError(f"OCI {platform_name} layers must be non-empty")
        for layer in layers:
            if (
                not isinstance(layer, dict)
                or set(layer) != {"digest", "bytes"}
                or not isinstance(layer["digest"], str)
                or not OCI_DIGEST.fullmatch(layer["digest"])
                or not isinstance(layer["bytes"], int)
                or layer["bytes"] <= 0
            ):
                raise ReleaseError(f"OCI {platform_name} layer evidence is invalid")


def _validate_native_release(release: dict[str, Any], label: str) -> None:
    version = release["version"]
    native = release["native"]
    if not isinstance(native, dict) or set(native) != {"linux-aarch64", "linux-x86_64"}:
        raise ReleaseError(f"{label}.native must contain exactly Linux aarch64 and x86_64")
    for target, artifact in native.items():
        if not isinstance(artifact, dict):
            raise ReleaseError(f"{label}.native.{target} must be an object")
        _require_keys(
            artifact,
            {"url", "bytes", "sha256", "executable_sha256", "executable"},
            f"{label}.native.{target}",
        )
        url = _require_https(artifact["url"], f"{label}.native.{target}.url")
        architecture = {"linux-aarch64": "arm64", "linux-x86_64": "x86_64"}[target]
        expected_url = (
            f"{TRAILBASE_REPOSITORY_URL}/releases/download/v{version}/"
            f"trailbase_v{version}_{architecture}_linux.zip"
        )
        if url != expected_url:
            raise ReleaseError(f"{label}.native.{target}.url is not pinned to the exact release")
        if not isinstance(artifact["bytes"], int) or artifact["bytes"] <= 0:
            raise ReleaseError(f"{label}.native.{target}.bytes must be positive")
        if not isinstance(artifact["sha256"], str) or not SHA256.fullmatch(artifact["sha256"]):
            raise ReleaseError(f"{label}.native.{target}.sha256 is invalid")
        if not isinstance(artifact["executable_sha256"], str) or not SHA256.fullmatch(
            artifact["executable_sha256"]
        ):
            raise ReleaseError(f"{label}.native.{target}.executable_sha256 is invalid")
        if artifact["executable"] != "trail":
            raise ReleaseError(f"{label}.native.{target}.executable must be trail")


def host_target() -> str:
    if platform.system() != "Linux":
        raise ReleaseError("native TrailBase is unavailable on this host; use a proven Linux host")
    machine = platform.machine().lower()
    aliases = {"amd64": "x86_64", "arm64": "aarch64"}
    machine = aliases.get(machine, machine)
    target = f"linux-{machine}"
    if target not in {"linux-aarch64", "linux-x86_64"}:
        raise ReleaseError(f"native TrailBase is unavailable for {target}")
    return target


def host_oci_platform() -> str:
    target = host_target()
    return {"linux-x86_64": "linux-amd64", "linux-aarch64": "linux-arm64"}[target]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    descriptor = _open_safe_regular_file(path)
    try:
        with os.fdopen(os.dup(descriptor), "rb") as source:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(block)
    finally:
        os.close(descriptor)
    return digest.hexdigest()


def verify_archive(path: Path, release: dict[str, Any], target: str | None = None) -> None:
    target = target or host_target()
    artifact = release["native"].get(target)
    if artifact is None:
        raise ReleaseError(f"no native TrailBase artifact is pinned for {target}")
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode):
        raise ReleaseError(f"TrailBase archive is not a regular file: {path}")
    if metadata.st_size != artifact["bytes"]:
        raise ReleaseError(
            f"TrailBase archive size mismatch: expected {artifact['bytes']}, got {metadata.st_size}"
        )
    actual_digest = sha256_file(path)
    if actual_digest != artifact["sha256"]:
        raise ReleaseError(
            f"TrailBase archive digest mismatch: expected {artifact['sha256']}, got {actual_digest}"
        )
    with zipfile.ZipFile(path) as archive:
        if set(archive.namelist()) != {"trail", "CHANGELOG.md", "LICENSE"}:
            raise ReleaseError("TrailBase archive members differ from the approved release layout")
        for entry in archive.infolist():
            mode = entry.external_attr >> 16
            if stat.S_ISLNK(mode) or entry.file_size > 128 * 1024 * 1024:
                raise ReleaseError(f"unsafe TrailBase archive member: {entry.filename}")
        if hashlib.sha256(archive.read("LICENSE")).hexdigest() != release["license"]["file_sha256"]:
            raise ReleaseError("TrailBase archive licence differs from the reviewed text")
        with tempfile.TemporaryDirectory(prefix="fasti-trailbase-version-") as directory:
            executable = Path(directory) / "trail"
            executable.write_bytes(archive.read("trail"))
            executable.chmod(0o700)
            if sha256_file(executable) != artifact["executable_sha256"]:
                raise ReleaseError("TrailBase executable digest differs from the approved release")
            output = subprocess.run(  # nosec -- nosemgrep -- digest verified immediately above; fixed argv, no shell.
                [executable, "--version"],
                check=True,
                capture_output=True,
                text=True,
                timeout=15,
            ).stdout.splitlines()
    if not output or output[0] != release["expected_version_line"]:
        actual = output[0] if output else "<empty>"
        raise ReleaseError(
            f"TrailBase executable version mismatch: expected {release['expected_version_line']}, got {actual}"
        )


def _write_private(path: Path, data: bytes, mode: int) -> None:
    descriptor = os.open(
        path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
        mode,
    )
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as destination:
            destination.write(data)
            destination.flush()
            os.fsync(destination.fileno())
    finally:
        os.close(descriptor)


def _private_directory(path: Path) -> None:
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    metadata = path.lstat()
    if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
        raise ReleaseError(f"private runtime path is not a directory: {path}")
    if metadata.st_mode & 0o077:
        raise ReleaseError(f"private runtime path permits group or other access: {path}")


def _acquire_runtime_lock(root: Path) -> int:
    _private_directory(root)
    path = root / "runtime.lock"
    descriptor = os.open(
        path,
        os.O_RDWR | os.O_CREAT | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    try:
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
        ):
            raise ReleaseError("TrailBase runtime lock is not a singly linked owner file")
        fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        os.fchmod(descriptor, 0o600)
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise ReleaseError("TrailBase runtime lock is not an owner-only regular file")
        return descriptor
    except (OSError, ReleaseError):
        os.close(descriptor)
        raise


def prepare_runtime_lock(root: Path) -> None:
    try:
        descriptor = _acquire_runtime_lock(root)
    except BlockingIOError as error:
        raise ReleaseError("TrailBase development root is already active") from error
    os.close(descriptor)


def verify_runtime_lock(root: Path) -> None:
    descriptor = _open_safe_regular_file(root / "runtime.lock")
    try:
        metadata = os.fstat(descriptor)
        if (
            metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise ReleaseError("TrailBase runtime lock is not an owner-only regular file")
    finally:
        os.close(descriptor)


def _prepare_native_release(root: Path, release: dict[str, Any], offline: bool) -> Path:
    target = host_target()
    artifact = release["native"][target]
    _private_directory(root)
    cache = root / "cache"
    runtime_parent = root / "runtime"
    _private_directory(cache)
    _private_directory(runtime_parent)
    archive_path = cache / f"trailbase-v{release['version']}-{target}.zip"
    runtime = runtime_parent / f"v{release['version']}-{target}"
    executable = runtime / "trail"
    if runtime.exists():
        if runtime.is_symlink() or not runtime.is_dir():
            raise ReleaseError(f"TrailBase runtime is not a directory: {runtime}")
        verify_executable(executable, release, artifact["executable_sha256"])
        installed_license = runtime / "LICENSE"
        if hashlib.sha256(_read_regular_file(installed_license, 1024 * 1024)).hexdigest() != release[
            "license"
        ]["file_sha256"]:
            raise ReleaseError("installed TrailBase licence differs from the reviewed text")
        return executable
    if not archive_path.exists():
        if offline:
            raise ReleaseError(
                f"exact TrailBase archive is not cached: {archive_path}; "
                "run './scripts/dev.sh --prepare-offline' with network access"
            )
        temporary = cache / f".{archive_path.name}.{os.getpid()}.tmp"
        request = urllib.request.Request(artifact["url"], headers={"User-Agent": "Fasti/TrailBase-pin"})
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        try:
            with opener.open(request, timeout=60) as response:
                final_url = response.geturl()
                if not final_url.startswith("https://"):
                    raise ReleaseError("TrailBase download redirected away from HTTPS")
                payload = response.read(artifact["bytes"] + 1)
            if len(payload) != artifact["bytes"]:
                raise ReleaseError(
                    f"TrailBase download size mismatch: expected {artifact['bytes']}, got {len(payload)}"
                )
            _write_private(temporary, payload, 0o600)
            verify_archive(temporary, release, target)
            os.replace(temporary, archive_path)
        finally:
            temporary.unlink(missing_ok=True)
    verify_archive(archive_path, release, target)

    temporary_runtime = Path(tempfile.mkdtemp(prefix=f".{runtime.name}.", dir=runtime_parent))
    try:
        os.chmod(temporary_runtime, 0o700)  # nosec B103 -- nosemgrep -- owner-only is required.
        with zipfile.ZipFile(archive_path) as archive:
            _write_private(temporary_runtime / "trail", archive.read("trail"), 0o700)
            _write_private(temporary_runtime / "LICENSE", archive.read("LICENSE"), 0o600)
            _write_private(temporary_runtime / "CHANGELOG.md", archive.read("CHANGELOG.md"), 0o600)
        verify_executable(temporary_runtime / "trail", release, artifact["executable_sha256"])
        temporary_runtime.rename(runtime)
    finally:
        if temporary_runtime.exists():
            for child in temporary_runtime.iterdir():
                child.unlink()
            temporary_runtime.rmdir()
    return executable


def prepare_native(root: Path, offline: bool) -> Path:
    return _prepare_native_release(root, load_release(), offline)


def prepare_upgrade_fixture(root: Path, offline: bool) -> Path:
    release = load_release()
    fixture = {**release["upgrade_fixture"], "license": release["license"]}
    return _prepare_native_release(root, fixture, offline)


def verify_executable(executable: Path, release: dict[str, Any], expected_sha256: str) -> None:
    metadata = executable.lstat()
    if not stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
        raise ReleaseError(f"TrailBase executable is not a regular file: {executable}")
    if sha256_file(executable) != expected_sha256:
        raise ReleaseError("installed TrailBase executable digest does not match the release lock")
    output = subprocess.run(  # nosec -- nosemgrep -- regular file with exact release-lock digest; fixed argv.
        [executable, "--version"],  # nosemgrep -- exact release-lock executable and fixed argument.
        check=True,
        capture_output=True,
        text=True,
        timeout=15,
    ).stdout.splitlines()
    if not output or output[0] != release["expected_version_line"]:
        raise ReleaseError("installed TrailBase executable does not match the release lock")


def verify_private_root(root: Path, release_version: str | None = None) -> None:
    release_version = release_version or load_release()["version"]
    marker = root / "bootstrap.json"
    try:
        receipt = json.loads(_read_regular_file(marker, 16 * 1024))
    except (OSError, json.JSONDecodeError) as error:
        raise ReleaseError(f"cannot read TrailBase bootstrap receipt: {error}") from error
    expected = {
        "schema_version": "fasti.trailbase-bootstrap.v1",
        "release": release_version,
        "admin": "admin@localhost",
        "initial_password_rotated": True,  # nosec B105 -- boolean receipt field, not a password.
    }
    if not isinstance(receipt, dict) or any(receipt.get(key) != value for key, value in expected.items()):
        raise ReleaseError("TrailBase bootstrap receipt does not match the exact release")
    if set(receipt) != {*expected, "completed_at"} or not isinstance(receipt["completed_at"], str):
        raise ReleaseError("TrailBase bootstrap receipt fields differ")
    required = {
        root / "depot" / "config.textproto",
        root / "depot" / "data" / "main.db",
        root / "depot" / "data" / "session.db",
        root / "depot" / "secrets" / "secrets.textproto",
        root / "depot" / "secrets" / "keys" / "private_key.pem",
        root / "depot" / "secrets" / "keys" / "public_key.pem",
    }
    missing = sorted(str(path.relative_to(root)) for path in required if not path.is_file() or path.is_symlink())
    if missing:
        raise ReleaseError(f"TrailBase depot is incomplete: {missing}")
    for directory, names, filenames in os.walk(root, followlinks=False):
        directory_path = Path(directory)
        metadata = directory_path.lstat()
        if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
            raise ReleaseError(f"unsafe TrailBase directory: {directory_path}")
        if metadata.st_mode & 0o077:
            raise ReleaseError(f"TrailBase directory permits group or other access: {directory_path}")
        for name in [*names, *filenames]:
            path = directory_path / name
            item = path.lstat()
            if stat.S_ISLNK(item.st_mode):
                raise ReleaseError(f"TrailBase root contains a symlink: {path}")
            if stat.S_ISREG(item.st_mode) and item.st_mode & 0o077:
                raise ReleaseError(f"TrailBase file permits group or other access: {path}")
            if not stat.S_ISREG(item.st_mode) and not stat.S_ISDIR(item.st_mode):
                raise ReleaseError(f"TrailBase root contains an unsupported file type: {path}")


def _new_admin_password() -> str:
    alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
    return "Aa1!" + "".join(secrets.choice(alphabet) for _ in range(28))


def _deliver_admin_password_to_tty(terminal_fd: int, password: str) -> None:
    if not os.isatty(terminal_fd):
        raise ReleaseError("administrator credential delivery requires the owning terminal")
    payload = (
        "TrailBase administrator: admin@localhost\n"
        f"One-time delivered password: {password}\n"
        "Store it now. It is not retained by Fasti or written to service logs.\n"
        "Press Enter after storing the credential: "
    ).encode("utf-8")
    while payload:
        written = os.write(terminal_fd, payload)
        if written <= 0:
            raise ReleaseError("administrator credential delivery did not complete")
        payload = payload[written:]


def _pin_to_one_cpu() -> None:
    if not hasattr(os, "sched_getaffinity") or not hasattr(os, "sched_setaffinity"):
        raise ReleaseError("TrailBase runtime requires Linux CPU-affinity controls")
    allowed = os.sched_getaffinity(0)
    if not allowed:
        raise ReleaseError("TrailBase runtime has no allowed CPU")
    os.sched_setaffinity(0, {min(allowed)})


def _command_json(command: list[str]) -> Any:
    output = subprocess.run(  # nosec -- nosemgrep -- absolute allowlisted runtime; digest-pinned internal argv.
        command,  # nosemgrep -- absolute allowlisted OCI runtime and internal fixed-shape arguments.
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
    ).stdout
    return json.loads(output)


def _oci_runtime_executable(runtime: str) -> str:
    if runtime not in {"podman", "docker"}:
        raise ReleaseError("OCI runtime must be podman or docker")
    discovered = shutil.which(runtime)
    if discovered is None:
        raise ReleaseError(
            f"OCI runtime is unavailable: {runtime}; install it or set "
            "FASTI_CONTAINER_RUNTIME to an installed podman or docker runtime"
        )
    executable = Path(discovered).resolve()
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise ReleaseError(f"OCI runtime is not an executable regular file: {executable}")
    return str(executable)


def _verify_oci_index(runtime_executable: str, reference: str, release: dict[str, Any]) -> None:
    index = _command_json([runtime_executable, "manifest", "inspect", reference])
    if not isinstance(index, dict) or index.get("schemaVersion") != 2:
        raise ReleaseError("OCI index inspection is invalid")
    descriptors = index.get("manifests")
    if not isinstance(descriptors, list) or len(descriptors) != 4:
        raise ReleaseError("OCI index must contain two runtime and two attestation manifests")
    runtime_platforms: dict[str, str] = {}
    attestations: dict[str, str] = {}
    for descriptor in descriptors:
        if not isinstance(descriptor, dict) or not isinstance(descriptor.get("platform"), dict):
            raise ReleaseError("OCI index descriptor is invalid")
        platform_info = descriptor["platform"]
        platform_name = f"{platform_info.get('os')}-{platform_info.get('architecture')}"
        digest = descriptor.get("digest")
        if not isinstance(digest, str):
            raise ReleaseError("OCI index descriptor digest is invalid")
        annotations = descriptor.get("annotations", {})
        if platform_name in {"linux-amd64", "linux-arm64"} and not annotations:
            if platform_name in runtime_platforms:
                raise ReleaseError("OCI index contains a duplicate runtime platform")
            runtime_platforms[platform_name] = digest
        elif platform_name == "unknown-unknown" and isinstance(annotations, dict):
            subject = annotations.get("vnd.docker.reference.digest")
            if annotations.get("vnd.docker.reference.type") != "attestation-manifest":
                raise ReleaseError("OCI unknown descriptor is not a recognized attestation")
            matching = [
                name
                for name, manifest in release["oci"]["platform_manifests"].items()
                if manifest["manifest_digest"] == subject
            ]
            if len(matching) != 1 or matching[0] in attestations:
                raise ReleaseError("OCI attestation subject is ambiguous")
            attestations[matching[0]] = digest
        else:
            raise ReleaseError(f"OCI index contains an unsupported platform: {platform_name}")
    expected_runtime = {
        name: manifest["manifest_digest"]
        for name, manifest in release["oci"]["platform_manifests"].items()
    }
    if runtime_platforms != expected_runtime or attestations != release["oci"]["attestation_manifests"]:
        raise ReleaseError("OCI index descriptors differ from the release lock")


def _fetch_oci_manifest(release: dict[str, Any], platform_name: str) -> None:
    token_url = "https://auth.docker.io/token?" + urllib.parse.urlencode(
        {"service": "registry.docker.io", "scope": "repository:trailbase/trailbase:pull"}
    )
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(token_url, timeout=30) as response:
        token_document = json.load(response)
    token = token_document.get("token")
    if not isinstance(token, str) or not token:
        raise ReleaseError("OCI registry did not return a pull token")
    expected = release["oci"]["platform_manifests"][platform_name]
    request = urllib.request.Request(
        "https://registry-1.docker.io/v2/trailbase/trailbase/manifests/"
        + expected["manifest_digest"],
        headers={
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.oci.image.manifest.v1+json",
        },
    )
    with opener.open(request, timeout=30) as response:
        payload = response.read(64 * 1024)
    if "sha256:" + hashlib.sha256(payload).hexdigest() != expected["manifest_digest"]:
        raise ReleaseError("OCI platform manifest payload digest differs")
    manifest = json.loads(payload)
    actual_layers = [
        {"digest": layer.get("digest"), "bytes": layer.get("size")}
        for layer in manifest.get("layers", [])
        if isinstance(layer, dict)
    ]
    if (
        manifest.get("schemaVersion") != 2
        or manifest.get("config", {}).get("digest") != expected["config_digest"]
        or actual_layers != expected["layers"]
    ):
        raise ReleaseError("OCI config or layer graph differs from the release lock")


def _verify_local_oci(
    runtime_executable: str,
    reference: str,
    release: dict[str, Any],
    platform_name: str,
) -> None:
    document = _command_json([runtime_executable, "image", "inspect", reference])
    if not isinstance(document, list) or len(document) != 1 or not isinstance(document[0], dict):
        raise ReleaseError("local OCI image inspection is invalid")
    image = document[0]
    expected = release["oci"]["platform_manifests"][platform_name]
    repo_digests = image.get("RepoDigests")
    required_digests = {
        f"{release['oci']['repository']}@{release['oci']['index_digest']}",
        f"{release['oci']['repository']}@{expected['manifest_digest']}",
    }
    config_id = image.get("Id", "")
    if isinstance(config_id, str):
        config_id = config_id.removeprefix("sha256:")
    expected_arch = platform_name.removeprefix("linux-")
    if (
        not isinstance(repo_digests, list)
        or not required_digests.issubset(repo_digests)
        or image.get("Os") != "linux"
        or image.get("Architecture") != expected_arch
        or config_id != expected["config_digest"].removeprefix("sha256:")
    ):
        raise ReleaseError("local OCI image identity differs from the release lock")


def prepare_oci(root: Path, runtime: str, offline: bool) -> str:
    runtime_executable = _oci_runtime_executable(runtime)
    release = load_release()
    platform_name = host_oci_platform()
    reference = f"{release['oci']['repository']}@{release['oci']['index_digest']}"
    _private_directory(root)
    receipt_path = root / f"oci-{runtime}.json"
    if not offline:
        subprocess.run(  # nosec -- nosemgrep -- absolute allowlisted runtime; digest-pinned reference.
            [runtime_executable, "pull", "--platform", platform_name.replace("-", "/", 1), reference],  # nosemgrep -- absolute allowlisted OCI runtime and digest-pinned reference.
            check=True,
            timeout=600,
        )
        _verify_oci_index(runtime_executable, reference, release)
        _fetch_oci_manifest(release, platform_name)
        receipt = {
            "schema_version": "fasti.trailbase-oci-cache.v1",
            "runtime": runtime,
            "platform": platform_name,
            "index_digest": release["oci"]["index_digest"],
            "manifest_digest": release["oci"]["platform_manifests"][platform_name][
                "manifest_digest"
            ],
            "config_digest": release["oci"]["platform_manifests"][platform_name]["config_digest"],
            "prepared_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        }
        receipt_path.unlink(missing_ok=True)
        _write_private(receipt_path, (json.dumps(receipt, indent=2) + "\n").encode(), 0o600)
    try:
        receipt = json.loads(_read_regular_file(receipt_path, 16 * 1024))
    except (OSError, json.JSONDecodeError) as error:
        raise ReleaseError(f"cannot read OCI preparation receipt: {error}") from error
    expected_receipt = {
        "schema_version": "fasti.trailbase-oci-cache.v1",
        "runtime": runtime,
        "platform": platform_name,
        "index_digest": release["oci"]["index_digest"],
        "manifest_digest": release["oci"]["platform_manifests"][platform_name][
            "manifest_digest"
        ],
        "config_digest": release["oci"]["platform_manifests"][platform_name]["config_digest"],
    }
    if (
        not isinstance(receipt, dict)
        or set(receipt) != {*expected_receipt, "prepared_at"}
        or any(receipt.get(key) != value for key, value in expected_receipt.items())
        or not isinstance(receipt.get("prepared_at"), str)
    ):
        raise ReleaseError("OCI preparation receipt differs from the release lock")
    _verify_local_oci(runtime_executable, reference, release, platform_name)
    return reference


def verify_oci_container(root: Path, runtime: str, name: str) -> None:
    reference = prepare_oci(root, runtime, offline=True)
    runtime_executable = _oci_runtime_executable(runtime)
    release = load_release()
    document = _command_json([runtime_executable, "inspect", name])
    if not isinstance(document, list) or len(document) != 1 or not isinstance(document[0], dict):
        raise ReleaseError("OCI container inspection is invalid")
    container = document[0]
    config = container.get("Config")
    state = container.get("State")
    host = container.get("HostConfig")
    mounts = container.get("Mounts")
    expected_config = release["oci"]["platform_manifests"][host_oci_platform()][
        "config_digest"
    ].removeprefix("sha256:")
    image_id = container.get("Image", "")
    if isinstance(image_id, str):
        image_id = image_id.removeprefix("sha256:")
    if (
        not isinstance(config, dict)
        or not isinstance(state, dict)
        or not isinstance(host, dict)
        or state.get("Running") is not True
        or config.get("Image") != reference
        or image_id != expected_config
    ):
        raise ReleaseError("running OCI container identity differs from the release lock")
    expected_host = {
        "Memory": 192 * 1024 * 1024,
        "MemorySwap": 192 * 1024 * 1024,
        "NanoCpus": 1_000_000_000,
        "PidsLimit": 128,
        "ReadonlyRootfs": True,
    }
    if (
        any(host.get(key) != value for key, value in expected_host.items())
        or "no-new-privileges" not in host.get("SecurityOpt", [])
        or host.get("CapAdd") not in (None, [])
        or host.get("LogConfig", {}).get("Type") != "none"
        or host.get("PortBindings")
        != {"4000/tcp": [{"HostIp": "127.0.0.1", "HostPort": "4000"}]}
    ):
        raise ReleaseError("running OCI container isolation policy differs from the launcher")
    user = str(config.get("User", "")).split(":", 1)[0]
    command = config.get("Cmd", [])
    entrypoint = config.get("Entrypoint", [])
    required_pairs = [
        ["--depot", "/app/trailroot/depot"],
        ["--public-url", "http://127.0.0.1:4000"],
        ["--address", "0.0.0.0:4000"],
        ["--admin-address", "127.0.0.1:4001"],
        ["--cors-allowed-origins", "http://127.0.0.1:4000"],
        ["--runtime-threads", "1"],
    ]
    if (
        user in {"", "0", "root"}
        or entrypoint not in (["/usr/bin/flock"], "/usr/bin/flock")
        or not isinstance(command, list)
        or not all(
            any(command[index : index + 2] == pair for index in range(len(command) - 1))
            for pair in required_pairs
        )
        or not isinstance(mounts, list)
        or not any(
            mount.get("Source") == str(root.resolve())
            and mount.get("Destination") == "/app/trailroot"
            and mount.get("RW") is True
            for mount in mounts
            if isinstance(mount, dict)
        )
    ):
        raise ReleaseError("running OCI container command or data boundary differs from the launcher")


def _post_json(
    url: str,
    payload: dict[str, str],
    token: str | None = None,
    expect_json: bool = True,
) -> dict[str, Any]:
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers=headers,
        method="POST",
    )
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(request, timeout=5) as response:
        body = response.read(1024 * 1024)
    if not expect_json or not body:
        return {}
    value = json.loads(body)
    if not isinstance(value, dict):
        raise ReleaseError("TrailBase returned a non-object authentication response")
    return value


def _loopback_address(value: str, label: str) -> None:
    try:
        host, port = value.rsplit(":", 1)
        address = ipaddress.ip_address(host.strip("[]"))
        port_number = int(port)
    except ValueError as error:
        raise ReleaseError(f"{label} must be a numeric loopback socket address") from error
    if not address.is_loopback or not 1 <= port_number <= 65535:
        raise ReleaseError(f"{label} must be a numeric loopback socket address")


def _validate_listener_configuration(
    public_url: str,
    address: str,
    admin_address: str,
    cors_origin: str,
) -> None:
    _loopback_address(address, "address")
    _loopback_address(admin_address, "admin_address")
    if address == admin_address:
        raise ReleaseError("TrailBase public and private admin addresses must differ")
    parsed_public_url = urllib.parse.urlsplit(public_url)
    if (
        parsed_public_url.scheme != "http"
        or parsed_public_url.netloc != address
        or parsed_public_url.path not in {"", "/"}
        or parsed_public_url.query
        or parsed_public_url.fragment
        or parsed_public_url.username is not None
    ):
        raise ReleaseError("public_url must name the exact loopback account listener")
    if cors_origin.rstrip("/") != public_url.rstrip("/"):
        raise ReleaseError("CORS origin must equal the loopback public URL")


def bootstrap_native(
    root: Path,
    public_url: str,
    address: str,
    admin_address: str,
    cors_origin: str,
) -> None:
    _validate_listener_configuration(public_url, address, admin_address, cors_origin)
    if not sys.stdin.isatty() or not sys.stdout.isatty():
        raise ReleaseError("first initialization requires the owning operator's terminal")
    terminal = sys.stdout
    runtime_lock: int | None = None
    try:
        _pin_to_one_cpu()
        executable = prepare_native(root, offline=True)
        try:
            runtime_lock = _acquire_runtime_lock(root)
        except BlockingIOError as error:
            raise ReleaseError("TrailBase development root is already active") from error
        depot = root / "depot"
        marker = root / "bootstrap.json"
        if marker.exists():
            raise ReleaseError("TrailBase is already initialized for this development root")
        if depot.exists() and any(depot.iterdir()):
            raise ReleaseError(
                "TrailBase depot is non-empty without a bootstrap receipt; restore or remove it explicitly"
            )
        _private_directory(depot)

        initial_password: list[str] = []
        password_ready = threading.Event()
        child_environment = {
            key: value for key, value in os.environ.items() if not key.startswith("TRAIL_")
        }
        child_environment["RUST_LOG"] = "info"
        old_umask = os.umask(0o077)
        try:
            process = start_managed_process_group(
                [
                    str(executable),
                    "--depot",
                    str(depot),
                    "--public-url",
                    public_url,
                    "run",
                    "--address",
                    address,
                    "--admin-address",
                    admin_address,
                    "--cors-allowed-origins",
                    cors_origin,
                    "--runtime-threads",
                    "1",
                    "--stderr-logging",
                ],
                environment=child_environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
            )
        finally:
            os.umask(old_umask)

        output_stream = process.stdout
        if output_stream is None:
            stop_managed_process_group(process)
            raise ReleaseError("TrailBase bootstrap process has no output pipe")

        def read_bootstrap_output() -> None:
            for line in output_stream:
                match = BOOTSTRAP_PASSWORD.fullmatch(line.rstrip("\r\n"))
                if match and not initial_password:
                    initial_password.append(match.group(1))
                    password_ready.set()

        reader = threading.Thread(target=read_bootstrap_output, daemon=True)
        reader.start()
        new_password = _new_admin_password()
        base_url = f"http://{address}"
        try:
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise ReleaseError("TrailBase exited during private initialization")
                if password_ready.wait(0.1):
                    try:
                        with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(
                            f"{base_url}/api/healthcheck", timeout=1
                        ):
                            break
                    except (OSError, urllib.error.URLError):
                        continue
            else:
                raise ReleaseError("TrailBase initialization did not become healthy")
            if not initial_password:
                raise ReleaseError("TrailBase did not provide its one-time administrator credential")

            login = _post_json(
                f"{base_url}/api/auth/v1/login",
                {"email": "admin@localhost", "password": initial_password[0]},
            )
            auth_token = login.get("auth_token")
            if not isinstance(auth_token, str) or not auth_token:
                raise ReleaseError("TrailBase administrator bootstrap login failed")
            _post_json(
                f"{base_url}/api/auth/v1/change_password",
                {
                    "old_password": initial_password[0],
                    "new_password": new_password,
                    "new_password_repeat": new_password,
                },
                auth_token,
                expect_json=False,
            )
            _post_json(
                f"{base_url}/api/auth/v1/login",
                {"email": "admin@localhost", "password": new_password},
            )
        except (OSError, urllib.error.URLError, json.JSONDecodeError) as error:
            raise ReleaseError("TrailBase private administrator rotation failed") from error
        finally:
            initial_password.clear()
            stop_managed_process_group(process)
            reader.join(timeout=2)

        _deliver_admin_password_to_tty(terminal.fileno(), new_password)
        if sys.stdin.readline() == "":
            raise ReleaseError(
                "credential custody was not confirmed; keep the unverified depot stopped and "
                "restore or remove it explicitly before retrying initialization"
            )
        receipt = {
            "schema_version": "fasti.trailbase-bootstrap.v1",
            "release": load_release()["version"],
            "admin": "admin@localhost",
            "initial_password_rotated": True,  # nosec B105 -- nosemgrep -- boolean receipt field.
            "completed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        }
        _write_private(marker, (json.dumps(receipt, indent=2) + "\n").encode("utf-8"), 0o600)
    finally:
        if runtime_lock is not None:
            os.close(runtime_lock)
        terminal.flush()


def run_native(
    root: Path,
    public_url: str,
    address: str,
    admin_address: str,
    cors_origin: str,
) -> None:
    _validate_listener_configuration(public_url, address, admin_address, cors_origin)
    verify_private_root(root)
    executable = prepare_native(root, offline=True)
    try:
        descriptor = _acquire_runtime_lock(root)
    except BlockingIOError as error:
        raise ReleaseError("TrailBase development root is already active") from error
    os.set_inheritable(descriptor, True)
    _pin_to_one_cpu()
    environment = {key: value for key, value in os.environ.items() if not key.startswith("TRAIL_")}
    environment["RUST_LOG"] = "warn"
    os.execve(  # nosec B606 -- nosemgrep -- executable is release-lock verified; listener and origin arguments are validated loopback values and no shell is used.
        executable,
        [
            str(executable),
            "--depot",
            str(root / "depot"),
            "--public-url",
            public_url,
            "run",
            "--address",
            address,
            "--admin-address",
            admin_address,
            "--cors-allowed-origins",
            cors_origin,
            "--runtime-threads",
            "1",
            "--stderr-logging",
        ],
        environment,
    )


def _archive_path(value: str) -> Path:
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        raise ReleaseError(f"unsafe depot archive path: {value}")
    return path


def backup_depot(
    root: Path,
    output_dir: Path,
    release_version: str | None = None,
) -> tuple[Path, str]:
    release_version = release_version or load_release()["version"]
    verify_private_root(root, release_version)
    try:
        runtime_lock = _acquire_runtime_lock(root)
    except BlockingIOError as error:
        raise ReleaseError("stop TrailBase before creating a full-depot backup") from error
    try:
        _private_directory(output_dir)
        timestamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
        destination = output_dir / f"trailbase-v{release_version}-{timestamp}.zip"
        temporary = output_dir / f".{destination.name}.{os.getpid()}.tmp"
        inventory: list[dict[str, Any]] = []
        sources = [root / "bootstrap.json", root / "depot"]
        descriptor = os.open(
            temporary,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
            0o600,
        )
        try:
            with os.fdopen(descriptor, "w+b", closefd=False) as archive_file:
                with zipfile.ZipFile(
                    archive_file, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6
                ) as archive:
                    for source in sources:
                        paths = [source] if source.is_file() else [source, *sorted(source.rglob("*"))]
                        for path in paths:
                            relative = path.relative_to(root)
                            metadata = path.lstat()
                            if stat.S_ISLNK(metadata.st_mode):
                                raise ReleaseError(f"depot backup refuses symlink: {relative}")
                            name = relative.as_posix()
                            if stat.S_ISDIR(metadata.st_mode):
                                archive.writestr(f"{name}/", b"")
                                inventory.append(
                                    {"path": name, "kind": "directory", "mode": metadata.st_mode & 0o777}
                                )
                                continue
                            if not stat.S_ISREG(metadata.st_mode):
                                raise ReleaseError(f"depot backup refuses unsupported file: {relative}")
                            source_fd = os.open(
                                path,
                                os.O_RDONLY
                                | getattr(os, "O_CLOEXEC", 0)
                                | getattr(os, "O_NOFOLLOW", 0),
                            )
                            digest = hashlib.sha256()
                            size = 0
                            try:
                                before = os.fstat(source_fd)
                                with os.fdopen(os.dup(source_fd), "rb") as input_file:
                                    with archive.open(name, "w") as output_file:
                                        for block in iter(lambda: input_file.read(1024 * 1024), b""):
                                            size += len(block)
                                            digest.update(block)
                                            output_file.write(block)
                                after = os.fstat(source_fd)
                            finally:
                                os.close(source_fd)
                            if (before.st_size, before.st_mtime_ns) != (
                                after.st_size,
                                after.st_mtime_ns,
                            ) or size != before.st_size:
                                raise ReleaseError(f"depot file changed during backup: {relative}")
                            inventory.append(
                                {
                                    "path": name,
                                    "kind": "file",
                                    "mode": metadata.st_mode & 0o777,
                                    "bytes": size,
                                    "sha256": digest.hexdigest(),
                                }
                            )
                    manifest = {
                        "schema_version": "fasti.trailbase-depot-backup.v1",
                        "release": release_version,
                        "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                        "entries": inventory,
                    }
                    archive.writestr(
                        "manifest.json", json.dumps(manifest, indent=2, sort_keys=True) + "\n"
                    )
                archive_file.flush()
                os.fsync(archive_file.fileno())
        finally:
            os.close(descriptor)
        os.replace(temporary, destination)
        return destination, sha256_file(destination)
    finally:
        os.close(runtime_lock)
        temporary_path = locals().get("temporary")
        if isinstance(temporary_path, Path):
            temporary_path.unlink(missing_ok=True)


def _validate_restore_parent(parent: Path) -> os.stat_result:
    immediate: os.stat_result | None = None
    for index, ancestor in enumerate((parent, *parent.parents)):
        try:
            metadata = ancestor.lstat()
        except OSError as error:
            raise ReleaseError(f"cannot inspect restore parent boundary: {ancestor}") from error
        if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
            raise ReleaseError(f"restore parent boundary is not a directory: {ancestor}")
        if index == 0:
            if metadata.st_uid != os.geteuid() or metadata.st_mode & 0o077:
                raise ReleaseError(
                    "isolated restore parent must be owned by this user and owner-only"
                )
            immediate = metadata
        else:
            if metadata.st_uid not in {0, os.geteuid()}:
                raise ReleaseError(f"restore parent ancestor has an untrusted owner: {ancestor}")
            if metadata.st_mode & 0o022 and not metadata.st_mode & stat.S_ISVTX:
                raise ReleaseError(f"restore parent ancestor is writable by another user: {ancestor}")
    if immediate is None:
        raise ReleaseError("isolated restore parent boundary is empty")
    return immediate


def restore_depot(
    archive_path: Path,
    target: Path,
    release_version: str | None = None,
) -> None:
    release_version = release_version or load_release()["version"]
    target = Path(os.path.abspath(target))
    if target.exists() or target.is_symlink():
        raise ReleaseError("isolated restore target already exists")
    parent = target.parent
    parent_metadata = _validate_restore_parent(parent)
    metadata = archive_path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
        raise ReleaseError("depot backup is not a regular file")
    temporary = Path(tempfile.mkdtemp(prefix=f".{target.name}.restore.", dir=parent))
    os.chmod(temporary, 0o700)  # nosec B103 -- nosemgrep -- owner-only is required.
    try:
        with zipfile.ZipFile(archive_path) as archive:
            names = archive.namelist()
            if len(names) != len(set(names)) or "manifest.json" not in names or len(names) > 100_000:
                raise ReleaseError("depot backup member inventory is invalid")
            member_info = {entry.filename: entry for entry in archive.infolist()}
            manifest_info = member_info["manifest.json"]
            if manifest_info.is_dir() or manifest_info.file_size > 16 * 1024:
                raise ReleaseError("depot backup manifest exceeds its size limit")
            manifest = json.loads(archive.read("manifest.json"))
            if not isinstance(manifest, dict) or set(manifest) != {
                "schema_version",
                "release",
                "created_at",
                "entries",
            }:
                raise ReleaseError("depot backup manifest fields differ")
            if (
                manifest["schema_version"] != "fasti.trailbase-depot-backup.v1"
                or manifest["release"] != release_version
                or not isinstance(manifest["created_at"], str)
                or not isinstance(manifest["entries"], list)
            ):
                raise ReleaseError("depot backup manifest does not match this release")
            entries: dict[str, dict[str, Any]] = {}
            total_bytes = 0
            for entry in manifest["entries"]:
                if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
                    raise ReleaseError("depot backup entry is invalid")
                relative = _archive_path(entry["path"])
                if not (
                    relative.parts == ("bootstrap.json",)
                    or relative.parts[0] == "depot"
                ):
                    raise ReleaseError(
                        "depot backup entry is outside the bootstrap and depot boundary"
                    )
                name = relative.as_posix()
                if name in entries or entry.get("kind") not in {"directory", "file"}:
                    raise ReleaseError("depot backup contains duplicate or invalid entries")
                mode = entry.get("mode")
                if not isinstance(mode, int) or mode & 0o077 or mode < 0:
                    raise ReleaseError("depot backup contains an unsafe mode")
                if entry["kind"] == "directory":
                    if set(entry) != {"path", "kind", "mode"}:
                        raise ReleaseError("depot backup directory fields differ")
                else:
                    if set(entry) != {"path", "kind", "mode", "bytes", "sha256"}:
                        raise ReleaseError("depot backup file fields differ")
                    if (
                        not isinstance(entry["bytes"], int)
                        or not 0 <= entry["bytes"] <= 1024 * 1024 * 1024
                        or not isinstance(entry["sha256"], str)
                        or not SHA256.fullmatch(entry["sha256"])
                    ):
                        raise ReleaseError("depot backup file evidence is invalid")
                    total_bytes += entry["bytes"]
                entries[name] = entry
            if total_bytes > 4 * 1024 * 1024 * 1024:
                raise ReleaseError("depot backup exceeds the restore size limit")
            expected_members = {"manifest.json"}
            expected_members.update(
                f"{name}/" if entry["kind"] == "directory" else name
                for name, entry in entries.items()
            )
            if set(names) != expected_members:
                raise ReleaseError("depot backup members differ from its manifest")
            for name, entry in entries.items():
                archive_name = f"{name}/" if entry["kind"] == "directory" else name
                info = member_info[archive_name]
                if bool(info.is_dir()) != (entry["kind"] == "directory") or (
                    entry["kind"] == "file" and info.file_size != entry["bytes"]
                ):
                    raise ReleaseError(f"depot backup member size or type mismatch: {name}")
            extracted_bytes = 0
            for name, entry in sorted(entries.items(), key=lambda item: len(Path(item[0]).parts)):
                destination = temporary / _archive_path(name)
                if entry["kind"] == "directory":
                    destination.mkdir(mode=entry["mode"], parents=True, exist_ok=True)
                    os.chmod(destination, entry["mode"])
                    continue
                destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                digest = hashlib.sha256()
                size = 0
                descriptor = os.open(
                    destination,
                    os.O_WRONLY
                    | os.O_CREAT
                    | os.O_EXCL
                    | getattr(os, "O_CLOEXEC", 0)
                    | getattr(os, "O_NOFOLLOW", 0),
                    entry["mode"],
                )
                try:
                    with archive.open(name) as input_file, os.fdopen(
                        descriptor, "wb", closefd=False
                    ) as output_file:
                        for block in iter(lambda: input_file.read(1024 * 1024), b""):
                            size += len(block)
                            extracted_bytes += len(block)
                            if (
                                size > entry["bytes"]
                                or extracted_bytes > 4 * 1024 * 1024 * 1024
                            ):
                                raise ReleaseError(f"depot backup content exceeds its bound: {name}")
                            digest.update(block)
                            output_file.write(block)
                        output_file.flush()
                        os.fsync(output_file.fileno())
                finally:
                    os.close(descriptor)
                if size != entry["bytes"] or digest.hexdigest() != entry["sha256"]:
                    raise ReleaseError(f"depot backup content mismatch: {name}")
        prepare_runtime_lock(temporary)
        verify_runtime_lock(temporary)
        verify_private_root(temporary, release_version)
        current_parent = _validate_restore_parent(parent)
        if (current_parent.st_dev, current_parent.st_ino) != (
            parent_metadata.st_dev,
            parent_metadata.st_ino,
        ):
            raise ReleaseError("isolated restore parent changed before publication")
        temporary.rename(target)
    finally:
        if temporary.exists():
            for path in sorted(temporary.rglob("*"), reverse=True):
                if path.is_dir():
                    path.rmdir()
                else:
                    path.unlink()
            temporary.rmdir()


def _backup_restore_self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="fasti-trailbase-depot-test-") as directory:
        base = Path(directory)
        os.chmod(base, 0o700)  # nosec B103 -- nosemgrep -- owner-only is required.
        root = base / "source"
        _private_directory(root)
        receipt = {
            "schema_version": "fasti.trailbase-bootstrap.v1",
            "release": load_release()["version"],
            "admin": "admin@localhost",
            "initial_password_rotated": True,  # nosec B105 -- nosemgrep -- boolean receipt field.
            "completed_at": "2026-08-30T00:00:00+00:00",
        }
        _write_private(root / "bootstrap.json", json.dumps(receipt).encode(), 0o600)
        for relative in [
            "depot/config.textproto",
            "depot/data/main.db",
            "depot/data/session.db",
            "depot/secrets/secrets.textproto",
            "depot/secrets/keys/private_key.pem",
            "depot/secrets/keys/public_key.pem",
            "depot/uploads/example.bin",
        ]:
            path = root / relative
            _private_directory(path.parent)
            _write_private(path, relative.encode(), 0o600)
        backup, _digest = backup_depot(root, base / "backups")
        restored = base / "restored"
        restore_depot(backup, restored)
        verify_runtime_lock(restored)
        verify_private_root(restored)
        if (restored / "depot/uploads/example.bin").read_bytes() != b"depot/uploads/example.bin":
            raise ReleaseError("full-depot backup self-test lost nested content")
        try:
            restore_depot(backup, base / "wrong-release", "0.0.0")
        except ReleaseError:
            pass
        else:
            raise ReleaseError("release-mismatched restore self-test did not fail")

        mismatched = base / "mismatched.zip"
        with zipfile.ZipFile(backup) as source, zipfile.ZipFile(
            mismatched, "w", compression=zipfile.ZIP_DEFLATED
        ) as destination:
            manifest = json.loads(source.read("manifest.json"))
            file_entry = next(entry for entry in manifest["entries"] if entry["kind"] == "file")
            file_entry["bytes"] += 1
            for info in source.infolist():
                payload = (
                    json.dumps(manifest, indent=2, sort_keys=True).encode()
                    if info.filename == "manifest.json"
                    else source.read(info.filename)
                )
                destination.writestr(info, payload)
        try:
            restore_depot(mismatched, base / "mismatched-restore")
        except ReleaseError:
            pass
        else:
            raise ReleaseError("member-size-mismatched restore self-test did not fail")

        unexpected = base / "unexpected-top-level.zip"
        unexpected_payload = b"archive-controlled-lock"
        with zipfile.ZipFile(backup) as source, zipfile.ZipFile(
            unexpected, "w", compression=zipfile.ZIP_DEFLATED
        ) as destination:
            manifest = json.loads(source.read("manifest.json"))
            manifest["entries"].append(
                {
                    "path": "runtime.lock",
                    "kind": "file",
                    "mode": 0o600,
                    "bytes": len(unexpected_payload),
                    "sha256": hashlib.sha256(unexpected_payload).hexdigest(),
                }
            )
            for info in source.infolist():
                payload = (
                    json.dumps(manifest, indent=2, sort_keys=True).encode()
                    if info.filename == "manifest.json"
                    else source.read(info.filename)
                )
                destination.writestr(info, payload)
            destination.writestr("runtime.lock", unexpected_payload)
        try:
            restore_depot(unexpected, base / "unexpected-top-level-restore")
        except ReleaseError:
            pass
        else:
            raise ReleaseError("unexpected top-level restore entry self-test did not fail")

        unsafe_parent = base / "unsafe-parent"
        unsafe_parent.mkdir(mode=0o777)
        os.chmod(unsafe_parent, 0o777)  # nosec B103 -- nosemgrep -- deliberate rejection fixture.
        try:
            restore_depot(backup, unsafe_parent / "restored")
        except ReleaseError:
            pass
        else:
            raise ReleaseError("non-private restore parent self-test did not fail")

        unsafe_ancestor = base / "unsafe-ancestor"
        unsafe_ancestor.mkdir(mode=0o777)
        os.chmod(unsafe_ancestor, 0o777)  # nosec B103 -- nosemgrep -- deliberate rejection fixture.
        private_child = unsafe_ancestor / "private"
        private_child.mkdir(mode=0o700)
        try:
            restore_depot(backup, private_child / "restored")
        except ReleaseError:
            pass
        else:
            raise ReleaseError("writable restore ancestor self-test did not fail")


def _managed_process_group_self_test() -> None:
    worker_source = f'''
import os
import signal
import subprocess
import sys

sys.path.insert(0, {str(ROOT / "scripts")!r})
import trailbase_runtime as runtime

runtime.install_termination_cleanup()
real_popen = runtime.subprocess.Popen

def interrupted_popen(*args, **kwargs):
    process = real_popen(*args, **kwargs)
    print(process.pid, flush=True)
    os.kill(os.getpid(), signal.SIGTERM)
    return process

runtime.subprocess.Popen = interrupted_popen
child_source = """
import signal
import subprocess
import sys
import time

subprocess.Popen([
    sys.executable,
    "-c",
    "import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(30)",
])
time.sleep(30)
"""
runtime.start_managed_process_group(
    [sys.executable, "-c", child_source],
    environment=dict(os.environ),
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
)
'''
    worker = start_managed_process_group(
        [sys.executable, "-B", "-c", worker_source],
        environment=dict(os.environ),
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    try:
        output_stream = worker.stdout
        if output_stream is None:
            raise ReleaseError("termination cleanup self-test has no output pipe")
        child_process_group = int(output_stream.readline().strip())
        if worker.wait(timeout=10) != 128 + signal.SIGTERM:
            raise ReleaseError("termination cleanup returned the wrong exit status")
        if _process_group_exists(child_process_group):
            raise ReleaseError("termination cleanup left a child process group active")
    finally:
        stop_managed_process_group(worker)


def _runtime_lock_self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="fasti-trailbase-lock-") as workspace:
        root = Path(workspace) / "root"
        previous_umask = os.umask(0o002)
        try:
            prepare_runtime_lock(root)
        finally:
            os.umask(previous_umask)
        lock = root / "runtime.lock"
        verify_runtime_lock(root)

        os.chmod(lock, 0o644)
        held = os.open(lock, os.O_RDWR | getattr(os, "O_NOFOLLOW", 0))
        try:
            fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
            try:
                verify_runtime_lock(root)
            except ReleaseError:
                pass
            else:
                raise ReleaseError("runtime lock self-test verified an unsafe active lock")
            try:
                prepare_runtime_lock(root)
            except ReleaseError:
                pass
            else:
                raise ReleaseError("runtime lock self-test ignored an active lock")
            if stat.S_IMODE(os.fstat(held).st_mode) != 0o644:
                raise ReleaseError("runtime lock self-test modified an active lock")
        finally:
            os.close(held)
        prepare_runtime_lock(root)
        if stat.S_IMODE(lock.stat().st_mode) != 0o600:
            raise ReleaseError("runtime lock self-test did not repair a stopped lock")

        alias = root / "runtime-lock-alias"
        os.link(lock, alias)
        try:
            try:
                verify_runtime_lock(root)
            except ReleaseError:
                pass
            else:
                raise ReleaseError("runtime lock self-test verified a multiply linked lock")
            try:
                prepare_runtime_lock(root)
            except ReleaseError:
                pass
            else:
                raise ReleaseError("runtime lock self-test accepted a multiply linked lock")
        finally:
            alias.unlink()

        lock.unlink()
        target = root / "symlink-target"
        _write_private(target, b"", 0o600)
        lock.symlink_to(target.name)
        try:
            prepare_runtime_lock(root)
        except OSError:
            pass
        else:
            raise ReleaseError("runtime lock self-test accepted a symlink")
        lock.unlink()
        lock.mkdir(mode=0o700)
        try:
            prepare_runtime_lock(root)
        except OSError:
            pass
        else:
            raise ReleaseError("runtime lock self-test accepted a directory")


def self_test() -> None:
    release = load_release()
    mutations = []

    floating_url = copy.deepcopy(release)
    floating_url["native"]["linux-x86_64"]["url"] = (
        "https://github.com/trailbaseio/trailbase/releases/latest/download/trail.zip"
    )
    mutations.append(floating_url)

    spoofed_release_host = copy.deepcopy(release)
    spoofed_release_host["release_url"] = (
        "https://example.invalid/trailbaseio/trailbase/releases/tag/v0.33.5"
    )
    mutations.append(spoofed_release_host)

    wrong_tag_commit = copy.deepcopy(release)
    wrong_tag_commit["tag_commit"] = "0" * 40
    mutations.append(wrong_tag_commit)

    floating_image = copy.deepcopy(release)
    floating_image["oci"]["index_digest"] = "latest"
    mutations.append(floating_image)

    wrong_process_boundary = copy.deepcopy(release)
    wrong_process_boundary["license"]["integration"] = "embedded"
    mutations.append(wrong_process_boundary)

    wrong_license = copy.deepcopy(release)
    wrong_license["license"]["file_sha256"] = "0" * 64
    mutations.append(wrong_license)

    missing_platform = copy.deepcopy(release)
    del missing_platform["native"]["linux-aarch64"]
    mutations.append(missing_platform)

    floating_upgrade_fixture = copy.deepcopy(release)
    floating_upgrade_fixture["upgrade_fixture"]["native"]["linux-x86_64"]["url"] = (
        "https://github.com/trailbaseio/trailbase/releases/latest/download/trail.zip"
    )
    mutations.append(floating_upgrade_fixture)

    wrong_oci_platform = copy.deepcopy(release)
    wrong_oci_platform["oci"]["platform_manifests"]["linux-amd64"][
        "manifest_digest"
    ] = "latest"
    mutations.append(wrong_oci_platform)

    missing_oci_layer = copy.deepcopy(release)
    missing_oci_layer["oci"]["platform_manifests"]["linux-arm64"]["layers"] = []
    mutations.append(missing_oci_layer)

    for index, mutation in enumerate(mutations, start=1):
        try:
            validate_release(mutation)
        except ReleaseError:
            continue
        raise ReleaseError(f"mutation sentinel {index} did not fail")
    generated = _new_admin_password()
    if len(generated) != 32 or not all(marker in generated for marker in "Aa1!"):
        raise ReleaseError("administrator password generator sentinel failed")
    match = BOOTSTRAP_PASSWORD.fullmatch("\tpassword: 'temporary-secret'")
    if match is None or match.group(1) != "temporary-secret":
        raise ReleaseError("bootstrap credential parser sentinel failed")
    try:
        _oci_runtime_executable("unsupported")
    except ReleaseError:
        pass
    else:
        raise ReleaseError("OCI runtime allowlist sentinel did not fail")
    master_fd, terminal_fd = os.openpty()
    try:
        _deliver_admin_password_to_tty(terminal_fd, "self-test-secret")  # nosec B106 -- self-test sentinel only.
        delivered = os.read(master_fd, 4096)
        if b"One-time delivered password: self-test-secret" not in delivered:
            raise ReleaseError("administrator credential delivery sentinel failed")
    finally:
        os.close(master_fd)
        os.close(terminal_fd)
    read_fd, write_fd = os.pipe()
    try:
        try:
            _deliver_admin_password_to_tty(write_fd, "must-not-be-delivered")  # nosec B106 -- self-test sentinel only.
        except ReleaseError:
            pass
        else:
            raise ReleaseError("non-terminal credential delivery sentinel did not fail")
    finally:
        os.close(read_fd)
        os.close(write_fd)
    _loopback_address("127.0.0.1:4000", "test address")
    try:
        _loopback_address("192.0.2.1:4000", "test address")
    except ReleaseError:
        pass
    else:
        raise ReleaseError("bootstrap non-loopback mutation sentinel failed")
    _validate_listener_configuration(
        "http://127.0.0.1:4000",
        "127.0.0.1:4000",
        "127.0.0.1:4001",
        "http://127.0.0.1:4000",
    )
    for public_url, address, admin_address, cors_origin in [
        (
            "https://example.invalid",
            "127.0.0.1:4000",
            "127.0.0.1:4001",
            "https://example.invalid",
        ),
        (
            "http://127.0.0.1:4002",
            "127.0.0.1:4000",
            "127.0.0.1:4001",
            "http://127.0.0.1:4002",
        ),
    ]:
        try:
            _validate_listener_configuration(public_url, address, admin_address, cors_origin)
        except ReleaseError:
            pass
        else:
            raise ReleaseError("non-loopback or mismatched public URL self-test did not fail")
    _backup_restore_self_test()
    _runtime_lock_self_test()
    _managed_process_group_self_test()
    print("PASS: TrailBase release-lock mutation sentinels")


def main() -> int:
    install_termination_cleanup()
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)
    subcommands.add_parser("verify-release", help="validate the checked-in exact release lock")
    verify_archive_parser = subcommands.add_parser(
        "verify-archive", help="validate an exact native release archive and executable"
    )
    verify_archive_parser.add_argument("archive", type=Path)
    verify_archive_parser.add_argument("--target", choices=["linux-aarch64", "linux-x86_64"])
    prepare_parser = subcommands.add_parser(
        "prepare-native", help="cache and install the exact native TrailBase release"
    )
    prepare_parser.add_argument("root", type=Path)
    prepare_parser.add_argument("--offline", action="store_true")
    prepare_upgrade_parser = subcommands.add_parser(
        "prepare-upgrade-fixture",
        help="cache and install the exact prior TrailBase upgrade fixture",
    )
    prepare_upgrade_parser.add_argument("root", type=Path)
    prepare_upgrade_parser.add_argument("--offline", action="store_true")
    prepare_oci_parser = subcommands.add_parser(
        "prepare-oci", help="pull and verify the exact OCI index, platform, config, and layers"
    )
    prepare_oci_parser.add_argument("root", type=Path)
    prepare_oci_parser.add_argument("--runtime", choices=["podman", "docker"], required=True)
    prepare_oci_parser.add_argument("--offline", action="store_true")
    prepare_runtime_lock_parser = subcommands.add_parser(
        "prepare-runtime-lock", help="create or repair the stopped owner-only runtime lock"
    )
    prepare_runtime_lock_parser.add_argument("root", type=Path)
    verify_oci_container_parser = subcommands.add_parser(
        "verify-oci-container", help="verify a running container uses the exact release image"
    )
    verify_oci_container_parser.add_argument("root", type=Path)
    verify_oci_container_parser.add_argument(
        "--runtime", choices=["podman", "docker"], required=True
    )
    verify_oci_container_parser.add_argument("--name", required=True)
    bootstrap_parser = subcommands.add_parser(
        "bootstrap-native", help="initialize and rotate the private TrailBase administrator"
    )
    bootstrap_parser.add_argument("root", type=Path)
    bootstrap_parser.add_argument("--public-url", required=True)
    bootstrap_parser.add_argument("--address", required=True)
    bootstrap_parser.add_argument("--admin-address", required=True)
    bootstrap_parser.add_argument("--cors-origin", required=True)
    run_parser = subcommands.add_parser(
        "run-native", help="exec the verified native runtime under the private-root lock"
    )
    run_parser.add_argument("root", type=Path)
    run_parser.add_argument("--public-url", required=True)
    run_parser.add_argument("--address", required=True)
    run_parser.add_argument("--admin-address", required=True)
    run_parser.add_argument("--cors-origin", required=True)
    verify_root_parser = subcommands.add_parser(
        "verify-root", help="verify a bootstrapped owner-only TrailBase development root"
    )
    verify_root_parser.add_argument("root", type=Path)
    backup_parser = subcommands.add_parser(
        "backup-depot", help="create a stopped, complete, digest-bound depot backup"
    )
    backup_parser.add_argument("root", type=Path)
    backup_parser.add_argument("output_dir", type=Path)
    restore_parser = subcommands.add_parser(
        "restore-depot", help="verify and restore a complete depot to an isolated target"
    )
    restore_parser.add_argument("archive", type=Path)
    restore_parser.add_argument("target", type=Path)
    subcommands.add_parser("self-test", help="prove release-lock mutations fail closed")
    arguments = parser.parse_args()
    try:
        release = load_release()
        if arguments.command == "verify-release":
            print(
                "PASS: TrailBase "
                f"v{release['version']} release lock; "
                f"OCI={release['oci']['index_digest']}"
            )
        elif arguments.command == "verify-archive":
            verify_archive(arguments.archive, release, arguments.target)
            print(f"PASS: exact TrailBase archive {arguments.archive}")
        elif arguments.command == "prepare-native":
            executable = prepare_native(arguments.root, arguments.offline)
            print(executable)
        elif arguments.command == "prepare-upgrade-fixture":
            executable = prepare_upgrade_fixture(arguments.root, arguments.offline)
            print(executable)
        elif arguments.command == "prepare-oci":
            print(prepare_oci(arguments.root, arguments.runtime, arguments.offline))
        elif arguments.command == "prepare-runtime-lock":
            prepare_runtime_lock(arguments.root)
            print(f"PASS: owner-only TrailBase runtime lock {arguments.root}")
        elif arguments.command == "verify-oci-container":
            verify_oci_container(arguments.root, arguments.runtime, arguments.name)
            print(f"PASS: exact running TrailBase OCI container {arguments.name}")
        elif arguments.command == "bootstrap-native":
            bootstrap_native(
                arguments.root,
                arguments.public_url,
                arguments.address,
                arguments.admin_address,
                arguments.cors_origin,
            )
            print("PASS: TrailBase administrator initialized and rotated")
        elif arguments.command == "run-native":
            run_native(
                arguments.root,
                arguments.public_url,
                arguments.address,
                arguments.admin_address,
                arguments.cors_origin,
            )
        elif arguments.command == "verify-root":
            verify_runtime_lock(arguments.root)
            verify_private_root(arguments.root)
            print("PASS: owner-only TrailBase root and bootstrap receipt")
        elif arguments.command == "backup-depot":
            path, digest = backup_depot(arguments.root, arguments.output_dir)
            print(json.dumps({"archive": str(path), "sha256": digest}, sort_keys=True))
        elif arguments.command == "restore-depot":
            restore_depot(arguments.archive, arguments.target)
            print(f"PASS: isolated TrailBase depot restored to {arguments.target}")
        elif arguments.command == "self-test":
            self_test()
        return 0
    except (OSError, ReleaseError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        print(f"FAIL: {error}", file=os.sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
