#!/usr/bin/env python3
"""Create, verify, and unpack an exact private B1 runner Git bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_URL = "https://fasti.scrobble.dev/schemas/benchmarks/b1/runner-bundle.schema.json"
VERSION = "fasti.b1.runner-bundle.v1"
MANIFEST_VALIDATOR = (
    ROOT / "benchmarks" / "b1" / "validate-runner-bundle.mjs"
)
BUNDLE_BASENAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*\.bundle$")


class BundleError(RuntimeError):
    """The private runner handoff could not be proven exact."""


def run_checked(parts: list[str | Path], *, cwd: Path = ROOT) -> str:
    result = subprocess.run(
        [str(part) for part in parts],
        cwd=cwd,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        diagnostic = result.stderr.strip() or result.stdout.strip() or "no diagnostic"
        raise BundleError(f"command failed ({' '.join(map(str, parts))}): {diagnostic}")
    return result.stdout.strip()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def snapshot_regular_file(source_path: Path, destination_path: Path, label: str) -> None:
    no_follow = getattr(os, "O_NOFOLLOW", None)
    if no_follow is None:
        raise BundleError(
            f"{label} snapshot requires O_NOFOLLOW support on this platform"
        )
    source_flags = os.O_RDONLY | no_follow
    try:
        source_descriptor = os.open(source_path, source_flags)
    except OSError as error:
        raise BundleError(f"{label} snapshot refused an unsafe input path") from error
    try:
        if not stat.S_ISREG(os.fstat(source_descriptor).st_mode):
            raise BundleError(f"{label} input must be a regular file")
        with os.fdopen(source_descriptor, "rb", closefd=False) as source, destination_path.open(
            "xb"
        ) as destination:
            shutil.copyfileobj(source, destination, length=1024 * 1024)
    finally:
        os.close(source_descriptor)


def source_identity(cwd: Path = ROOT) -> dict[str, str]:
    if run_checked(["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=cwd):
        raise BundleError("runner handoff requires a clean source tree")
    return {
        "git_commit": run_checked(["git", "rev-parse", "HEAD"], cwd=cwd),
        "git_tree": run_checked(["git", "rev-parse", "HEAD^{tree}"], cwd=cwd),
        "contract_ref": run_checked(["git", "rev-parse", "HEAD:contracts"], cwd=cwd),
        "tree_state": "clean",
    }


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise BundleError(f"runner bundle manifest contains duplicate key {key!r}")
        value[key] = item
    return value


def validate_manifest_source(source: str) -> dict[str, Any]:
    try:
        manifest = json.loads(source, object_pairs_hook=_reject_duplicate_keys)
    except BundleError:
        raise
    except (json.JSONDecodeError, UnicodeError) as error:
        raise BundleError("runner bundle manifest is not strict JSON") from error
    result = subprocess.run(
        ["node", str(MANIFEST_VALIDATOR), "--stdin"],
        cwd=ROOT,
        input=source,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        diagnostic = result.stderr.strip() or result.stdout.strip() or "no diagnostic"
        raise BundleError(
            f"bundle manifest failed canonical JSON Schema validation: {diagnostic}"
        )
    return manifest


def validate_manifest(manifest: Any) -> dict[str, Any]:
    return validate_manifest_source(json.dumps(manifest))


def verify_bundle(bundle_path: Path, manifest_path: Path) -> dict[str, Any]:
    if not bundle_path.is_file() or not manifest_path.is_file():
        raise BundleError("bundle and manifest files are both required")
    with tempfile.TemporaryDirectory(prefix="fasti-b1-bundle-snapshot-") as name:
        snapshot_root = Path(name)
        snapshot_bundle = snapshot_root / bundle_path.name
        snapshot_manifest = snapshot_root / manifest_path.name
        snapshot_regular_file(bundle_path, snapshot_bundle, "bundle")
        snapshot_regular_file(manifest_path, snapshot_manifest, "manifest")
        manifest = validate_manifest_source(snapshot_manifest.read_text(encoding="utf-8"))
        artifact = manifest["bundle"]
        if artifact["filename"] != bundle_path.name:
            raise BundleError("manifest filename does not match the supplied bundle")
        if artifact["size_bytes"] != snapshot_bundle.stat().st_size:
            raise BundleError("bundle byte size does not match its manifest")
        if artifact["sha256"] != sha256_file(snapshot_bundle):
            raise BundleError("bundle digest does not match its manifest")
        heads = run_checked(
            ["git", "bundle", "list-heads", snapshot_bundle]
        ).splitlines()
        expected = f"{manifest['source']['git_commit']} HEAD"
        if heads != [expected]:
            raise BundleError("bundle must contain exactly the manifest's HEAD and no extra refs")
        bare_repository = snapshot_root / "repository.git"
        run_checked(["git", "init", "--bare", bare_repository])
        # Verification in an empty repository proves the handoff has no hidden
        # prerequisite on the maintainer's local object database.
        run_checked(["git", "bundle", "verify", snapshot_bundle], cwd=bare_repository)
        run_checked(
            ["git", "fetch", "--no-tags", snapshot_bundle, "HEAD"],
            cwd=bare_repository,
        )
        observed_tree = run_checked(
            ["git", "rev-parse", "FETCH_HEAD^{tree}"], cwd=bare_repository
        )
        observed_contracts = run_checked(
            ["git", "rev-parse", "FETCH_HEAD:contracts"], cwd=bare_repository
        )
        if observed_tree != manifest["source"]["git_tree"]:
            raise BundleError("bundle commit tree does not match its manifest")
        if observed_contracts != manifest["source"]["contract_ref"]:
            raise BundleError("bundle contract object does not match its manifest")
    print(
        "PASS: verified private runner bundle "
        f"{bundle_path.name} at {manifest['source']['git_commit']}"
    )
    return manifest


def create_bundle(output: Path, repository: Path = ROOT) -> None:
    if not BUNDLE_BASENAME.fullmatch(output.name):
        raise BundleError("output must be a safe basename ending in .bundle")
    repository = repository.resolve()
    output = output.resolve()
    if output == repository or repository in output.parents:
        raise BundleError("runner handoff output must be outside the source repository")
    manifest_path = output.with_suffix(".manifest.json")
    if output.exists() or manifest_path.exists():
        raise BundleError("refusing to overwrite an existing bundle or manifest")
    source = source_identity(repository)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="fasti-b1-bundle-", dir=output.parent) as name:
        temporary_bundle = Path(name) / output.name
        run_checked(
            ["git", "bundle", "create", temporary_bundle, "HEAD"], cwd=repository
        )
        temporary_bundle.chmod(0o600)
        manifest = {
            "$schema": SCHEMA_URL,
            "schema_version": VERSION,
            "created_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
            "source": source,
            "bundle": {
                "filename": output.name,
                "sha256": sha256_file(temporary_bundle),
                "size_bytes": temporary_bundle.stat().st_size,
                "head_ref": "HEAD",
            },
            "handoff": {
                "checkout_mode": "detached_exact_commit",
                "public_remote_required": False,
                "bundle_scope": "self_contained_objects_reachable_from_exact_head_only",
            },
        }
        validate_manifest(manifest)
        temporary_manifest = Path(name) / manifest_path.name
        temporary_manifest.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        temporary_manifest.chmod(0o600)
        verify_bundle(temporary_bundle, temporary_manifest)
        if source_identity(repository) != source:
            raise BundleError("source identity changed while creating the bundle")
        temporary_bundle.replace(output)
        temporary_manifest.replace(manifest_path)
    print(f"PASS: wrote private runner handoff {output} and {manifest_path}")


def unpack_bundle(bundle_path: Path, manifest_path: Path, destination: Path) -> None:
    destination = Path(os.path.abspath(destination))
    if destination.exists() or destination.is_symlink():
        raise BundleError(f"refusing to replace existing destination: {destination}")
    with tempfile.TemporaryDirectory(prefix="fasti-b1-unpack-snapshot-") as name:
        snapshot_root = Path(name)
        snapshot_bundle = snapshot_root / bundle_path.name
        snapshot_manifest = snapshot_root / manifest_path.name
        snapshot_regular_file(bundle_path, snapshot_bundle, "bundle")
        snapshot_regular_file(manifest_path, snapshot_manifest, "manifest")
        manifest = verify_bundle(snapshot_bundle, snapshot_manifest)
        run_checked(["git", "clone", "--no-checkout", snapshot_bundle, destination])
        run_checked(
            ["git", "checkout", "--detach", manifest["source"]["git_commit"]],
            cwd=destination,
        )
        observed = source_identity(destination)
        if observed != manifest["source"]:
            raise BundleError("detached runner checkout does not match the bundle manifest")
    print(f"PASS: unpacked exact detached runner checkout at {destination}")


def self_test() -> None:
    fixture = {
        "$schema": SCHEMA_URL,
        "schema_version": VERSION,
        "created_at": "2026-08-22T00:00:00Z",
        "source": {
            "git_commit": "1" * 40,
            "git_tree": "2" * 40,
            "contract_ref": "3" * 40,
            "tree_state": "clean",
        },
        "bundle": {
            "filename": "fasti-b1.bundle",
            "sha256": "4" * 64,
            "size_bytes": 1,
            "head_ref": "HEAD",
        },
        "handoff": {
            "checkout_mode": "detached_exact_commit",
            "public_remote_required": False,
            "bundle_scope": "self_contained_objects_reachable_from_exact_head_only",
        },
    }
    validate_manifest(fixture)
    mutation = json.loads(json.dumps(fixture))
    mutation["handoff"]["public_remote_required"] = True
    try:
        validate_manifest(mutation)
    except BundleError:
        pass
    else:
        raise AssertionError("public-remote mutation passed bundle validation")
    invalid_time = json.loads(json.dumps(fixture))
    invalid_time["created_at"] = "not-a-dateZ"
    try:
        validate_manifest(invalid_time)
    except BundleError:
        pass
    else:
        raise AssertionError("invalid timestamp passed bundle validation")
    invalid_name = json.loads(json.dumps(fixture))
    invalid_name["bundle"]["filename"] = ".bundle"
    try:
        validate_manifest(invalid_name)
    except BundleError:
        print("PASS: private runner bundle manifest self-test")
        return
    raise AssertionError("empty bundle basename passed bundle validation")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    create = subparsers.add_parser("create")
    create.add_argument("--output", required=True, type=Path)
    verify = subparsers.add_parser("verify")
    verify.add_argument("--bundle", required=True, type=Path)
    verify.add_argument("--manifest", required=True, type=Path)
    unpack = subparsers.add_parser("unpack")
    unpack.add_argument("--bundle", required=True, type=Path)
    unpack.add_argument("--manifest", required=True, type=Path)
    unpack.add_argument("--destination", required=True, type=Path)
    subparsers.add_parser("self-test")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "create":
            create_bundle(args.output)
        elif args.command == "verify":
            verify_bundle(args.bundle.resolve(), args.manifest.resolve())
        elif args.command == "unpack":
            unpack_bundle(args.bundle, args.manifest, args.destination)
        else:
            self_test()
        return 0
    except (BundleError, OSError, json.JSONDecodeError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
