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
import stat
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
PHYSICAL_PROFILES_PATH = B1_DIR / "physical-profiles.json"
HARNESS_VERSION = "fasti-b1-benchmark.v3"
EVIDENCE_SCHEMA_VERSION = "fasti.b1.performance-evidence.v3"
MACHINE_FINGERPRINT_DOMAIN = b"fasti:b1:machine-fingerprint:v1\0"
MINIMUM_REPETITIONS = 5
GOVERNED_DOCKERFILE = B1_DIR / "Dockerfile"
LOCKED_BUDGETS = json.loads(BUDGETS_PATH.read_text(encoding="utf-8"))
PHYSICAL_PROFILES = json.loads(PHYSICAL_PROFILES_PATH.read_text(encoding="utf-8"))
PROFILE_DERIVATION = PHYSICAL_PROFILES["hardware_profile_derivation"]
PI_PROFILE = PHYSICAL_PROFILES["profiles"]["raspberry_pi_5_champion"]
J4125_PROFILE = PHYSICAL_PROFILES["profiles"]["j4125_calibrated"]
J4125_CGROUP_LIMIT_BYTES = J4125_PROFILE["oci"]["memory_limit_bytes"]
IDLE_WARMUP_SECONDS = float(LOCKED_BUDGETS["timing_seconds"]["idle_warmup"])
IDLE_MEASUREMENT_SECONDS = float(
    LOCKED_BUDGETS["timing_seconds"]["idle_measurement"]
)
IDLE_CPU_AVERAGE_LIMIT_PERCENT = float(
    LOCKED_BUDGETS["idle_cpu_percent_one_core"]["average"]
)
IDLE_CPU_P95_LIMIT_PERCENT = float(
    LOCKED_BUDGETS["idle_cpu_percent_one_core"]["p95"]
)
ARTIFACT_LIMITS = LOCKED_BUDGETS["artifact_bytes"]
SAMPLE_INTERVAL_MS = int(LOCKED_BUDGETS["timing_seconds"]["sample_interval_ms"])
IMAGE_SOURCE_LABELS = {
    "git_commit": "org.opencontainers.image.revision",
    "git_tree": "dev.scrobble.fasti.source.tree",
    "contract_ref": "dev.scrobble.fasti.contracts",
    "build_recipe_sha256": "dev.scrobble.fasti.build.recipe.sha256",
    "build_context_archive_sha256": "dev.scrobble.fasti.build.context.archive.sha256",
}
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


def sha256_regular_file(path: Path, label: str) -> tuple[str, int]:
    """Hash one retained regular file through a no-follow descriptor."""

    if not isinstance(getattr(os, "O_NOFOLLOW", None), int):
        raise CaptureError(f"{label} verification requires O_NOFOLLOW support")
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    except OSError as error:
        raise CaptureError(
            f"{label} cannot be opened without following links: {path}: {error}"
        ) from error
    digest = hashlib.sha256()
    size = 0
    try:
        metadata_before = os.fstat(descriptor)
        if not stat.S_ISREG(metadata_before.st_mode):
            raise CaptureError(f"{label} must be a retained regular file: {path}")
        with os.fdopen(descriptor, "rb", closefd=False) as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                digest.update(chunk)
        metadata_after = os.fstat(descriptor)
        stable_identity_before = (
            metadata_before.st_dev,
            metadata_before.st_ino,
            metadata_before.st_size,
            metadata_before.st_mtime_ns,
            metadata_before.st_ctime_ns,
        )
        stable_identity_after = (
            metadata_after.st_dev,
            metadata_after.st_ino,
            metadata_after.st_size,
            metadata_after.st_mtime_ns,
            metadata_after.st_ctime_ns,
        )
        if (
            size != metadata_before.st_size
            or stable_identity_after != stable_identity_before
        ):
            raise CaptureError(f"{label} changed while it was hashed: {path}")
    finally:
        os.close(descriptor)
    return digest.hexdigest(), size


def sha256_file(path: Path) -> str:
    digest, _ = sha256_regular_file(path, "file")
    return digest


def hash_retained_os_image(path: Path) -> dict[str, Any]:
    digest, size = sha256_regular_file(path, "retained OS image")
    if size <= 0:
        raise CaptureError(f"retained OS image is empty: {path}")
    return {"file_name": path.name, "size_bytes": size, "sha256": digest}


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


def parse_os_image() -> dict[str, str | None]:
    path = Path("/etc/os-release")
    if not path.is_file():
        raise CaptureError("/etc/os-release is required for the OS image fingerprint")
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" not in line or line.startswith("#"):
            continue
        key, value = line.split("=", 1)
        values[key] = value.strip().strip('"')
    required = ["PRETTY_NAME", "ID", "VERSION_ID", "VERSION_CODENAME"]
    missing = [key for key in required if not values.get(key)]
    if missing:
        raise CaptureError(f"OS image fingerprint is missing {', '.join(missing)} in /etc/os-release")
    return {
        "pretty_name": values["PRETTY_NAME"],
        "id": values["ID"],
        "version_id": values["VERSION_ID"],
        "version_codename": values["VERSION_CODENAME"],
        "build_id": values.get("BUILD_ID"),
        "image_id": values.get("IMAGE_ID"),
        "image_version": values.get("IMAGE_VERSION"),
    }


def reject_placeholder(label: str, value: str) -> str:
    normalized = " ".join(value.split())
    if len(normalized) < 3 or not re.search(r"[A-Za-z0-9]", normalized):
        raise CaptureError(f"{label} must be a meaningful stable value")
    if re.search(r"(?:^|[^a-z0-9])(tbd|todo|placeholder|unknown|unassigned|example)(?:$|[^a-z0-9])", normalized.casefold()):
        raise CaptureError(f"{label} cannot be a placeholder: {value!r}")
    if normalized.casefold() in {"n/a", "na", "none", "null", "runner", "custodian", "test"}:
        raise CaptureError(f"{label} cannot be a generic value: {value!r}")
    return normalized


def read_machine_id() -> bytes:
    for path in [Path("/etc/machine-id"), Path("/var/lib/dbus/machine-id")]:
        try:
            value = path.read_text(encoding="ascii").strip()
        except (FileNotFoundError, PermissionError):
            continue
        if re.fullmatch(r"[0-9a-fA-F]{32}", value):
            return value.casefold().encode("ascii")
    raise CaptureError("a valid local machine-id is required to derive the privacy-safe runner fingerprint")


