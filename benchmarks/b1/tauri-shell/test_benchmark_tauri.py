#!/usr/bin/env python3
"""Portable tests for the benchmark-only Tauri process-tree harness."""

from __future__ import annotations

import importlib.util
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts" / "benchmark-tauri-b1.py"
SPEC = importlib.util.spec_from_file_location("fasti_benchmark_tauri_b1", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
benchmark = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(benchmark)


class CgroupBoundaryTests(unittest.TestCase):
    def test_expected_transient_scope_is_accepted(self) -> None:
        unit = "fasti-b1-tauri-" + "1" * 32 + ".scope"
        path = benchmark.validate_control_group(
            f"/user.slice/user-1000.slice/user@1000.service/app.slice/{unit}", unit
        )
        self.assertEqual(path.name, unit)

    def test_different_or_traversing_scope_is_refused(self) -> None:
        unit = "fasti-b1-tauri-" + "1" * 32 + ".scope"
        for value in ["../../escape", "/user.slice/unrelated.scope"]:
            with self.subTest(value=value):
                with self.assertRaises(benchmark.CaptureError):
                    benchmark.validate_control_group(value, unit)


class DerivationTests(unittest.TestCase):
    def test_macos_is_refused_before_linux_capture_requirements(self) -> None:
        with mock.patch.object(benchmark.platform, "system", return_value="Darwin"):
            with self.assertRaisesRegex(benchmark.CaptureError, "WebKit XPC"):
                benchmark.display_session()

    def test_simulated_display_process_is_refused(self) -> None:
        with self.assertRaisesRegex(benchmark.CaptureError, "simulated"):
            benchmark.validate_display_evidence(
                session_id="2",
                session_type="x11",
                session_remote="no",
                session_class="user",
                session_state="active",
                seat="seat0",
                connected_drm_connectors=["card0-HDMI-A-1"],
                process_inventory="Xvfb :99 -screen 0 1920x1080x24",
                wayland=False,
                x11=True,
            )

    def test_local_physical_display_evidence_is_accepted(self) -> None:
        evidence = benchmark.validate_display_evidence(
            session_id="2",
            session_type="wayland",
            session_remote="no",
            session_class="user",
            session_state="active",
            seat="seat0",
            connected_drm_connectors=["card0-HDMI-A-1"],
            process_inventory="gnome-shell /usr/bin/gnome-shell",
            wayland=True,
            x11=True,
        )
        self.assertEqual(evidence["display_server"], "wayland_and_x11")

    def test_summary_is_derived(self) -> None:
        self.assertEqual(
            benchmark.metric_summary([5, 1, 3, 2, 4]),
            {"minimum": 1, "median": 3, "maximum": 5},
        )

    def test_runner_identity_rejects_placeholders(self) -> None:
        for value in ["runner", "runner-id", "self-test", "TBD"]:
            with self.subTest(value=value):
                with self.assertRaises(benchmark.CaptureError):
                    benchmark.validate_runner_id(value)

    def test_runner_identity_accepts_stable_lab_label(self) -> None:
        self.assertEqual(
            benchmark.validate_runner_id("linux-desktop-lab-01"),
            "linux-desktop-lab-01",
        )

    def test_visible_fixture_is_refused(self) -> None:
        config = __import__("json").loads(benchmark.TAURI_CONFIG.read_text(encoding="utf-8"))
        config["app"]["windows"][0]["visible"] = True
        with self.assertRaisesRegex(benchmark.CaptureError, "hidden"):
            benchmark.validate_fixture_policy(
                config,
                __import__("tomllib").loads(
                    benchmark.MANIFEST.read_text(encoding="utf-8")
                ),
                benchmark.INDEX_HTML.read_text(encoding="utf-8"),
            )

    def test_receipt_output_is_confined_to_private_evidence_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(benchmark.CaptureError, "directly under"):
                benchmark.require_private_evidence_path(
                    Path(directory) / "tauri-receipt.json"
                )

    @unittest.skipUnless(hasattr(os, "O_NOFOLLOW"), "requires O_NOFOLLOW")
    def test_retained_artifact_is_content_addressed_and_private(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            artifact_root = Path(directory) / "artifacts"
            artifact_root.mkdir()
            with mock.patch.object(benchmark, "ARTIFACT_ROOT", artifact_root):
                artifact, created = benchmark.retain_artifact(b"exact bytes")
                self.assertTrue(created)
                self.assertEqual(benchmark.read_regular_file_once(artifact), b"exact bytes")
                self.assertEqual(artifact.stat().st_mode & 0o777, 0o600)
                same, created_again = benchmark.retain_artifact(b"exact bytes")
                self.assertEqual(same, artifact)
                self.assertFalse(created_again)

    @unittest.skipUnless(hasattr(os, "O_NOFOLLOW"), "requires O_NOFOLLOW")
    def test_os_image_digest_is_derived_from_a_real_regular_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / "linux.iso"
            image.write_bytes(b"retained image")
            fingerprint = benchmark.fingerprint_regular_file(image)
            self.assertEqual(fingerprint["file_name"], "linux.iso")
            self.assertEqual(fingerprint["size_bytes"], len(b"retained image"))
            self.assertEqual(
                fingerprint["sha256"],
                __import__("hashlib").sha256(b"retained image").hexdigest(),
            )

    @unittest.skipUnless(
        hasattr(os, "O_NOFOLLOW") and hasattr(os, "symlink"),
        "requires no-follow symlink checks",
    )
    def test_symlinked_artifact_and_os_image_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.write_bytes(b"bytes")
            link = root / "link"
            link.symlink_to(target)
            with self.assertRaisesRegex(benchmark.CaptureError, "without following"):
                benchmark.read_regular_file_once(link)
            with self.assertRaisesRegex(benchmark.CaptureError, "without following"):
                benchmark.fingerprint_regular_file(link)


if __name__ == "__main__":
    unittest.main()
