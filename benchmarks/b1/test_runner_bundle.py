#!/usr/bin/env python3
"""Portable tests for the exact-commit private runner handoff."""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import stat
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "package-b1-runner.py"
SPEC = importlib.util.spec_from_file_location("fasti_package_b1_runner", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
bundle = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bundle)


def fixture() -> dict:
    return {
        "$schema": bundle.SCHEMA_URL,
        "schema_version": bundle.VERSION,
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


class ManifestTests(unittest.TestCase):
    def test_fixture_is_valid(self) -> None:
        self.assertEqual(bundle.validate_manifest(fixture())["schema_version"], bundle.VERSION)

    def test_unknown_fields_fail_closed(self) -> None:
        value = fixture()
        value["invented"] = True
        with self.assertRaises(bundle.BundleError):
            bundle.validate_manifest(value)

    def test_duplicate_manifest_keys_fail_before_schema_validation(self) -> None:
        source = json.dumps(fixture(), indent=2).replace(
            '    "git_tree": "' + "2" * 40 + '",',
            '    "git_tree": "' + "9" * 40 + '",\n'
            '    "git_tree": "' + "2" * 40 + '",',
        )
        with self.assertRaisesRegex(bundle.BundleError, "duplicate key 'git_tree'"):
            bundle.validate_manifest_source(source)


class HandoffIntegrationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="fasti-b1-bundle-test-")
        self.root = Path(self.temporary.name)
        self.repository = self.root / "source"
        self.repository.mkdir()
        self.git("init", "--initial-branch=main")
        self.git("config", "user.name", "Fasti Test")
        self.git("config", "user.email", "test@fasti.invalid")
        (self.repository / "contracts").mkdir()
        (self.repository / "contracts" / "contract.txt").write_text(
            "contract-v1\n", encoding="utf-8"
        )
        (self.repository / "README.md").write_text("fixture\n", encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-m", "fixture")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git(self, *args: str) -> str:
        result = subprocess.run(
            ["git", *args],
            cwd=self.repository,
            text=True,
            capture_output=True,
            check=True,
        )
        return result.stdout.strip()

    def canonical_handoff(self) -> tuple[Path, Path]:
        artifact = self.root / "handoff.bundle"
        bundle.create_bundle(artifact, repository=self.repository)
        return artifact, artifact.with_suffix(".manifest.json")

    def write_manifest(self, path: Path, artifact: Path) -> None:
        source = bundle.source_identity(self.repository)
        value = fixture()
        value["source"] = source
        value["bundle"] = {
            "filename": artifact.name,
            "sha256": bundle.sha256_file(artifact),
            "size_bytes": artifact.stat().st_size,
            "head_ref": "HEAD",
        }
        path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")

    def test_create_verify_unpack_round_trip(self) -> None:
        artifact, manifest = self.canonical_handoff()
        if os.name == "posix":
            self.assertEqual(stat.S_IMODE(artifact.stat().st_mode), 0o600)
            self.assertEqual(stat.S_IMODE(manifest.stat().st_mode), 0o600)
        bundle.verify_bundle(artifact, manifest)
        destination = self.root / "unpacked"
        bundle.unpack_bundle(artifact, manifest, destination)
        self.assertEqual(
            bundle.source_identity(destination), bundle.source_identity(self.repository)
        )

    def test_unpack_uses_owned_snapshot_after_verification(self) -> None:
        artifact, manifest = self.canonical_handoff()
        original_verify = bundle.verify_bundle

        def mutate_caller_copy(snapshot_bundle: Path, snapshot_manifest: Path) -> dict:
            artifact.write_bytes(b"swapped after unpack snapshot")
            return original_verify(snapshot_bundle, snapshot_manifest)

        destination = self.root / "snapshot-unpacked"
        with mock.patch.object(bundle, "verify_bundle", side_effect=mutate_caller_copy):
            bundle.unpack_bundle(artifact, manifest, destination)
        self.assertEqual(
            bundle.source_identity(destination), bundle.source_identity(self.repository)
        )

    def test_false_tree_and_contract_objects_are_refused(self) -> None:
        artifact, manifest = self.canonical_handoff()
        for field in ["git_tree", "contract_ref"]:
            with self.subTest(field=field):
                value = json.loads(manifest.read_text(encoding="utf-8"))
                value["source"][field] = "9" * 40
                mutated = self.root / f"false-{field}.json"
                mutated.write_text(json.dumps(value), encoding="utf-8")
                expected = "tree" if field == "git_tree" else "contract"
                with self.assertRaisesRegex(bundle.BundleError, expected):
                    bundle.verify_bundle(artifact, mutated)

    def test_extra_refs_are_refused(self) -> None:
        self.git("branch", "extra")
        artifact = self.root / "extra.bundle"
        self.git("bundle", "create", str(artifact), "HEAD", "refs/heads/extra")
        manifest = self.root / "extra.manifest.json"
        self.write_manifest(manifest, artifact)
        with self.assertRaisesRegex(bundle.BundleError, "extra refs"):
            bundle.verify_bundle(artifact, manifest)

    def test_prerequisite_dependent_bundle_is_refused(self) -> None:
        base = self.git("rev-parse", "HEAD")
        (self.repository / "README.md").write_text("fixture-v2\n", encoding="utf-8")
        self.git("add", "README.md")
        self.git("commit", "-m", "fixture v2")
        artifact = self.root / "prerequisite.bundle"
        self.git("bundle", "create", str(artifact), f"{base}..HEAD")
        manifest = self.root / "prerequisite.manifest.json"
        self.write_manifest(manifest, artifact)
        with self.assertRaisesRegex(bundle.BundleError, "command failed"):
            bundle.verify_bundle(artifact, manifest)

    def test_symlink_input_is_refused(self) -> None:
        artifact, manifest = self.canonical_handoff()
        link = self.root / "linked.bundle"
        try:
            os.symlink(artifact, link)
        except (OSError, NotImplementedError):
            self.skipTest("symbolic links unavailable")
        value = json.loads(manifest.read_text(encoding="utf-8"))
        value["bundle"]["filename"] = link.name
        linked_manifest = self.root / "linked.manifest.json"
        linked_manifest.write_text(json.dumps(value), encoding="utf-8")
        with self.assertRaisesRegex(bundle.BundleError, "unsafe input"):
            bundle.verify_bundle(link, linked_manifest)

    def test_unpack_refuses_symlinked_bundle_and_manifest_caller_paths(self) -> None:
        artifact, manifest = self.canonical_handoff()
        links = self.root / "links"
        links.mkdir()
        linked_artifact = links / artifact.name
        linked_manifest = links / manifest.name
        try:
            os.symlink(artifact, linked_artifact)
            os.symlink(manifest, linked_manifest)
        except (OSError, NotImplementedError):
            self.skipTest("symbolic links unavailable")
        with self.assertRaisesRegex(bundle.BundleError, "unsafe input"):
            bundle.unpack_bundle(
                linked_artifact,
                linked_manifest,
                self.root / "symlink-unpacked",
            )

    def test_public_remote_requirement_is_refused(self) -> None:
        value = json.loads(json.dumps(fixture()))
        value["handoff"]["public_remote_required"] = True
        with self.assertRaises(bundle.BundleError):
            bundle.validate_manifest(value)

    def test_traversal_filename_is_refused(self) -> None:
        value = fixture()
        value["bundle"]["filename"] = "../fasti-b1.bundle"
        with self.assertRaises(bundle.BundleError):
            bundle.validate_manifest(value)

    def test_in_repository_output_is_refused_even_when_ignored(self) -> None:
        ignored_output = self.repository / "target" / "handoff.bundle"
        with self.assertRaisesRegex(bundle.BundleError, "outside"):
            bundle.create_bundle(ignored_output, repository=self.repository)

    def test_snapshot_refuses_source_changed_during_copy(self) -> None:
        source = self.root / "changing.bundle"
        destination = self.root / "snapshot.bundle"
        source.write_bytes(b"stable source")
        original_copy = bundle.shutil.copyfileobj

        def copy_then_mutate(
            source_handle: object, destination_handle: object, length: int
        ) -> None:
            original_copy(source_handle, destination_handle, length=length)
            source.write_bytes(b"changed after copy and before the final file check")

        with mock.patch.object(
            bundle.shutil, "copyfileobj", side_effect=copy_then_mutate
        ):
            with self.assertRaisesRegex(
                bundle.BundleError, "changed while it was copied"
            ):
                bundle.snapshot_regular_file(source, destination, "bundle")
        self.assertFalse(destination.exists())

    def test_snapshot_fails_closed_without_no_follow_support(self) -> None:
        source = self.root / "regular.bundle"
        destination = self.root / "snapshot.bundle"
        source.write_bytes(b"fixture")
        with mock.patch.object(bundle.os, "O_NOFOLLOW", new=None, create=False):
            with self.assertRaisesRegex(bundle.BundleError, "O_NOFOLLOW"):
                bundle.snapshot_regular_file(source, destination, "bundle")

    def test_empty_bundle_basename_is_refused(self) -> None:
        value = fixture()
        value["bundle"]["filename"] = ".bundle"
        with self.assertRaises(bundle.BundleError):
            bundle.validate_manifest(value)

    def test_invalid_utc_timestamp_is_refused(self) -> None:
        value = fixture()
        value["created_at"] = "not-a-dateZ"
        with self.assertRaises(bundle.BundleError):
            bundle.validate_manifest(value)


if __name__ == "__main__":
    unittest.main()
