#!/usr/bin/env python3
"""Preserve pinned native source notices; never build or execute native code."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import tarfile


ROOT = Path(__file__).resolve().parent
SELECTION_SHA256 = "dd6774843d132092f39c645706231b86a3d29c75558f40c956ca9f2424274c4f"
ORIGINAL_SHA256 = "b6876ac6f00ecc2900499218104e29ecc0d582f5eeea8719f418867cd7765009"
PATHS_SHA256 = "4e54f326616546771cfe0b98afd7e81d222d1947f917682e3d9bfebb491125ee"
ARCHIVE_SHA256 = "b20a92e7ec25b285eafa349d721a5bb27e3a8ba94c0816630a127883f1d1b3ab"
ARCHIVE_BYTES = 2_082_843
MAX_BUNDLE_BYTES = 4 * 1024 * 1024
HISTORICAL_ROOT = "/tmp/fasti-c3-license-evidence-AwiTuW/native-source/libsodium-stable/"
SOURCE_PREFIX = "source/libsodium-stable/"
SCOPE = b"""Qualification-only source-notice custody, not production adoption.
Complete selected source bytes preserve embedded notices, not referenced legal
texts absent from this archive. No licence alternative or legal clearance is
established. This is not complete distribution attribution, a linker map,
execution proof, a new native build or measurement, or C3 profile approval.
ARM and other source-only context does not establish measured x86 execution.
Existing binary/source/build receipts remain separate. Unbound retained
binaries remain unqualified. No compiled binary is included in this bundle.
"""


class CustodyError(ValueError):
    """The requested artifact cannot be proven exact."""


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_regular(path: Path, limit: int) -> bytes:
    """Bound one opened regular file; refuse links, FIFOs and changing input."""
    if not hasattr(os, "O_NOFOLLOW"):
        raise CustodyError("this local custody tool requires O_NOFOLLOW support")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as handle:
        before = os.fstat(handle.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_size > limit:
            raise CustodyError("input must be a bounded regular file")
        data = handle.read(limit + 1)
        after = os.fstat(handle.fileno())
        if (len(data) != before.st_size or len(data) > limit
                or (before.st_size, before.st_mtime_ns, before.st_ctime_ns)
                != (after.st_size, after.st_mtime_ns, after.st_ctime_ns)):
            raise CustodyError("input changed or exceeded its bound")
    return data


def strict_json(data: bytes):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise CustodyError("duplicate JSON key")
            result[key] = value
        return result

    def reject_constant(_value):
        raise CustodyError("non-JSON numeric constant")

    return json.loads(data, object_pairs_hook=unique, parse_constant=reject_constant)


def safe_path(name: str) -> bool:
    """Only canonical archive-relative POSIX paths; never extraction targets."""
    return (isinstance(name, str) and bool(name) and "\\" not in name
            and "\x00" not in name and not name.startswith("/")
            and str(PurePosixPath(name)) == name
            and all(part not in (".", "..") for part in name.split("/")))


def load_selection():
    selection = read_regular(ROOT / "selection.json", 64 * 1024)
    original = read_regular(ROOT / "original-inventory.json", 32 * 1024)
    if sha256(selection) != SELECTION_SHA256 or sha256(original) != ORIGINAL_SHA256:
        raise CustodyError("checked-in selection or original inventory changed")
    document, legacy = strict_json(selection), strict_json(original)
    entries = document["files"]
    names = [entry["path"] for entry in entries]
    if (len(entries) != 135 or names != sorted(set(names))
            or not all(safe_path(name) for name in names)
            or sum(entry["bytes"] for entry in entries) != 2_844_540
            or sha256(("\n".join(names) + "\n").encode()) != PATHS_SHA256
            or document["archive_sha256"] != ARCHIVE_SHA256
            or document["archive_bytes"] != ARCHIVE_BYTES
            or document["original_inventory_sha256"] != ORIGINAL_SHA256):
        raise CustodyError("frozen selection invariant failed")
    by_path = {entry["path"]: entry for entry in entries}
    if len(legacy) != 45:
        raise CustodyError("original inventory must retain all 45 entries")
    for entry in legacy:
        if not entry["path"].startswith(HISTORICAL_ROOT):
            raise CustodyError("unexpected historical inventory path")
        relative = entry["path"][len(HISTORICAL_ROOT):]
        if by_path.get(relative) != {**entry, "path": relative}:
            raise CustodyError("original inventory entry is not preserved")
    metadata = {
        "metadata/selection.json": selection,
        "metadata/original-inventory.json": original,
        "metadata/SCOPE.txt": SCOPE,
    }
    return entries, metadata


def source_members(data: bytes, entries: list[dict]) -> dict[str, bytes]:
    """Read selected regular members. Caller pins the entire compressed input."""
    wanted = {entry["path"]: entry for entry in entries}
    if len(wanted) != len(entries):
        raise CustodyError("duplicate selected source path")
    result = {}
    seen = set()
    with tarfile.open(fileobj=io.BytesIO(data), mode="r|gz") as archive:
        for member in archive:
            name = member.name.rstrip("/") if member.isdir() else member.name
            if not safe_path(name) or name in seen:
                raise CustodyError("unsafe or duplicate source member")
            seen.add(name)
            if not name.startswith("libsodium-stable/"):
                if name == "libsodium-stable" and member.isdir():
                    continue
                raise CustodyError("unexpected source root")
            relative = name[len("libsodium-stable/"):]
            if relative not in wanted:
                continue
            entry = wanted[relative]
            if not member.isreg() or member.size != entry["bytes"]:
                raise CustodyError("selected source member is not exact regular content")
            handle = archive.extractfile(member)
            if handle is None:
                raise CustodyError("selected source member cannot be read")
            with handle:
                payload = handle.read(entry["bytes"] + 1)
            if len(payload) != entry["bytes"] or sha256(payload) != entry["sha256"]:
                raise CustodyError("selected source member content mismatch")
            result[SOURCE_PREFIX + relative] = payload
    if len(result) != len(wanted):
        raise CustodyError("selected source member missing")
    return result


def deterministic_tar(payloads: dict[str, bytes]) -> bytes:
    """Canonical USTAR headers, path ordering, zero padding and EOF records."""
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for name, payload in sorted(payloads.items()):
            if not safe_path(name):
                raise CustodyError("unsafe bundle path")
            member = tarfile.TarInfo(name)
            member.size = len(payload)
            member.mode = 0o644
            # TarInfo defaults: uid/gid/mtime zero, empty uname/gname, regular file.
            archive.addfile(member, io.BytesIO(payload))
    data = output.getvalue()
    if len(data) > MAX_BUNDLE_BYTES:
        raise CustodyError("bundle exceeded its fixed bound")
    return data


def expected_members(entries: list[dict], metadata: dict[str, bytes]):
    expected = {SOURCE_PREFIX + entry["path"]: (entry["bytes"], entry["sha256"])
                for entry in entries}
    expected.update({name: (len(data), sha256(data)) for name, data in metadata.items()})
    return expected


def verify_bytes(data: bytes, expected: dict[str, tuple[int, str]]) -> None:
    """Compare content against trusted package data, not the embedded manifest."""
    if len(data) > MAX_BUNDLE_BYTES:
        raise CustodyError("oversized bundle")
    payloads = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:") as archive:
        for member in archive:
            if (not safe_path(member.name) or member.name in payloads
                    or member.name not in expected or not member.isreg()):
                raise CustodyError("unexpected, duplicate, unsafe or nonregular bundle member")
            size, digest = expected[member.name]
            if member.size != size:
                raise CustodyError("bundle member length mismatch")
            handle = archive.extractfile(member)
            if handle is None:
                raise CustodyError("bundle member cannot be read")
            with handle:
                payload = handle.read(size + 1)
            if len(payload) != size or sha256(payload) != digest:
                raise CustodyError("bundle member content mismatch")
            payloads[member.name] = payload
    if payloads.keys() != expected.keys():
        raise CustodyError("bundle member missing")
    # Reject accepted-but-noncanonical tar metadata, PAX headers, padding,
    # truncated EOF, concatenated archives and trailing material in one check.
    if data != deterministic_tar(payloads):
        raise CustodyError("noncanonical tar framing or trailing material")


def verify(path: Path) -> dict:
    entries, metadata = load_selection()
    data = read_regular(path, MAX_BUNDLE_BYTES)
    verify_bytes(data, expected_members(entries, metadata))
    return {"status": "verified", "scope": "selected-source-notice-custody-only",
            "source_files": len(entries), "bytes": len(data), "sha256": sha256(data)}


def build(archive_path: Path, output: Path) -> dict:
    entries, metadata = load_selection()
    source = read_regular(archive_path, ARCHIVE_BYTES)
    if len(source) != ARCHIVE_BYTES or sha256(source) != ARCHIVE_SHA256:
        raise CustodyError("native archive is not the pinned source")
    payloads = source_members(source, entries)
    payloads.update(metadata)
    data = deterministic_tar(payloads)
    verify_bytes(data, expected_members(entries, metadata))
    # Exclusive creation preserves existing outputs, including failed evidence.
    # On interruption a partial file remains, but no verified result is emitted.
    with output.open("xb") as handle:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    return verify(output)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("build")
    create.add_argument("--archive", required=True, type=Path)
    create.add_argument("--output", required=True, type=Path)
    check = commands.add_parser("verify")
    check.add_argument("artifact", type=Path)
    args = parser.parse_args()
    try:
        result = (build(args.archive, args.output) if args.command == "build"
                  else verify(args.artifact))
    except (CustodyError, OSError, tarfile.TarError, EOFError, UnicodeError,
            json.JSONDecodeError, KeyError, TypeError) as error:
        print(f"FAIL: {error}. Any existing output is retained, not certified.", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
