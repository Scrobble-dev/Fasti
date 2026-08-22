#!/usr/bin/env python3
"""Portable negative tests for the Linux-only B1 capture trust boundary."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import socket
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "benchmark-b1.py"
SPEC = importlib.util.spec_from_file_location("fasti_benchmark_b1", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
benchmark = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(benchmark)


class HardwareProfileTests(unittest.TestCase):
    def test_profiles_are_derived_from_observed_fingerprint(self) -> None:
        self.assertEqual(
            benchmark.derive_hardware_profile(
                "ARMv8 Processor rev 1 (v8l)",
                "Raspberry Pi 5 Model B Rev 1.0",
            ),
            "raspberry_pi_5_champion",
        )
        self.assertEqual(
            benchmark.derive_hardware_profile(
                "Intel(R) Celeron(R) CPU J4125 @ 2.00GHz",
                None,
            ),
            "j4125_calibrated",
        )

    def test_fixture_words_cannot_claim_target_hardware(self) -> None:
        self.assertEqual(
            benchmark.derive_hardware_profile("self-test", None),
            "unclassified",
        )

    def test_j4125_virtualization_signals_fail_closed(self) -> None:
        dmi = {"sys_vendor": "Physical Vendor", "product_name": "J4125 Box"}
        with self.assertRaisesRegex(benchmark.CaptureError, "virtualization"):
            benchmark.physicality_evidence(
                "j4125_calibrated", set(), dmi, "kvm"
            )
        with self.assertRaisesRegex(benchmark.CaptureError, "hypervisor flag"):
            benchmark.physicality_evidence(
                "j4125_calibrated", {"hypervisor"}, dmi, "none"
            )
        with self.assertRaisesRegex(benchmark.CaptureError, "identifies virtualized"):
            benchmark.physicality_evidence(
                "j4125_calibrated",
                set(),
                {"sys_vendor": "QEMU", "product_name": "Standard PC"},
                "none",
            )

    def test_physical_j4125_requires_independent_positive_evidence(self) -> None:
        proof = benchmark.physicality_evidence(
            "j4125_calibrated",
            {"sse4_2"},
            {"sys_vendor": "Lab Vendor", "product_name": "J4125 Appliance"},
            "none",
        )
        self.assertEqual(proof["status"], "physical")
        self.assertEqual(proof["systemd_detect_virt"], "none")


class DockerLocalityTests(unittest.TestCase):
    def test_remote_endpoint_is_refused_before_pid_measurement(self) -> None:
        with self.assertRaisesRegex(benchmark.CaptureError, "local Unix socket"):
            benchmark.local_docker_socket_path("tcp://benchmark.example:2376")

    def test_regular_file_cannot_impersonate_local_socket(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            path = Path(temp_name) / "docker.sock"
            path.write_text("not a socket", encoding="utf-8")
            with self.assertRaisesRegex(benchmark.CaptureError, "not a Unix socket"):
                benchmark.local_docker_socket_path(f"unix://{path}")

    def test_bound_unix_socket_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            path = Path(temp_name) / "docker.sock"
            server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            try:
                server.bind(str(path))
                self.assertEqual(
                    benchmark.local_docker_socket_path(f"unix://{path}"),
                    path,
                )
            finally:
                server.close()

    def test_state_pid_must_correlate_to_exact_local_container_cgroup(self) -> None:
        container_id = "6" * 64
        benchmark.validate_container_cgroup_identity(
            f"/system.slice/docker-{container_id}.scope", container_id
        )
        with self.assertRaisesRegex(benchmark.CaptureError, "not correlated"):
            benchmark.validate_container_cgroup_identity(
                "/system.slice/docker-unrelated.scope", container_id
            )


class ImmutableSourceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.commit = "1" * 40
        self.tree = "2" * 40
        self.contract = "3" * 40
        self.image_id = "sha256:" + "4" * 64
        self.labels = {
            "org.opencontainers.image.revision": self.commit,
            "dev.scrobble.fasti.source.tree": self.tree,
            "dev.scrobble.fasti.contracts": self.contract,
        }

    def test_unlabeled_or_mismatched_image_is_refused(self) -> None:
        inspection = json.dumps(
            [
                {
                    "Id": self.image_id,
                    "Config": {
                        "Labels": {
                            **self.labels,
                            "org.opencontainers.image.revision": "9" * 40,
                        }
                    },
                }
            ]
        )
        with patch.object(benchmark, "run_checked", return_value=inspection):
            with self.assertRaisesRegex(benchmark.CaptureError, "must equal"):
                benchmark.inspect_bound_image(
                    "fasti:mutable",
                    {
                        "git_commit": self.commit,
                        "git_tree": self.tree,
                        "contract_ref": self.contract,
                    },
                )

    def test_measurement_command_refuses_mutable_tag(self) -> None:
        with self.assertRaisesRegex(benchmark.CaptureError, "immutable image ID"):
            benchmark.oci_run_command("subject", "fasti:mutable", "exec true")
        command = benchmark.oci_run_command("subject", self.image_id, "exec true")
        self.assertIn(self.image_id, command)

    def test_post_measurement_native_byte_change_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            binary = Path(temp_name) / "fastid"
            binary.write_bytes(b"before")
            source = {
                "git_commit": self.commit,
                "git_tree": self.tree,
                "contract_ref": self.contract,
                "native_fastid_sha256": hashlib.sha256(b"before").hexdigest(),
                "oci_image_ref": "fasti:mutable",
                "oci_image_id": self.image_id,
            }
            context = {
                "source": source,
                "docker_locality": {"locality": "verified_local_unix_socket"},
            }
            binary.write_bytes(b"after")
            args = argparse.Namespace(native_binary=binary)
            with (
                patch.object(
                    benchmark,
                    "ensure_clean_tree",
                    return_value=(self.commit, self.tree, self.contract),
                ),
                patch.object(
                    benchmark,
                    "verify_local_docker",
                    return_value=context["docker_locality"],
                ),
            ):
                with self.assertRaisesRegex(benchmark.CaptureError, "bytes changed"):
                    benchmark.verify_capture_inputs_unchanged(args, context)

    def test_post_measurement_clean_head_contract_change_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            binary = Path(temp_name) / "fastid"
            binary.write_bytes(b"stable")
            context = {
                "source": {
                    "git_commit": self.commit,
                    "git_tree": self.tree,
                    "contract_ref": self.contract,
                    "native_fastid_sha256": hashlib.sha256(b"stable").hexdigest(),
                    "oci_image_ref": "fasti:mutable",
                    "oci_image_id": self.image_id,
                },
                "docker_locality": {"locality": "verified_local_unix_socket"},
            }
            args = argparse.Namespace(native_binary=binary)
            with patch.object(
                benchmark,
                "ensure_clean_tree",
                return_value=(self.commit, self.tree, "9" * 40),
            ):
                with self.assertRaisesRegex(benchmark.CaptureError, "source identity changed"):
                    benchmark.verify_capture_inputs_unchanged(args, context)

    def test_post_measurement_dirty_tree_is_refused(self) -> None:
        args = argparse.Namespace(native_binary=Path("/not/reached"))
        context = {"source": {}, "docker_locality": {}}
        with patch.object(
            benchmark,
            "ensure_clean_tree",
            side_effect=benchmark.CaptureError(
                "performance evidence requires a clean source tree"
            ),
        ):
            with self.assertRaisesRegex(benchmark.CaptureError, "clean source tree"):
                benchmark.verify_capture_inputs_unchanged(args, context)

    def test_post_measurement_mutable_tag_move_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            binary = Path(temp_name) / "fastid"
            binary.write_bytes(b"stable")
            source = {
                "git_commit": self.commit,
                "git_tree": self.tree,
                "contract_ref": self.contract,
                "native_fastid_sha256": hashlib.sha256(b"stable").hexdigest(),
                "oci_image_ref": "fasti:mutable",
                "oci_image_id": self.image_id,
            }
            context = {
                "source": source,
                "docker_locality": {"locality": "verified_local_unix_socket"},
            }
            args = argparse.Namespace(native_binary=binary)
            moved = {"id": "sha256:" + "8" * 64, "source_labels": self.labels}
            immutable = {"id": self.image_id, "source_labels": self.labels}
            with (
                patch.object(
                    benchmark,
                    "ensure_clean_tree",
                    return_value=(self.commit, self.tree, self.contract),
                ),
                patch.object(
                    benchmark,
                    "verify_local_docker",
                    return_value=context["docker_locality"],
                ),
                patch.object(
                    benchmark,
                    "inspect_bound_image",
                    side_effect=[moved, immutable],
                ),
            ):
                with self.assertRaisesRegex(benchmark.CaptureError, "identity.*changed"):
                    benchmark.verify_capture_inputs_unchanged(args, context)


class IdleGateTests(unittest.TestCase):
    def test_steady_gate_uses_maximum_and_retains_statistics(self) -> None:
        records = [
            {
                "at": 1.0,
                "rss_bytes": 10,
                "cpu_ticks": 1,
                "process_count": 1,
                "cgroup_current_bytes": 20,
                "cgroup_peak_bytes": 40,
                "cgroup_cpu_seconds": 0.01,
            },
            {
                "at": 2.0,
                "rss_bytes": 100,
                "cpu_ticks": 2,
                "process_count": 1,
                "cgroup_current_bytes": 200,
                "cgroup_peak_bytes": 220,
                "cgroup_cpu_seconds": 0.02,
            },
            {
                "at": 3.0,
                "rss_bytes": 20,
                "cpu_ticks": 3,
                "process_count": 1,
                "cgroup_current_bytes": 30,
                "cgroup_peak_bytes": 220,
                "cgroup_cpu_seconds": 0.03,
            },
        ]
        result = benchmark.metrics_from_records(
            records,
            started_at=0.0,
            ready_at=1.0,
            finished_at=3.0,
            clock_ticks=100,
            with_cgroup=True,
        )
        self.assertEqual(result["steady_process_tree_rss_bytes"], 100)
        self.assertEqual(
            result["steady_process_tree_rss_statistics"],
            {"minimum": 10, "median": 20, "maximum": 100},
        )
        self.assertEqual(result["cgroup"]["steady_memory_current_bytes"], 200)
        self.assertEqual(
            result["cgroup"]["steady_memory_current_statistics"],
            {"minimum": 20, "median": 30, "maximum": 200},
        )


if __name__ == "__main__":
    unittest.main()