def stable_machine_fingerprint(
    cpu_model: str,
    device_model: str | None,
    dmi: dict[str, str],
    storage_fingerprint_sha256: str,
) -> str:
    payload = json.dumps(
        {
            "machine_id": read_machine_id().decode("ascii"),
            "cpu_model": cpu_model,
            "device_model": device_model,
            "dmi": dmi,
            "storage_fingerprint_sha256": storage_fingerprint_sha256,
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(MACHINE_FINGERPRINT_DOMAIN + payload).hexdigest()


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


def parse_device_tree_identity() -> dict[str, Any] | None:
    bases = [Path("/proc/device-tree"), Path("/sys/firmware/devicetree/base")]
    for base in bases:
        try:
            model_bytes = (base / "model").read_bytes()
            compatible_bytes = (base / "compatible").read_bytes()
        except (FileNotFoundError, PermissionError):
            continue
        model = model_bytes.rstrip(b"\x00").decode("utf-8", errors="strict").strip()
        compatible = [
            item.decode("ascii", errors="strict")
            for item in compatible_bytes.split(b"\x00")
            if item
        ]
        if not model or not compatible or len(compatible) != len(set(compatible)):
            raise CaptureError(f"device-tree model/compatible evidence is incomplete: {base}")
        return {
            "source": str(base),
            "model": model,
            "compatible": compatible,
        }
    return None


def parse_device_model() -> str | None:
    identity = parse_device_tree_identity()
    return identity["model"] if identity is not None else None


def derive_hardware_profile(
    cpu_model: str,
    device_model: str | None,
    device_tree_compatible: list[str] | None = None,
) -> str:
    """Derive a target profile only from host-observed fingerprint fields."""

    if (
        device_model is not None
        and re.fullmatch(PI_PROFILE["device_tree"]["model_pattern"], device_model)
        and set(PI_PROFILE["device_tree"]["required_compatible"]).issubset(
            set(device_tree_compatible or [])
        )
    ):
        return "raspberry_pi_5_champion"
    if re.search(J4125_PROFILE["cpu_model_pattern"], cpu_model, re.IGNORECASE):
        return "j4125_calibrated"
    return "unclassified"


def parse_cpu_flags() -> set[str]:
    path = Path("/proc/cpuinfo")
    if not path.is_file():
        raise CaptureError("/proc/cpuinfo is required for virtualization checks")
    flags: set[str] = set()
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        key, separator, value = line.partition(":")
        if separator and key.strip().lower() in {"flags", "features"}:
            flags.update(value.casefold().split())
    return flags


def parse_dmi_identity() -> dict[str, str]:
    result: dict[str, str] = {}
    for field in ["sys_vendor", "product_name", "product_version", "board_vendor", "board_name"]:
        path = Path("/sys/class/dmi/id") / field
        try:
            value = path.read_text(encoding="utf-8", errors="replace").strip()
        except (FileNotFoundError, PermissionError):
            continue
        if value:
            result[field] = value
    return result


def parse_firmware_identity(hardware_profile: str) -> dict[str, str]:
    if hardware_profile == "raspberry_pi_5_champion":
        require_command("vcgencmd")
        version = run_checked(["vcgencmd", "version"])
        if not version:
            raise CaptureError("vcgencmd returned no Raspberry Pi firmware version")
        return {
            "source": "vcgencmd version",
            "description": version,
            "sha256": hashlib.sha256(version.encode("utf-8")).hexdigest(),
        }

    values: list[str] = []
    for field in ["bios_vendor", "bios_version", "bios_date"]:
        path = Path("/sys/class/dmi/id") / field
        try:
            value = path.read_text(encoding="utf-8", errors="replace").strip()
        except (FileNotFoundError, PermissionError):
            continue
        if value:
            values.append(f"{field}={value}")
    if not values:
        raise CaptureError("DMI BIOS vendor/version/date are required for the firmware fingerprint")
    description = "; ".join(values)
    return {
        "source": "/sys/class/dmi/id/{bios_vendor,bios_version,bios_date}",
        "description": description,
        "sha256": hashlib.sha256(description.encode("utf-8")).hexdigest(),
    }


def parse_cpu_governors() -> dict[str, Any]:
    paths = sorted(Path("/sys/devices/system/cpu").glob("cpu[0-9]*/cpufreq/scaling_governor"))
    if not paths:
        raise CaptureError("CPU governor evidence is unavailable under /sys/devices/system/cpu")
    values: list[str] = []
    for path in paths:
        value = path.read_text(encoding="ascii").strip()
        if not value:
            raise CaptureError(f"CPU governor value is empty: {path}")
        values.append(value)
    return {
        "source": "/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor",
        "observed": sorted(set(values)),
        "cpu_count_observed": len(paths),
    }


def parse_temperature() -> dict[str, Any]:
    candidates: list[tuple[str, Path]] = []
    for zone in sorted(Path("/sys/class/thermal").glob("thermal_zone*")):
        try:
            sensor = (zone / "type").read_text(encoding="ascii").strip()
        except (FileNotFoundError, PermissionError):
            sensor = zone.name
        candidates.append((sensor, zone / "temp"))
    preferred = sorted(
        candidates,
        key=lambda item: (
            not any(marker in item[0].casefold() for marker in ["cpu", "soc", "x86_pkg_temp"]),
            item[0],
        ),
    )
    for sensor, path in preferred:
        try:
            raw = float(path.read_text(encoding="ascii").strip())
        except (FileNotFoundError, PermissionError, ValueError):
            continue
        celsius = raw / 1000 if abs(raw) > 500 else raw
        if -20 <= celsius <= 125:
            return {
                "source": str(path),
                "sensor": sensor,
                "celsius": round(celsius, 3),
            }
    raise CaptureError("no plausible CPU/SoC thermal reading was available in /sys/class/thermal")


def top_level_block_device(source: str) -> str:
    if not source.startswith("/dev/"):
        raise CaptureError(f"root filesystem source is not a physical block device: {source!r}")
    current_path = source
    visited: set[str] = set()
    while current_path not in visited:
        visited.add(current_path)
        output = run_checked(["lsblk", "-dnro", "PKNAME", current_path])
        parents = sorted(
            {
                line.strip().removeprefix("/dev/")
                for line in output.splitlines()
                if line.strip()
            }
        )
        if not parents:
            return Path(current_path).name
        if len(parents) != 1:
            raise CaptureError(
                f"root block device has ambiguous or multiple backing parents: {parents}"
            )
        parent = parents[0]
        if parent in {".", ".."} or Path(parent).name != parent:
            raise CaptureError(f"root block-device parent name is unsafe: {parent!r}")
        current_path = f"/dev/{parent}"
    raise CaptureError(f"block-device parent chain contains a cycle: {source}")


def usb_link_speed_mbps(block_device: str) -> float | None:
    sys_device = Path("/sys/class/block") / block_device / "device"
    try:
        resolved = sys_device.resolve(strict=True)
    except (FileNotFoundError, PermissionError):
        return None
    for parent in [resolved, *resolved.parents]:
        speed_path = parent / "speed"
        try:
            value = float(speed_path.read_text(encoding="ascii").strip())
        except (FileNotFoundError, PermissionError, ValueError):
            continue
        if value > 0:
            return value
    return None


def udev_storage_properties(block_device: str) -> dict[str, str]:
    require_command("udevadm")
    output = run_checked(
        ["udevadm", "info", "--query=property", "--name", f"/dev/{block_device}"]
    )
    allowed = {
        "ID_BUS",
        "ID_TYPE",
        "ID_ATA",
        "ID_ATA_ROTATION_RATE_RPM",
        "ID_SSD",
        "ID_DRIVE_FLASH",
        "ID_DRIVE_FLASH_SD",
        "ID_DRIVE_FLASH_MMC",
    }
    result: dict[str, str] = {}
    for line in output.splitlines():
        key, separator, value = line.partition("=")
        if separator and key in allowed and value:
            result[key] = value
    return result


def classify_storage(
    transport: str, rotational: bool, udev: dict[str, str]
) -> tuple[str, list[str]]:
    if rotational:
        return "hdd", ["lsblk.ROTA=1"]
    flash_markers = [
        key for key in ["ID_DRIVE_FLASH", "ID_DRIVE_FLASH_SD", "ID_DRIVE_FLASH_MMC"]
        if udev.get(key) == "1"
    ]
    if flash_markers:
        return "emmc_or_flash", [f"udev.{key}=1" for key in flash_markers]
    if transport == "nvme":
        return "ssd", ["lsblk.TRAN=nvme", "lsblk.ROTA=0"]
    if udev.get("ID_SSD") == "1" or udev.get("ID_ATA_ROTATION_RATE_RPM") == "0":
        markers = ["lsblk.ROTA=0"]
        if udev.get("ID_SSD") == "1":
            markers.append("udev.ID_SSD=1")
        if udev.get("ID_ATA_ROTATION_RATE_RPM") == "0":
            markers.append("udev.ID_ATA_ROTATION_RATE_RPM=0")
        return "ssd", markers
    return "unknown_non_rotational", ["lsblk.ROTA=0", "no_exact_ssd_marker"]


def parse_storage_identity() -> tuple[dict[str, Any], str]:
    require_command("findmnt")
    require_command("lsblk")
    root_fields = run_checked(["findmnt", "-n", "-o", "SOURCE,FSTYPE,OPTIONS", "-T", "/"])
    parts = root_fields.split(maxsplit=2)
    if len(parts) != 3:
        raise CaptureError(f"unexpected findmnt root-filesystem output: {root_fields!r}")
    source, filesystem_type, mount_options = parts
    block = top_level_block_device(source)
    properties_text = run_checked(
        [
            "lsblk",
            "-dnPb",
            "-o",
            "NAME,TYPE,TRAN,ROTA,SIZE,MODEL,SERIAL,WWN",
            f"/dev/{block}",
        ]
    )
    properties: dict[str, str] = {}
    for match in re.finditer(r'(\w+)="([^"]*)"', properties_text):
        properties[match.group(1)] = match.group(2).strip()
    required = ["NAME", "TYPE", "ROTA", "SIZE"]
    if any(not properties.get(field) for field in required):
        raise CaptureError(f"lsblk did not return a complete storage fingerprint: {properties!r}")
    stable_secret = json.dumps(
        {
            key: properties.get(key, "")
            for key in ["NAME", "TYPE", "TRAN", "ROTA", "SIZE", "MODEL", "SERIAL", "WWN"]
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    fingerprint = hashlib.sha256(MACHINE_FINGERPRINT_DOMAIN + b"storage\0" + stable_secret).hexdigest()
    transport = properties.get("TRAN") or "unknown"
    rotational = properties["ROTA"] == "1"
    udev = udev_storage_properties(block)
    storage_class, classification_evidence = classify_storage(
        transport, rotational, udev
    )
    speed = usb_link_speed_mbps(block) if transport == "usb" else None
    evidence = {
        "root_source": source,
        "root_filesystem_type": filesystem_type,
        "root_mount_options": sorted(option for option in mount_options.split(",") if option),
        "physical_device": f"/dev/{block}",
        "device_type": properties["TYPE"],
        "transport": transport,
        "storage_class": storage_class,
        "classification_evidence": classification_evidence,
        "rotational": rotational,
        "size_bytes": int(properties["SIZE"]),
        "model": properties.get("MODEL") or "not_reported",
        "usb_link_speed_mbps": speed,
        "identity_sha256": fingerprint,
        "raw_serial_recorded": False,
    }
    return evidence, fingerprint


def parse_pi_overclock_configuration() -> dict[str, Any]:
    require_command("vcgencmd")
    output = run_checked(["vcgencmd", "get_config", "int"])
    configured: dict[str, int] = {}
    for line in output.splitlines():
        key, separator, value = line.partition("=")
        if separator and re.fullmatch(r"-?\d+", value.strip()):
            configured[key.strip()] = int(value.strip())
    policy = PI_PROFILE["overclock"]
    forbidden: dict[str, int] = {}
    for key, value in configured.items():
        if any(
            key.startswith(prefix)
            for prefix in policy["forbidden_nonzero_prefixes"]
        ) and value != 0:
            forbidden[key] = value
        for exact_key, allowed in policy["allowed_exact_values"].items():
            if (key == exact_key or key.startswith(f"{exact_key}.")) and value not in allowed:
                forbidden[key] = value
    if forbidden:
        raise CaptureError(f"Raspberry Pi overclock-related firmware settings are not stock: {forbidden!r}")
    return {
        "source": "vcgencmd get_config int",
        "status": policy["status"],
        "policy_sha256": sha256_file(PHYSICAL_PROFILES_PATH),
        "checked_keys": sorted(
            [*policy["allowed_exact_values"], *policy["forbidden_nonzero_prefixes"]]
        ),
    }


def parse_pi_active_cooling() -> dict[str, Any]:
    observed: list[str] = []
    for path in sorted(Path("/sys/class/thermal").glob("cooling_device*/type")):
        try:
            value = path.read_text(encoding="ascii").strip()
        except (FileNotFoundError, PermissionError):
            continue
        if value:
            observed.append(value)
    fan_types = sorted({value for value in observed if "fan" in value.casefold()})
    if not fan_types:
        raise CaptureError("Raspberry Pi champion requires a kernel-visible active fan cooling device")
    return {
        "source": "/sys/class/thermal/cooling_device*/type",
        "status": "active",
        "fan_types": fan_types,
    }


def validate_profile_requirements(
    hardware_profile: str,
    os_image: dict[str, str | None],
    architecture: str,
    logical_cpu_count: int,
    total_memory_bytes: int,
    storage: dict[str, Any],
    retained_os_image: dict[str, Any],
) -> dict[str, Any]:
    if hardware_profile == "raspberry_pi_5_champion":
        policy = PI_PROFILE
        if architecture not in policy["architectures"]:
            raise CaptureError(f"Raspberry Pi 5 champion requires 64-bit ARM, observed {architecture!r}")
        if logical_cpu_count != policy["logical_cpu_count"]:
            raise CaptureError(
                f"Raspberry Pi 5 champion requires exactly {policy['logical_cpu_count']} logical cores, observed {logical_cpu_count}"
            )
        runtime_os = policy["running_os_release"]
        if any(os_image[key] != runtime_os[key] for key in ["id", "version_codename"]):
            raise CaptureError(
                f"Raspberry Pi champion requires the locked runtime OS release fields, observed {os_image!r}"
            )
        approved_images = policy["approved_images"]
        if not approved_images:
            raise CaptureError(
                "Raspberry Pi champion OS image approval is blocked: no official image digest is pinned in benchmarks/b1/physical-profiles.json"
            )
        approved_image = next(
            (
                image
                for image in approved_images
                if image["sha256"] == retained_os_image["sha256"]
                and image["file_name"] == retained_os_image["file_name"]
            ),
            None,
        )
        if approved_image is None:
            raise CaptureError(
                "retained Raspberry Pi OS image filename and digest are not approved by benchmarks/b1/physical-profiles.json"
            )
        memory = policy["memory_bytes"]
        if not memory["minimum"] <= total_memory_bytes <= memory["maximum"]:
            raise CaptureError(f"Raspberry Pi champion requires the 4 GB model, observed {total_memory_bytes} bytes")
        storage_policy = policy["storage"]
        if (
            storage["transport"] != storage_policy["transport"]
            or storage["storage_class"] != storage_policy["class"]
        ):
            raise CaptureError("Raspberry Pi champion / filesystem must be on a positively identified USB SSD")
        if (storage["usb_link_speed_mbps"] or 0) < storage_policy["minimum_link_speed_mbps"]:
            raise CaptureError("Raspberry Pi champion USB SSD must negotiate at USB 3 speed (at least 5000 Mb/s)")
        return {
            "profile": hardware_profile,
            "profile_policy_sha256": sha256_file(PHYSICAL_PROFILES_PATH),
            "running_os_release": runtime_os,
            "retained_os_image_approval": "approved_by_canonical_digest_policy",
            "memory": memory["label"],
            "storage": storage_policy["label"],
            "cooling": parse_pi_active_cooling(),
            "overclock": parse_pi_overclock_configuration(),
            "mechanical_scope_note": "Runtime OS fields and the retained image digest are separate facts; the receipt claims no edition or image provenance beyond the canonical digest allowlist.",
        }
    if hardware_profile == "j4125_calibrated":
        policy = J4125_PROFILE
        if architecture not in policy["architectures"]:
            raise CaptureError(f"J4125 calibration requires x86_64, observed {architecture!r}")
        if logical_cpu_count != policy["logical_cpu_count"]:
            raise CaptureError(f"J4125 calibration requires exactly four logical cores, observed {logical_cpu_count}")
        storage_policy = policy["storage"]
        if (
            storage["storage_class"] != storage_policy["class"]
            or storage["transport"] not in storage_policy["accepted_transports"]
        ):
            raise CaptureError("J4125 calibration requires a positively identified SSD on an accepted transport")
        return {
            "profile": hardware_profile,
            "profile_policy_sha256": sha256_file(PHYSICAL_PROFILES_PATH),
            "cpu": "physical_j4125_four_core",
            "retained_os_image_approval": "retained_digest_recorded_no_profile_allowlist",
            "oci_memory_limit_bytes": policy["oci"]["memory_limit_bytes"],
            "oci_swap_limit_bytes": policy["oci"]["swap_limit_bytes"],
            "storage": storage_policy["label"],
            "mechanical_scope_note": "This fingerprints only the root filesystem used by the B1 benchmark; it makes no claim about a future application data path.",
        }
    raise CaptureError(f"B1 physical capture does not qualify profile {hardware_profile!r}")


def detect_systemd_virtualization() -> str:
    require_command("systemd-detect-virt")
    result = subprocess.run(
        ["systemd-detect-virt"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    observed = result.stdout.strip().casefold()
    if result.returncode == 0 and observed and observed != "none":
        return observed
    if result.returncode == 1 and observed in {"", "none"}:
        return "none"
    diagnostic = result.stderr.strip() or result.stdout.strip() or f"exit {result.returncode}"
    raise CaptureError(f"systemd-detect-virt could not establish physical hardware: {diagnostic}")


def physicality_evidence(
    hardware_profile: str,
    cpu_flags: set[str],
    dmi: dict[str, str],
    systemd_virtualization: str,
    device_tree: dict[str, Any] | None,
) -> dict[str, Any]:
    if hardware_profile == "raspberry_pi_5_champion":
        required_compatible = set(PI_PROFILE["device_tree"]["required_compatible"])
        if systemd_virtualization != PI_PROFILE["systemd_detect_virt"]:
            raise CaptureError(
                f"Raspberry Pi 5 qualification requires systemd-detect-virt to prove none, observed {systemd_virtualization!r}"
            )
        if "hypervisor" in cpu_flags:
            raise CaptureError("Raspberry Pi 5 qualification refused because virtualization is visible")
        if (
            device_tree is None
            or re.fullmatch(PI_PROFILE["device_tree"]["model_pattern"], device_tree["model"]) is None
            or not required_compatible.issubset(set(device_tree["compatible"]))
        ):
            raise CaptureError("Raspberry Pi 5 Model B requires exact model and BCM2712 compatible evidence")
        return {
            "status": "physical",
            "mechanism": "raspberry_pi_systemd_device_tree_cross_check",
            "systemd_detect_virt": systemd_virtualization,
            "cpu_hypervisor_flag": False,
            "dmi": None,
            "device_tree": device_tree,
        }
    if hardware_profile == "j4125_calibrated":
        if systemd_virtualization != J4125_PROFILE["systemd_detect_virt"]:
            raise CaptureError(
                f"J4125 qualification requires a physical runner; virtualization was {systemd_virtualization!r}"
            )
        if "hypervisor" in cpu_flags:
            raise CaptureError("J4125 qualification refused because the CPU hypervisor flag is present")
        required = {"sys_vendor", "product_name"}
        if not required.issubset(dmi):
            raise CaptureError("J4125 qualification requires DMI system vendor and product name")
        virtual_markers = {
            "bhyve",
            "bochs",
            "kvm",
            "microsoft corporation virtual machine",
            "parallels",
            "qemu",
            "virtualbox",
            "vmware",
            "xen",
        }
        identity = " ".join(dmi.values()).casefold()
        if any(marker in identity for marker in virtual_markers):
            raise CaptureError(f"J4125 DMI identifies virtualized hardware: {identity!r}")
        return {
            "status": "physical",
            "mechanism": "j4125_systemd_cpu_dmi_cross_check",
            "systemd_detect_virt": systemd_virtualization,
            "cpu_hypervisor_flag": False,
            "dmi": dmi,
            "device_tree": None,
        }
    if "hypervisor" in cpu_flags:
        raise CaptureError("device-tree hardware qualification refused because virtualization is visible")
    return {
        "status": "physical",
        "mechanism": "device_tree_model",
        "systemd_detect_virt": None,
        "cpu_hypervisor_flag": False,
        "dmi": None,
        "device_tree": device_tree,
    }


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


def ensure_clean_tree() -> tuple[str, str, str]:
    status = run_checked(["git", "status", "--porcelain=v1", "--untracked-files=all"])
    if status:
        raise CaptureError(
            "performance evidence requires a clean source tree; commit or remove every tracked and untracked change first"
        )
    commit = run_checked(["git", "rev-parse", "HEAD"])
    tree = run_checked(["git", "rev-parse", "HEAD^{tree}"])
    contract_ref = run_checked(["git", "rev-parse", "HEAD:contracts"])
    if any(re.fullmatch(r"[0-9a-f]{40}", value) is None for value in [commit, tree, contract_ref]):
        raise CaptureError("Git did not return full commit, tree, and HEAD:contracts object IDs")
    return commit, tree, contract_ref


def local_docker_socket_path(endpoint: str) -> Path:
    if not endpoint.startswith("unix://"):
        raise CaptureError(
            f"Docker must use a demonstrably local Unix socket; effective endpoint was {endpoint!r}"
        )
    socket_path = Path(endpoint.removeprefix("unix://"))
    if not socket_path.is_absolute():
        raise CaptureError(f"Docker Unix socket path must be absolute: {socket_path}")
    try:
        mode = socket_path.stat().st_mode
    except (FileNotFoundError, PermissionError) as error:
        raise CaptureError(f"Docker Unix socket is unavailable: {socket_path}: {error}") from error
    if not stat.S_ISSOCK(mode):
        raise CaptureError(f"Docker endpoint is not a Unix socket: {socket_path}")
    return socket_path


def verify_local_docker() -> dict[str, str]:
    context = run_checked(["docker", "context", "show"])
    configured_endpoint = json.loads(
        run_checked(
            [
                "docker",
                "context",
                "inspect",
                context,
                "--format",
                "{{json .Endpoints.docker.Host}}",
            ]
        )
    )
    environment_endpoint = os.environ.get("DOCKER_HOST")
    endpoint = environment_endpoint or configured_endpoint
    socket_path = local_docker_socket_path(endpoint)
    return {
        "context": context,
        "endpoint": endpoint,
        "socket_path": str(socket_path),
        "locality": "verified_local_unix_socket",
    }


def inspect_bound_image(image_ref: str, expected_source: dict[str, str]) -> dict[str, Any]:
    documents = json.loads(run_checked(["docker", "image", "inspect", image_ref]))
    if not isinstance(documents, list) or len(documents) != 1:
        raise CaptureError(f"Docker returned an unexpected image inspection for {image_ref!r}")
    document = documents[0]
    image_id = document.get("Id")
    if not isinstance(image_id, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", image_id) is None:
        raise CaptureError(f"Docker image has no immutable content ID: {image_ref}")
    labels = (document.get("Config") or {}).get("Labels") or {}
    observed_labels: dict[str, str] = {}
    for source_field, label_name in IMAGE_SOURCE_LABELS.items():
        expected = expected_source[source_field]
        observed = labels.get(label_name)
        if observed != expected:
            raise CaptureError(
                f"Docker image {image_ref!r} label {label_name!r} must equal {expected!r}, observed {observed!r}"
            )
        observed_labels[label_name] = observed
    return {"id": image_id, "source_labels": observed_labels}


def create_exact_git_archive_context(
    destination: Path, source_root: Path = ROOT
) -> dict[str, Any]:
    if destination.exists() or destination.is_symlink():
        raise CaptureError(f"verifier-owned build context must not already exist: {destination}")
    destination.mkdir(mode=0o700)
    archive = destination.parent / f"{destination.name}.tar"
    if archive.exists() or archive.is_symlink():
        raise CaptureError(f"verifier-owned Git archive path already exists: {archive}")
    archive_command = [
        "git",
        "archive",
        "--format=tar",
        "--output",
        str(archive),
        "HEAD",
    ]
    run_checked(archive_command, cwd=source_root)
    archive_sha256, archive_size = sha256_regular_file(
        archive, "verifier-owned exact HEAD Git archive"
    )
    entries = run_checked(["tar", "-tf", str(archive)]).splitlines()
    if not entries or any(
        not entry
        or entry.startswith("/")
        or ".." in Path(entry).parts
        for entry in entries
    ):
        raise CaptureError("exact HEAD Git archive contains an unsafe path")
    run_checked(
        [
            "tar",
            "--extract",
            "--file",
            str(archive),
            "--directory",
            str(destination),
            "--no-same-owner",
            "--no-same-permissions",
        ]
    )
    recipe_relative = Path("benchmarks/b1/Dockerfile")
    recipe = destination / recipe_relative
    live_recipe = source_root / recipe_relative
    try:
        recipe_mode = recipe.lstat().st_mode
    except FileNotFoundError as error:
        raise CaptureError("exact HEAD Git archive omits the governed Dockerfile") from error
    if not stat.S_ISREG(recipe_mode) or sha256_file(recipe) != sha256_file(live_recipe):
        raise CaptureError("archived governed Dockerfile differs from the clean HEAD recipe")
    return {
        "method": "verifier_owned_git_archive_head",
        "git_archive_sha256": archive_sha256,
        "git_archive_size_bytes": archive_size,
        "archive_command": command_text(archive_command),
        "archive_entry_count": len(entries),
    }


def governed_build_image(args: argparse.Namespace, context: dict[str, Any]) -> list[str]:
    with tempfile.TemporaryDirectory(prefix="fasti-b1-git-archive-") as temp_name:
        build_context = Path(temp_name) / "context"
        provenance = create_exact_git_archive_context(build_context)
        context["source"]["build_context"] = provenance
        context["source"]["build_context_archive_sha256"] = provenance[
            "git_archive_sha256"
        ]
        expected_source = {
            key: context["source"][key]
            for key in [
                "git_commit",
                "git_tree",
                "contract_ref",
                "build_recipe_sha256",
                "build_context_archive_sha256",
            ]
        }
        archived_recipe = build_context / GOVERNED_DOCKERFILE.relative_to(ROOT)
        command = [
            "docker",
            "build",
            "--file",
            str(archived_recipe),
            "--tag",
            args.image,
            "--build-arg",
            f"FASTI_SOURCE_COMMIT={expected_source['git_commit']}",
            "--build-arg",
            f"FASTI_SOURCE_TREE={expected_source['git_tree']}",
            "--build-arg",
            f"FASTI_CONTRACT_REF={expected_source['contract_ref']}",
            "--build-arg",
            f"FASTI_BUILD_RECIPE_SHA256={expected_source['build_recipe_sha256']}",
            "--build-arg",
            f"FASTI_BUILD_CONTEXT_ARCHIVE_SHA256={expected_source['build_context_archive_sha256']}",
            str(build_context),
        ]
        run_checked(command, timeout=args.build_timeout_seconds)
    image = inspect_bound_image(args.image, expected_source)
    args.immutable_image = image["id"]
    context["source"].update(
        {
            "oci_image_ref": args.image,
            "oci_image_id": image["id"],
            "oci_source_labels": image["source_labels"],
        }
    )
    return [provenance["archive_command"], command_text(command)]


def verify_capture_inputs_unchanged(args: argparse.Namespace, context: dict[str, Any]) -> None:
    commit, tree, contract_ref = ensure_clean_tree()
    source = context["source"]
    observed_source = {
        "git_commit": commit,
        "git_tree": tree,
        "contract_ref": contract_ref,
    }
    expected_git_source = {key: source[key] for key in observed_source}
    if observed_source != expected_git_source:
        raise CaptureError(
            f"source identity changed during capture: expected {expected_git_source!r}, observed {observed_source!r}"
        )
    expected_source = {
        **expected_git_source,
        "build_recipe_sha256": source["build_recipe_sha256"],
        "build_context_archive_sha256": source["build_context_archive_sha256"],
    }
    native_digest = sha256_file(args.native_binary)
    if native_digest != source["native_fastid_sha256"]:
        raise CaptureError("native fastid bytes changed during capture")
    docker_locality = verify_local_docker()
    if docker_locality != context["docker_locality"]:
        raise CaptureError("Docker context, endpoint, or local socket changed during capture")
    mutable_ref = inspect_bound_image(source["oci_image_ref"], expected_source)
    immutable_ref = inspect_bound_image(source["oci_image_id"], expected_source)
    if mutable_ref != immutable_ref or immutable_ref["id"] != source["oci_image_id"]:
        raise CaptureError("Docker image identity or source labels changed during capture")
    with tempfile.TemporaryDirectory(prefix="fasti-b1-git-archive-recheck-") as temp_name:
        rechecked = create_exact_git_archive_context(Path(temp_name) / "context")
    if rechecked["git_archive_sha256"] != source["build_context_archive_sha256"]:
        raise CaptureError("exact HEAD Git archive bytes changed during capture")


def preflight(args: argparse.Namespace) -> dict[str, Any]:
    if platform.system() != "Linux":
        raise CaptureError(
            f"B1 performance capture is Linux-only; {platform.system()} cannot provide the required /proc, netns, and cgroup-v2 evidence"
        )

    for command in ["curl", "docker", "findmnt", "git", "gzip", "ip", "lsblk", "node", "pnpm", "tar", "udevadm", "unshare"]:
        require_command(command)

    if args.command == "capture" and args.output.exists():
        raise CaptureError(f"refusing to overwrite existing evidence: {args.output}")

    commit, tree, contract_ref = ensure_clean_tree()
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

    docker_locality = verify_local_docker()
    docker_cgroup = run_checked(["docker", "info", "--format", "{{.CgroupVersion}}"])
    if docker_cgroup != "2":
        raise CaptureError(f"Docker must use cgroup v2, reported {docker_cgroup!r}")
    docker_version = run_checked(["docker", "version", "--format", "{{.Server.Version}}"])
    if not GOVERNED_DOCKERFILE.is_file():
        raise CaptureError(f"governed OCI build recipe is missing: {GOVERNED_DOCKERFILE}")
    build_recipe_sha256 = sha256_file(GOVERNED_DOCKERFILE)

    cpu_model = parse_cpu_model()
    device_tree = parse_device_tree_identity()
    device_model = device_tree["model"] if device_tree is not None else None
    hardware_profile = derive_hardware_profile(
        cpu_model,
        device_model,
        device_tree["compatible"] if device_tree is not None else None,
    )
    if hardware_profile == "unclassified":
        raise CaptureError(
            "runner fingerprint does not match a supported hardware profile; profile labels cannot be supplied by the operator"
        )
    cpu_flags = parse_cpu_flags()
    dmi = parse_dmi_identity()
    virtualization = detect_systemd_virtualization()
    physicality = physicality_evidence(
        hardware_profile, cpu_flags, dmi, virtualization, device_tree
    )
    os_image = parse_os_image()
    retained_os_image = hash_retained_os_image(args.os_image)
    architecture = platform.machine()
    logical_cpu_count = os.cpu_count() or 1
    total_memory_bytes = parse_total_memory_bytes()
    storage, storage_fingerprint = parse_storage_identity()
    firmware = parse_firmware_identity(hardware_profile)
    governor = parse_cpu_governors()
    temperature = {"preflight": parse_temperature(), "post_capture": None}
    profile_requirements = validate_profile_requirements(
        hardware_profile,
        os_image,
        architecture,
        logical_cpu_count,
        total_memory_bytes,
        storage,
        retained_os_image,
    )
    machine_fingerprint = stable_machine_fingerprint(
        cpu_model, device_model, dmi, storage_fingerprint
    )
    oci_memory_limit = (
        J4125_CGROUP_LIMIT_BYTES if hardware_profile == "j4125_calibrated" else None
    )
    args.oci_memory_limit_bytes = oci_memory_limit

    fingerprint_commands = [
        "uname -srmo",
        "read /etc/os-release:PRETTY_NAME",
        "read /proc/cpuinfo:first(model name|model|hardware)",
        "read /proc/device-tree/model or /sys/firmware/devicetree/base/model when present",
        "read /proc/meminfo:MemTotal",
        "read /etc/machine-id, retain only domain-separated SHA-256 fingerprint",
        "read /proc/cpuinfo:flags or features",
        "read /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor",
        "read /sys/class/thermal/thermal_zone*/{type,temp}",
        command_text(["findmnt", "-n", "-o", "SOURCE,FSTYPE,OPTIONS", "-T", "/"]),
        "read lsblk root-device parent chain and hash serial/WWN without retaining raw identifiers",
        "read allowlisted non-secret udev storage classification properties",
        command_text(["docker", "version", "--format", "{{.Server.Version}}"]),
        command_text(["docker", "info", "--format", "{{.CgroupVersion}}"]),
        command_text(["docker", "context", "show"]),
        command_text(
            [
                "docker",
                "context",
                "inspect",
                docker_locality["context"],
                "--format",
                "{{json .Endpoints.docker.Host}}",
            ]
        ),
        f"stat Unix socket {docker_locality['socket_path']}",
        command_text(unshare_test),
        command_text(["systemd-detect-virt"]),
        f"open retained OS image with O_NOFOLLOW and hash exact regular-file bytes: {args.os_image.name}",
    ]
    if hardware_profile == "j4125_calibrated":
        fingerprint_commands.extend(
            [
                "read /sys/class/dmi/id/{sys_vendor,product_name,product_version,board_vendor,board_name}",
                "read /sys/class/dmi/id/{bios_vendor,bios_version,bios_date}",
            ]
        )
    else:
        fingerprint_commands.extend(
            [
                command_text(["vcgencmd", "version"]),
                command_text(["vcgencmd", "get_config", "int"]),
                "read exact device-tree model and NUL-delimited compatible values",
                "read /sys/class/thermal/cooling_device*/type",
            ]
        )

    return {
        "runner": {
            "runner_id": args.runner_id,
            "machine_fingerprint_sha256": machine_fingerprint,
            "hardware_profile": hardware_profile,
            "hardware_profile_derivation": PROFILE_DERIVATION,
            "profile_policy_sha256": sha256_file(PHYSICAL_PROFILES_PATH),
            "physicality": physicality,
            "custodian": args.custodian,
            "os_release": parse_os_release(),
            "os_image": {
                **os_image,
                "claim_scope": "runtime_os_release_fields_only",
                "retained_image": retained_os_image,
                "approval": (
                    "approved_by_canonical_digest_policy"
                    if hardware_profile == "raspberry_pi_5_champion"
                    else "retained_digest_recorded_no_profile_allowlist"
                ),
            },
            "kernel_release": platform.release(),
            "architecture": architecture,
            "cpu_model": cpu_model,
            "device_model": device_model,
            "logical_cpu_count": logical_cpu_count,
            "total_memory_bytes": total_memory_bytes,
            "firmware": firmware,
            "root_filesystem": {
                "source": storage["root_source"],
                "type": storage["root_filesystem_type"],
                "mount_options": storage["root_mount_options"],
            },
            "storage": storage,
            "cpu_governor": governor,
            "temperature": temperature,
            "profile_requirements": profile_requirements,
            "cgroup_version": "v2",
            "cgroup": {
                "version": "v2",
                "oci_memory_limit_bytes": oci_memory_limit,
                "oci_swap_limit_bytes": 0 if oci_memory_limit is not None else None,
            },
            "container_engine": {
                "name": "docker",
                "version": docker_version,
                **docker_locality,
            },
        },
        "source": {
            "git_commit": commit,
            "git_tree": tree,
            "tree_state": "clean",
            "native_fastid_sha256": None,
            "native_artifact_origin": "extracted_from_immutable_oci_image",
            "oci_image_ref": None,
            "oci_image_id": None,
            "oci_source_labels": None,
            "contract_ref": contract_ref,
            "build_recipe_path": str(GOVERNED_DOCKERFILE.relative_to(ROOT)),
            "build_recipe_sha256": build_recipe_sha256,
            "profile_policy_path": str(PHYSICAL_PROFILES_PATH.relative_to(ROOT)),
            "profile_policy_sha256": sha256_file(PHYSICAL_PROFILES_PATH),
            "build_context": None,
        },
        "fingerprint_commands": fingerprint_commands,
        "docker_locality": docker_locality,
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


def process_cpu_runtime_ns(pid: int) -> int:
    try:
        fields = Path(f"/proc/{pid}/schedstat").read_text(encoding="ascii").split()
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        return 0
    if len(fields) < 1 or re.fullmatch(r"[0-9]+", fields[0]) is None:
        raise CaptureError(f"/proc/{pid}/schedstat has no runtime nanosecond counter")
    return int(fields[0])


def validate_container_cgroup_identity(relative: str, container_id: str) -> None:
    if re.fullmatch(r"[0-9a-f]{64}", container_id) is None:
        raise CaptureError(f"Docker returned an invalid container ID: {container_id!r}")
    if container_id not in relative:
        raise CaptureError(
            "Docker State.Pid is not correlated to a local cgroup containing the exact container ID"
        )


def cgroup_path_for_pid(pid: int, container_id: str) -> Path:
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
    validate_container_cgroup_identity(relative, container_id)
    path = Path("/sys/fs/cgroup") / relative.lstrip("/")
    for required in [
        "memory.current",
        "memory.peak",
        "memory.max",
        "memory.swap.max",
        "memory.events",
        "cpu.stat",
    ]:
        if not (path / required).is_file():
            raise CaptureError(f"container cgroup lacks {required}: {path}")
    return path


def cgroup_cpu_runtime_ns(path: Path) -> int:
    for line in (path / "cpu.stat").read_text(encoding="ascii").splitlines():
        key, value = line.split(maxsplit=1)
        if key == "usage_usec":
            return int(value) * 1000
    raise CaptureError(f"usage_usec is missing from {path / 'cpu.stat'}")


def parse_cgroup_limit(path: Path) -> int | None:
    value = path.read_text(encoding="ascii").strip()
    return None if value == "max" else int(value)


def cgroup_oom_kill_count(path: Path) -> int:
    for line in (path / "memory.events").read_text(encoding="ascii").splitlines():
        key, value = line.split(maxsplit=1)
        if key == "oom_kill":
            return int(value)
    raise CaptureError(f"oom_kill is missing from {path / 'memory.events'}")


def read_cgroup(path: Path) -> tuple[int, int, int, int | None, int | None, int]:
    current = int((path / "memory.current").read_text(encoding="ascii").strip())
    peak_text = (path / "memory.peak").read_text(encoding="ascii").strip()
    if peak_text == "max":
        raise CaptureError(f"unsupported unbounded memory.peak value in {path}")
    return (
        current,
        int(peak_text),
        cgroup_cpu_runtime_ns(path),
        parse_cgroup_limit(path / "memory.max"),
        parse_cgroup_limit(path / "memory.swap.max"),
        cgroup_oom_kill_count(path),
    )


class Sampler:
    def __init__(self, root_pid: int, interval_ms: int, cgroup_path: Path | None = None):
        self.root_pid = root_pid
        self.interval_seconds = interval_ms / 1000
        self.cgroup_path = cgroup_path
        self.records: list[dict[str, Any]] = []
        self.error: Exception | None = None
        self._stop = threading.Event()
        self._records_lock = threading.Lock()
        self._thread = threading.Thread(target=self._run, name="fasti-b1-sampler", daemon=True)
        self._previous_runtime_ns: dict[int, int] = {}
        self._accumulated_runtime_ns = 0

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        self._thread.join(timeout=5)
        if self._thread.is_alive():
            raise CaptureError("measurement sampler did not stop")
        if self.error is not None:
            raise CaptureError(f"measurement sampler failed: {self.error}") from self.error

    def records_snapshot(self) -> list[dict[str, Any]]:
        with self._records_lock:
            return list(self.records)

    def is_running(self) -> bool:
        return self._thread.is_alive() and self.error is None

    def _sample(self) -> None:
        pids = process_tree(self.root_pid)
        if not pids:
            return
        runtimes: dict[int, int] = {}
        for pid in pids:
            runtimes[pid] = process_cpu_runtime_ns(pid)
            previous = self._previous_runtime_ns.get(pid)
            if previous is None:
                self._accumulated_runtime_ns += runtimes[pid]
            elif runtimes[pid] >= previous:
                self._accumulated_runtime_ns += runtimes[pid] - previous
        self._previous_runtime_ns = runtimes

        record: dict[str, Any] = {
            "at_ns": time.monotonic_ns(),
            "rss_bytes": sum(process_rss_bytes(pid) for pid in pids),
            "cpu_runtime_ns": self._accumulated_runtime_ns,
            "process_count": len(pids),
        }
        if self.cgroup_path is not None:
            current, peak, cpu_runtime_ns, limit, swap_limit, oom_kills = read_cgroup(
                self.cgroup_path
            )
            record.update(
                {
                    "cgroup_current_bytes": current,
                    "cgroup_peak_bytes": peak,
                    "cgroup_cpu_runtime_ns": cpu_runtime_ns,
                    "cgroup_memory_limit_bytes": limit,
                    "cgroup_swap_limit_bytes": swap_limit,
                    "cgroup_oom_kill_count": oom_kills,
                }
            )
        with self._records_lock:
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


def steady_observation_span_ns(
    records: list[dict[str, Any]], ready_at_ns: int
) -> int:
    timestamps = [
        int(record["at_ns"])
        for record in records
        if int(record["at_ns"]) >= ready_at_ns
    ]
    return timestamps[-1] - timestamps[0] if len(timestamps) >= 2 else 0


def wait_for_observed_steady_span(
    sampler: Sampler,
    process_alive: callable,
    ready_at_ns: int,
    required_seconds: float,
) -> None:
    required_ns = round(required_seconds * 1_000_000_000)
    deadline = time.monotonic() + required_seconds + max(
        5.0, sampler.interval_seconds * 3
    )
    while True:
        observed_ns = steady_observation_span_ns(
            sampler.records_snapshot(), ready_at_ns
        )
        if observed_ns >= required_ns:
            return
        if not process_alive():
            raise CaptureError(
                "subject exited before the raw steady observations spanned the locked window"
            )
        if not sampler.is_running():
            if sampler.error is not None:
                raise CaptureError(
                    f"measurement sampler failed: {sampler.error}"
                ) from sampler.error
            raise CaptureError("measurement sampler stopped before the locked steady span")
        if time.monotonic() >= deadline:
            raise CaptureError(
                "raw steady observations did not span the locked measurement window"
            )
        time.sleep(min(0.1, max(sampler.interval_seconds / 4, 0.01)))


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
    records: list[dict[str, Any]],
    *,
    started_at_ns: int,
    ready_at_ns: int,
    finished_at_ns: int,
    with_cgroup: bool,
    idle_cpu_window: bool = False,
    startup_ready_at_ns: int | None = None,
) -> dict[str, Any]:
    if not records:
        raise CaptureError("no process-tree measurements were captured")
    steady = [record for record in records if int(record["at_ns"]) >= ready_at_ns]
    if len(steady) < 2:
        raise CaptureError("fewer than two steady-state samples were captured")
    if any(int(record["rss_bytes"]) <= 0 for record in steady):
        raise CaptureError("a steady-state process-tree RSS sample was missing")

    finished_at_ns = max(finished_at_ns, int(records[-1]["at_ns"]))
    elapsed_ns = max(finished_at_ns - started_at_ns, 1)
    elapsed = elapsed_ns / 1_000_000_000
    cpu_seconds = int(records[-1]["cpu_runtime_ns"]) / 1_000_000_000
    steady_rss = [int(record["rss_bytes"]) for record in steady]
    result: dict[str, Any] = {
        "startup_ms": round(
            ((startup_ready_at_ns or ready_at_ns) - started_at_ns) / 1_000_000,
            3,
        ),
        "steady_process_tree_rss_bytes": max(steady_rss),
        "steady_process_tree_rss_statistics": summarize(steady_rss),
        "peak_process_tree_rss_bytes": max(int(record["rss_bytes"]) for record in records),
        "process_tree_cpu_seconds": round(cpu_seconds, 6),
        "process_tree_cpu_percent": round((cpu_seconds / elapsed) * 100, 6),
        "process_count_peak": max(int(record["process_count"]) for record in records),
        "cgroup": None,
        "container_identity": None,
        "idle_cpu": None,
        "steady_started_elapsed_ns": ready_at_ns - started_at_ns,
        "finished_elapsed_ns": finished_at_ns - started_at_ns,
    }

    if with_cgroup:
        required = {
            "cgroup_current_bytes",
            "cgroup_peak_bytes",
            "cgroup_cpu_runtime_ns",
            "cgroup_memory_limit_bytes",
            "cgroup_swap_limit_bytes",
            "cgroup_oom_kill_count",
        }
        if any(not required.issubset(record) for record in steady):
            raise CaptureError("one or more cgroup-v2 measurements were missing")
        cgroup_cpu_runtime_ns = int(records[-1]["cgroup_cpu_runtime_ns"])
        steady_cgroup = [int(record["cgroup_current_bytes"]) for record in steady]
        result["cgroup"] = {
            "steady_memory_current_bytes": max(steady_cgroup),
            "steady_memory_current_statistics": summarize(steady_cgroup),
            "peak_memory_bytes": max(int(record["cgroup_peak_bytes"]) for record in records),
            "cpu_seconds": round(cgroup_cpu_runtime_ns / 1_000_000_000, 6),
            "cpu_percent": round(
                (cgroup_cpu_runtime_ns / elapsed_ns) * 100, 6
            ),
            "memory_limit_bytes": steady[-1]["cgroup_memory_limit_bytes"],
            "swap_limit_bytes": steady[-1]["cgroup_swap_limit_bytes"],
            "oom_kill_count": max(int(record["cgroup_oom_kill_count"]) for record in records),
        }
        if len({record["cgroup_memory_limit_bytes"] for record in steady}) != 1:
            raise CaptureError("cgroup memory.max changed during the measurement window")
        if len({record["cgroup_swap_limit_bytes"] for record in steady}) != 1:
            raise CaptureError("cgroup memory.swap.max changed during the measurement window")
        if result["cgroup"]["oom_kill_count"] != 0:
            raise CaptureError("the subject cgroup recorded one or more OOM kills")

    if idle_cpu_window:
        counter_key = "cgroup_cpu_runtime_ns" if with_cgroup else "cpu_runtime_ns"
        cpu_percent_intervals: list[float] = []
        for previous, current in zip(steady, steady[1:]):
            duration = int(current["at_ns"]) - int(previous["at_ns"])
            delta = int(current[counter_key]) - int(previous[counter_key])
            if duration <= 0 or delta < 0:
                raise CaptureError("idle CPU counter or timestamp was not monotonic")
            cpu_percent_intervals.append((delta / duration) * 100)
        if not cpu_percent_intervals:
            raise CaptureError("idle CPU measurement produced no intervals")
        measured_ns = int(steady[-1]["at_ns"]) - int(steady[0]["at_ns"])
        total_cpu_delta_ns = int(steady[-1][counter_key]) - int(steady[0][counter_key])
        ordered = sorted(cpu_percent_intervals)
        p95_index = max(0, min(len(ordered) - 1, int(len(ordered) * 0.95 + 0.999999) - 1))
        result["idle_cpu"] = {
            "counter_scope": "cgroup_v2_usage_usec" if with_cgroup else "native_process_tree_schedstat",
            "measurement_seconds": round(measured_ns / 1_000_000_000, 6),
            "average_percent_one_core": round(
                (total_cpu_delta_ns / measured_ns) * 100, 6
            ),
            "p95_percent_one_core": round(ordered[p95_index], 6),
            "interval_count": len(cpu_percent_intervals),
        }
    observations: list[dict[str, Any]] = []
    for sequence, record in enumerate(records, start=1):
        observation = {
            "sequence": sequence,
            "elapsed_ns": int(record["at_ns"]) - started_at_ns,
            "steady": int(record["at_ns"]) >= ready_at_ns,
            "process_tree_rss_bytes": int(record["rss_bytes"]),
            "process_tree_cpu_runtime_ns": int(record["cpu_runtime_ns"]),
            "process_count": int(record["process_count"]),
            "cgroup_memory_current_bytes": None,
            "cgroup_memory_peak_bytes": None,
            "cgroup_cpu_runtime_ns": None,
            "cgroup_memory_limit_bytes": None,
            "cgroup_swap_limit_bytes": None,
            "cgroup_oom_kill_count": None,
        }
        if with_cgroup:
            observation.update(
                {
                    "cgroup_memory_current_bytes": int(record["cgroup_current_bytes"]),
                    "cgroup_memory_peak_bytes": int(record["cgroup_peak_bytes"]),
                    "cgroup_cpu_runtime_ns": int(record["cgroup_cpu_runtime_ns"]),
                    "cgroup_memory_limit_bytes": record["cgroup_memory_limit_bytes"],
                    "cgroup_swap_limit_bytes": record["cgroup_swap_limit_bytes"],
                    "cgroup_oom_kill_count": int(record["cgroup_oom_kill_count"]),
                }
            )
        observations.append(observation)
    if len({item["elapsed_ns"] for item in observations}) != len(observations):
        raise CaptureError("measurement observations contain duplicate monotonic timestamps")
    result["observations"] = observations
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

        started_at_ns = time.monotonic_ns()
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
                service_ready_at_ns = time.monotonic_ns()
                if routes.read_text(encoding="utf-8").strip():
                    raise CaptureError(f"{scenario_id} network namespace unexpectedly has an IP route")
                if scenario_id == "native_fastid_idle":
                    payload = json.loads(health.read_text(encoding="utf-8"))
                    if payload.get("status") != "healthy" or not payload.get("version"):
                        raise CaptureError(f"native health response is invalid: {payload!r}")
                    wait_steady(lambda: process.poll() is None, args.idle_warmup_seconds)
                    ready_at_ns = time.monotonic_ns()
                    wait_for_observed_steady_span(
                        sampler,
                        lambda: process.poll() is None,
                        ready_at_ns,
                        args.idle_measurement_seconds,
                    )
                else:
                    ready_at_ns = service_ready_at_ns
                    wait_steady(lambda: process.poll() is None, args.steady_window_seconds)
                finished_at_ns = time.monotonic_ns()
            finally:
                try:
                    sampler.stop()
                finally:
                    stop_process_group(process)

        metrics = metrics_from_records(
            sampler.records_snapshot(),
            started_at_ns=started_at_ns,
            ready_at_ns=ready_at_ns,
            finished_at_ns=finished_at_ns,
            with_cgroup=False,
            idle_cpu_window=scenario_id == "native_fastid_idle",
            startup_ready_at_ns=service_ready_at_ns,
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


def oci_run_command(
    name: str,
    immutable_image: str,
    script: str,
    memory_limit_bytes: int | None = None,
) -> list[str]:
    if re.fullmatch(r"sha256:[0-9a-f]{64}", immutable_image) is None:
        raise CaptureError(f"OCI measurement requires an immutable image ID, got {immutable_image!r}")
    command = [
        "docker",
        "run",
        "--detach",
        "--name",
        name,
        "--network",
        "none",
    ]
    if memory_limit_bytes is not None:
        command.extend(
            [
                "--memory",
                str(memory_limit_bytes),
                "--memory-swap",
                str(memory_limit_bytes),
            ]
        )
    command.extend([
        "--entrypoint",
        "/bin/sh",
        immutable_image,
        "-c",
        script,
    ])
    return command


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

    run_command = oci_run_command(
        name,
        args.immutable_image,
        script,
        args.oci_memory_limit_bytes,
    )
    commands.append(command_text(run_command))
    started_at_ns = time.monotonic_ns()
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
        cgroup_path = cgroup_path_for_pid(pid, container_id)
        commands.extend(
            [
                command_text(["docker", "inspect", "--format", "{{.State.Pid}}", name]),
                f"read {cgroup_path / 'memory.current'} {cgroup_path / 'memory.peak'} {cgroup_path / 'cpu.stat'}",
                f"read {cgroup_path / 'memory.max'} {cgroup_path / 'memory.swap.max'} {cgroup_path / 'memory.events'}",
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

            service_ready_at_ns = time.monotonic_ns()
            if scenario_id == "oci_fastid_idle":
                wait_steady(lambda: docker_running(name), args.idle_warmup_seconds)
                ready_at_ns = time.monotonic_ns()
                wait_for_observed_steady_span(
                    sampler,
                    lambda: docker_running(name),
                    ready_at_ns,
                    args.idle_measurement_seconds,
                )
            else:
                ready_at_ns = service_ready_at_ns
                wait_steady(lambda: docker_running(name), args.steady_window_seconds)
            finished_at_ns = time.monotonic_ns()
        finally:
            sampler.stop()

        metrics = metrics_from_records(
            sampler.records_snapshot(),
            started_at_ns=started_at_ns,
            ready_at_ns=ready_at_ns,
            finished_at_ns=finished_at_ns,
            with_cgroup=True,
            idle_cpu_window=scenario_id == "oci_fastid_idle",
            startup_ready_at_ns=service_ready_at_ns,
        )
        if args.oci_memory_limit_bytes is not None:
            cgroup = metrics["cgroup"]
            if cgroup["memory_limit_bytes"] != args.oci_memory_limit_bytes:
                raise CaptureError(
                    f"J4125 OCI memory.max must be {args.oci_memory_limit_bytes}, observed {cgroup['memory_limit_bytes']}"
                )
            if cgroup["swap_limit_bytes"] != 0:
                raise CaptureError(
                    f"J4125 OCI memory.swap.max must be 0, observed {cgroup['swap_limit_bytes']}"
                )
        metrics["run"] = run_number
        metrics["container_identity"] = {
            "container_id": container_id,
            "host_pid": pid,
            "cgroup_path": str(cgroup_path),
        }
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


def publish_content_addressed_artifact(
    source: Path, receipt_parent: Path
) -> dict[str, Any]:
    digest, size = sha256_regular_file(source, "compressed benchmark artifact")
    artifact_root = receipt_parent / "artifacts" / "sha256"
    current = receipt_parent
    for segment in ["artifacts", "sha256"]:
        current = current / segment
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            current.mkdir(mode=0o755)
            metadata = current.lstat()
        if not stat.S_ISDIR(metadata.st_mode) or current.is_symlink():
            raise CaptureError(f"artifact publication path is not a real directory: {current}")
    destination = artifact_root / f"{digest}.tar.gz"
    try:
        os.link(source, destination, follow_symlinks=False)
    except FileExistsError:
        existing_digest, existing_size = sha256_regular_file(
            destination, "existing content-addressed benchmark artifact"
        )
        if existing_digest != digest or existing_size != size:
            raise CaptureError(
                f"content-addressed artifact path contains different bytes: {destination}"
            )
    return {
        "path": destination.relative_to(receipt_parent).as_posix(),
        "sha256": digest,
        "size_bytes": size,
    }


def artifact_sizes(
    args: argparse.Namespace,
) -> tuple[dict[str, Any], list[str], dict[str, Any]]:
    image_size_command = [
        "docker",
        "image",
        "inspect",
        "--format",
        "{{.Size}}",
        args.immutable_image,
    ]
    binary_size_command = [
        "docker",
        "run",
        "--rm",
        "--network",
        "none",
        "--entrypoint",
        "/bin/sh",
        args.immutable_image,
        "-c",
        "stat -c '%s %s' /usr/local/bin/fastid /usr/local/bin/fasti",
    ]
    binary_values = run_checked(binary_size_command).split()
    if len(binary_values) != 2:
        raise CaptureError(f"unexpected OCI binary size output: {binary_values!r}")
    with tempfile.TemporaryDirectory(
        prefix=".fasti-b1-artifact-sizes-", dir=args.output.parent
    ) as temp_name:
        temp = Path(temp_name)
        oci_archive = temp / "fasti-oci.tar.gz"
        contract_pack = temp / "fasti-contract-pack.tar.gz"
        docker_save = ["docker", "image", "save", args.immutable_image]
        contract_context = temp / "contract-context"
        contract_context_provenance = create_exact_git_archive_context(contract_context)
        sdk_install_command = ["pnpm", "--offline", "install", "--frozen-lockfile"]
        sdk_build_command = ["pnpm", "--offline", "--filter", "@fasti/sdk", "build"]
        run_checked(sdk_install_command, cwd=contract_context, timeout=300)
        run_checked(sdk_build_command, cwd=contract_context, timeout=300)
        contract_archive = [
            "tar",
            "--sort=name",
            "--mtime=@0",
            "--owner=0",
            "--group=0",
            "--numeric-owner",
            "-cf",
            "-",
            "contracts",
            "tests/conformance",
            "packages/sdk/dist",
            "packages/sdk/package.json",
            "packages/sdk/tsconfig.json",
        ]

        def gzip_pipeline(
            source_command: list[str], destination: Path, *, source_cwd: Path = ROOT
        ) -> None:
            with destination.open("wb") as output:
                source = subprocess.Popen(
                    source_command,
                    cwd=source_cwd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                )
                assert source.stdout is not None
                compressor = subprocess.Popen(
                    ["gzip", "-n", "-9"],
                    cwd=source_cwd,
                    stdin=source.stdout,
                    stdout=output,
                    stderr=subprocess.PIPE,
                )
                source.stdout.close()
                compressor_stderr = compressor.communicate(timeout=300)[1]
                source_stderr = source.communicate(timeout=300)[1]
                if source.returncode != 0 or compressor.returncode != 0:
                    diagnostic = (source_stderr + compressor_stderr).decode(
                        "utf-8", errors="replace"
                    )
                    raise CaptureError(
                        f"artifact compression failed ({command_text(source_command)} | gzip -n -9): {diagnostic.strip()}"
                    )

        gzip_pipeline(docker_save, oci_archive)
        gzip_pipeline(contract_archive, contract_pack, source_cwd=contract_context)
        retained_artifacts = {
            "oci_image_compressed": publish_content_addressed_artifact(
                oci_archive, args.output.parent
            ),
            "contract_pack_compressed": publish_content_addressed_artifact(
                contract_pack, args.output.parent
            ),
        }
        compressed_sizes = {
            "oci_image_compressed_bytes": retained_artifacts["oci_image_compressed"]["size_bytes"],
            "oci_image_compressed_sha256": retained_artifacts["oci_image_compressed"]["sha256"],
            "contract_pack_compressed_bytes": retained_artifacts["contract_pack_compressed"]["size_bytes"],
            "contract_pack_compressed_sha256": retained_artifacts["contract_pack_compressed"]["sha256"],
        }

    sizes: dict[str, Any] = {
        "native_fastid_binary_bytes": args.native_binary.stat().st_size,
        "oci_fastid_binary_bytes": int(binary_values[0]),
        "oci_fasti_cli_binary_bytes": int(binary_values[1]),
        "oci_image_bytes": int(run_checked(image_size_command)),
        "native_runtime_installed_bytes": None,
        "native_archive_compressed_bytes": None,
        **compressed_sizes,
    }
    if any(value <= 0 for value in sizes.values() if isinstance(value, int)):
        raise CaptureError(f"one or more artifact sizes are invalid: {sizes!r}")
    commands = [
        command_text(["stat", "-c", "%s", str(args.native_binary)]),
        command_text(binary_size_command),
        command_text(image_size_command),
        f"{command_text(docker_save)} | gzip -n -9",
        contract_context_provenance["archive_command"],
        f"(cd verifier-owned-git-archive && {command_text(sdk_install_command)})",
        command_text(sdk_build_command),
        f"(cd verifier-owned-git-archive && {command_text(contract_archive)} | gzip -n -9)",
    ]
    return sizes, commands, retained_artifacts


def artifact_budget_verdicts(sizes: dict[str, Any]) -> list[dict[str, Any]]:
    def measured(budget: str, value: int, reason: str) -> dict[str, Any]:
        limit = ARTIFACT_LIMITS[budget]
        return {
            "budget": budget,
            "limit_bytes": limit,
            "measured_bytes": value,
            "status": "pass" if value <= limit else "fail",
            "reason": reason,
        }

    return [
        {
            "budget": "native_runtime_installed",
            "limit_bytes": ARTIFACT_LIMITS["native_runtime_installed"],
            "measured_bytes": None,
            "status": "not_applicable",
            "reason": "B1 extracts a benchmark-only native fastid from the governed OCI image; it does not produce an installed native fastid-plus-fasti distribution.",
        },
        {
            "budget": "native_archive_compressed",
            "limit_bytes": ARTIFACT_LIMITS["native_archive_compressed"],
            "measured_bytes": None,
            "status": "not_applicable",
            "reason": "B1 does not produce a supported native distribution archive, so no compressed native package result is claimed.",
        },
        measured(
            "oci_image_compressed",
            sizes["oci_image_compressed_bytes"],
            "Deterministic gzip -n -9 size of docker image save for the immutable governed image ID.",
        ),
        measured(
            "oci_image_unpacked",
            sizes["oci_image_bytes"],
            "Docker image inspection reports the unpacked layer size for the immutable governed image ID.",
        ),
        measured(
            "contract_pack_compressed",
            sizes["contract_pack_compressed_bytes"],
            "Deterministic gzip -n -9 size of the HEAD contract, conformance, and generated SDK git archive.",
        ),
    ]


def extract_native_fastid(
    args: argparse.Namespace, context: dict[str, Any], destination: Path
) -> list[str]:
    name = f"fasti-b1-native-artifact-{os.getpid()}-{uuid.uuid4().hex[:10]}"
    create_command = [
        "docker",
        "create",
        "--name",
        name,
        "--network",
        "none",
        "--entrypoint",
        "/bin/true",
        args.immutable_image,
    ]
    copy_command = [
        "docker",
        "cp",
        f"{name}:/usr/local/bin/fastid",
        str(destination),
    ]
    try:
        container_id = run_checked(create_command)
        if not container_id:
            raise CaptureError("Docker returned no container ID for native artifact extraction")
        run_checked(copy_command)
    finally:
        subprocess.run(
            ["docker", "rm", "--force", name],
            cwd=ROOT,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    destination.chmod(0o755)
    if not destination.is_file() or not os.access(destination, os.X_OK):
        raise CaptureError("extracted immutable-image fastid is missing or not executable")
    args.native_binary = destination
    context["source"]["native_fastid_sha256"] = sha256_file(destination)
    return [command_text(create_command), command_text(copy_command)]


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


def idle_cpu_verdicts(scenarios: list[dict[str, Any]]) -> list[dict[str, Any]]:
    by_id = {scenario["id"]: scenario for scenario in scenarios}
    verdicts: list[dict[str, Any]] = []
    for scenario_id in ["native_fastid_idle", "oci_fastid_idle"]:
        measurements = [sample["idle_cpu"] for sample in by_id[scenario_id]["samples"]]
        if any(measurement is None for measurement in measurements):
            raise CaptureError(f"{scenario_id} is missing locked idle CPU measurements")
        average = max(measurement["average_percent_one_core"] for measurement in measurements)
        p95 = max(measurement["p95_percent_one_core"] for measurement in measurements)
        passed = (
            average <= IDLE_CPU_AVERAGE_LIMIT_PERCENT
            and p95 <= IDLE_CPU_P95_LIMIT_PERCENT
        )
        verdicts.append(
            {
                "scenario": scenario_id,
                "warmup_seconds": IDLE_WARMUP_SECONDS,
                "measurement_seconds": IDLE_MEASUREMENT_SECONDS,
                "average_limit_percent_one_core": IDLE_CPU_AVERAGE_LIMIT_PERCENT,
                "p95_limit_percent_one_core": IDLE_CPU_P95_LIMIT_PERCENT,
                "measured_worst_average_percent_one_core": average,
                "measured_worst_p95_percent_one_core": p95,
                "status": "pass" if passed else "fail",
                "reason": "Worst independent run after the locked ten-minute warm-up and fifteen-minute network-denied idle window.",
            }
        )
    return verdicts


def capture_bound(
    args: argparse.Namespace,
    context: dict[str, Any],
    native_artifact_commands: list[str],
    governed_build_commands: list[str],
) -> None:
    budgets_bytes = BUDGETS_PATH.read_bytes()
    budgets_document = json.loads(budgets_bytes)
    memory_budgets = budgets_document["memory_bytes"]

    args.output.parent.mkdir(parents=True, exist_ok=True)
    sizes, size_commands, retained_artifacts = artifact_sizes(args)
    scenarios = [capture_scenario(scenario_id, args) for scenario_id in SCENARIO_IDS]
    post_governor = parse_cpu_governors()
    if post_governor != context["runner"]["cpu_governor"]:
        raise CaptureError(
            f"CPU governor configuration changed during capture: before={context['runner']['cpu_governor']!r}, after={post_governor!r}"
        )
    context["runner"]["temperature"]["post_capture"] = parse_temperature()
    verify_capture_inputs_unchanged(args, context)
    evidence = {
        "$schema": "https://fasti.scrobble.dev/schemas/benchmarks/b1/evidence.schema.json",
        "schema_version": EVIDENCE_SCHEMA_VERSION,
        "body": "B1",
        "status": "complete",
        "captured_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "runner": context["runner"],
        "source": context["source"],
        "corpus": {
            "status": "not_applicable",
            "seed": None,
            "digest": None,
            "reason": "B1 measures empty-process and idle feasibility baselines; no synthetic or provider corpus is loaded.",
        },
        "budget_snapshot": {
            "source": "benchmarks/b1/budgets.json",
            "sha256": hashlib.sha256(budgets_bytes).hexdigest(),
            "memory_bytes": memory_budgets,
            "idle_cpu_percent_one_core": budgets_document["idle_cpu_percent_one_core"],
            "timing_seconds": budgets_document["timing_seconds"],
            "artifact_bytes": budgets_document["artifact_bytes"],
        },
        "harness": {
            "version": HARNESS_VERSION,
            "repetitions": args.repetitions,
            "steady_window_seconds": args.steady_window_seconds,
            "idle_warmup_seconds": args.idle_warmup_seconds,
            "idle_measurement_seconds": args.idle_measurement_seconds,
            "sample_interval_ms": args.sample_interval_ms,
            "baseline_subtraction": False,
            "capture_command": command_text([sys.executable, str(Path(__file__).resolve()), *sys.argv[1:]]),
            "governed_build_commands": governed_build_commands,
            "fingerprint_commands": context["fingerprint_commands"],
            "artifact_size_commands": size_commands,
            "native_artifact_commands": native_artifact_commands,
            "source_recheck_commands": [
                command_text(["git", "status", "--porcelain=v1", "--untracked-files=all"]),
                command_text(["git", "rev-parse", "HEAD"]),
                command_text(["git", "rev-parse", "HEAD^{tree}"]),
                command_text(["git", "rev-parse", "HEAD:contracts"]),
                f"sha256 {args.native_binary}",
                command_text(["docker", "image", "inspect", context["source"]["oci_image_ref"]]),
                command_text(["docker", "image", "inspect", context["source"]["oci_image_id"]]),
                command_text(["docker", "context", "show"]),
                f"verify local Unix socket {context['docker_locality']['socket_path']}",
            ],
        },
        "scenarios": scenarios,
        "artifact_sizes": sizes,
        "retained_artifacts": retained_artifacts,
        "budget_verdicts": budget_verdicts(scenarios, memory_budgets),
        "artifact_budget_verdicts": artifact_budget_verdicts(sizes),
        "idle_cpu_verdicts": idle_cpu_verdicts(scenarios),
    }

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

    failures = [
        verdict.get("budget", verdict.get("scenario", "unknown"))
        for collection in [
            evidence["budget_verdicts"],
            evidence["artifact_budget_verdicts"],
            evidence["idle_cpu_verdicts"],
        ]
        for verdict in collection
        if verdict["status"] == "fail"
    ]
    print(f"PASS: validated B1 performance evidence written to {args.output}")
    if failures:
        print(f"BUDGET_FAILURES: {', '.join(failures)}")
        raise SystemExit(1)


def capture(args: argparse.Namespace) -> None:
    context = preflight(args)
    build_commands = governed_build_image(args, context)
    with tempfile.TemporaryDirectory(prefix="fasti-b1-native-artifact-") as temp_name:
        native_commands = extract_native_fastid(
            args, context, Path(temp_name) / "fastid"
        )
        capture_bound(args, context, native_commands, build_commands)


def self_test() -> None:
    run_checked(["node", str(VALIDATOR_PATH), "--self-test"])
    print("PASS: B1 benchmark harness validator self-test")


def add_runner_arguments(command_parser: argparse.ArgumentParser, *, with_output: bool) -> None:
    command_parser.add_argument(
        "--image",
        required=True,
        help="local destination tag built only from benchmarks/b1/Dockerfile before capture",
    )
    command_parser.add_argument(
        "--runner-id",
        required=True,
        help="stable non-secret label for this exact physical runner",
    )
    command_parser.add_argument(
        "--custodian",
        required=True,
        help="named person accountable for the physical run",
    )
    command_parser.add_argument(
        "--os-image",
        type=Path,
        required=True,
        help="retained installation image regular file; the harness opens it with no-follow and hashes the exact bytes",
    )
    if with_output:
        command_parser.add_argument("--output", type=Path, required=True)
        command_parser.add_argument("--repetitions", type=int, default=MINIMUM_REPETITIONS)
        command_parser.add_argument("--steady-window-seconds", type=float, default=5.0)
        command_parser.add_argument(
            "--idle-warmup-seconds", type=float, default=IDLE_WARMUP_SECONDS
        )
        command_parser.add_argument(
            "--idle-measurement-seconds",
            type=float,
            default=IDLE_MEASUREMENT_SECONDS,
        )
        command_parser.add_argument(
            "--sample-interval-ms", type=int, default=SAMPLE_INTERVAL_MS
        )
        command_parser.add_argument("--startup-timeout-seconds", type=float, default=15.0)
        command_parser.add_argument("--build-timeout-seconds", type=float, default=1800.0)
    else:
        command_parser.set_defaults(
            output=Path("/non-mutating-preflight"),
            repetitions=MINIMUM_REPETITIONS,
            steady_window_seconds=5.0,
            idle_warmup_seconds=IDLE_WARMUP_SECONDS,
            idle_measurement_seconds=IDLE_MEASUREMENT_SECONDS,
            sample_interval_ms=SAMPLE_INTERVAL_MS,
            startup_timeout_seconds=15.0,
            build_timeout_seconds=1800.0,
        )


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(
        description="Capture B1 Fasti native process-tree and OCI cgroup-v2 performance evidence."
    )
    subcommands = root.add_subparsers(dest="command", required=True)
    subcommands.add_parser("self-test", help="run portable schema and negative-sentinel tests")

    preflight_parser = subcommands.add_parser(
        "preflight",
        help="emit a non-mutating JSON prerequisite result for a physical B1 runner",
    )
    add_runner_arguments(preflight_parser, with_output=False)

    capture_parser = subcommands.add_parser(
        "capture",
        help="capture complete evidence; Linux, route-less netns, Docker network-none, and cgroup v2 are mandatory",
    )
    add_runner_arguments(capture_parser, with_output=True)
    return root


def validate_arguments(args: argparse.Namespace) -> None:
    if args.command not in {"capture", "preflight"}:
        return
    args.runner_id = reject_placeholder("runner ID", args.runner_id)
    args.custodian = reject_placeholder("custodian", args.custodian)
    args.os_image = Path(os.path.abspath(args.os_image))
    if args.command == "capture":
        args.output = args.output.resolve()
    if args.repetitions < MINIMUM_REPETITIONS:
        raise CaptureError(f"at least {MINIMUM_REPETITIONS} independent repetitions are required")
    if args.steady_window_seconds < 3:
        raise CaptureError("steady measurement window must be at least three seconds")
    if args.sample_interval_ms != SAMPLE_INTERVAL_MS:
        raise CaptureError(
            f"sample interval is locked to {SAMPLE_INTERVAL_MS} milliseconds so raw evidence remains bounded and comparable"
        )
    if args.startup_timeout_seconds <= 0:
        raise CaptureError("startup timeout must be positive")
    if args.build_timeout_seconds <= 0:
        raise CaptureError("build timeout must be positive")
    if args.idle_warmup_seconds != IDLE_WARMUP_SECONDS:
        raise CaptureError(f"idle warm-up is locked to {IDLE_WARMUP_SECONDS:g} seconds")
    if args.idle_measurement_seconds != IDLE_MEASUREMENT_SECONDS:
        raise CaptureError(
            f"idle measurement is locked to {IDLE_MEASUREMENT_SECONDS:g} seconds"
        )
    if not args.image.strip():
        raise CaptureError("image reference must not be empty")


def preflight_requirements() -> list[dict[str, str]]:
    return [
        {
            "id": "physical_profile",
            "requirement": "Physical Raspberry Pi 5 champion or physical four-core J4125 runner; emulation is refused.",
            "action": "Run preflight directly on the named physical target.",
        },
        {
            "id": "clean_exact_source",
            "requirement": "Clean Git tree containing the exact commit, contracts tree, lockfiles, and governed benchmark Dockerfile.",
            "action": "Commit intended benchmark changes and remove unrelated generated files before capture.",
        },
        {
            "id": "linux_measurement_primitives",
            "requirement": "Linux /proc, cgroup v2, unprivileged route-less network namespaces, local Unix-socket Docker, and fingerprint commands.",
            "action": "Enable cgroup v2 and unprivileged user/network namespaces and use a local Docker Engine.",
        },
        {
            "id": "retained_os_image",
            "requirement": "Named custodian retains the exact installed OS image as a local regular file; the harness hashes it through a no-follow descriptor.",
            "action": "Pass the retained image with --os-image. Raspberry Pi qualification remains blocked until an official image digest is pinned in physical-profiles.json.",
        },
        {
            "id": "locked_duration",
            "requirement": "At least five independent runs, each idle run using a 600-second warm-up and 900-second network-denied measurement.",
            "action": "Reserve enough uninterrupted time; the locked windows cannot be shortened.",
        },
    ]


def preflight_json(args: argparse.Namespace) -> None:
    context = preflight(args)
    result = {
        "schema_version": "fasti.b1.performance-preflight.v1",
        "status": "pass",
        "non_mutating": True,
        "runner": context["runner"],
        "source": {
            "git_commit": context["source"]["git_commit"],
            "git_tree": context["source"]["git_tree"],
            "contract_ref": context["source"]["contract_ref"],
            "build_recipe_path": context["source"]["build_recipe_path"],
            "build_recipe_sha256": context["source"]["build_recipe_sha256"],
        },
        "capture_prerequisites": {
            "minimum_independent_repetitions": MINIMUM_REPETITIONS,
            "idle_warmup_seconds": IDLE_WARMUP_SECONDS,
            "idle_measurement_seconds": IDLE_MEASUREMENT_SECONDS,
            "network_policy": "route-less native namespace and Docker network none",
            "governed_image_build": str(GOVERNED_DOCKERFILE.relative_to(ROOT)),
        },
        "requirements": preflight_requirements(),
        "next_action": "Run the capture command with the same runner, custodian, image tag, and retained OS image file; allow at least the locked idle windows for every independent run.",
    }
    print(json.dumps(result, indent=2))


def main() -> None:
    args = parser().parse_args()
    try:
        validate_arguments(args)
        if args.command == "self-test":
            self_test()
        elif args.command == "preflight":
            preflight_json(args)
        else:
            capture(args)
    except CaptureError as error:
        if getattr(args, "command", None) == "preflight":
            print(
                json.dumps(
                    {
                        "schema_version": "fasti.b1.performance-preflight.v1",
                        "status": "refused",
                        "non_mutating": True,
                        "error": str(error),
                        "requirements": preflight_requirements(),
                        "next_action": "Resolve the reported prerequisite on the physical runner and rerun preflight. Do not substitute emulation or edit the resulting evidence by hand.",
                    },
                    indent=2,
                )
            )
        else:
            print(f"REFUSED: {error}", file=sys.stderr)
        raise SystemExit(2) from error


if __name__ == "__main__":
    main()
