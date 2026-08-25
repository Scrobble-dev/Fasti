#!/usr/bin/env python3
"""Portable negative tests for the Linux-only B1 capture trust boundary."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import importlib.util
import io
import json
import os
import socket
import stat
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
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
                ["raspberrypi,5-model-b", "brcm,bcm2712"],
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

    def test_pi_substring_or_incomplete_soc_proof_cannot_claim_champion(self) -> None:
        self.assertEqual(
            benchmark.derive_hardware_profile(
                "ARMv8",
                "Raspberry Pi 5 compatible test board",
                ["raspberrypi,5-model-b", "brcm,bcm2712"],
            ),
            "unclassified",
        )
        self.assertEqual(
            benchmark.derive_hardware_profile(
                "ARMv8",
                "Raspberry Pi 5 Model B Rev 1.0",
                ["raspberrypi,5-model-b"],
            ),
            "unclassified",
        )

    def test_j4125_virtualization_signals_fail_closed(self) -> None:
        dmi = {"sys_vendor": "Physical Vendor", "product_name": "J4125 Box"}
        with self.assertRaisesRegex(benchmark.CaptureError, "virtualization"):
            benchmark.physicality_evidence(
                "j4125_calibrated", set(), dmi, "kvm", None
            )
        with self.assertRaisesRegex(benchmark.CaptureError, "hypervisor flag"):
            benchmark.physicality_evidence(
                "j4125_calibrated", {"hypervisor"}, dmi, "none", None
            )
        with self.assertRaisesRegex(benchmark.CaptureError, "identifies virtualized"):
            benchmark.physicality_evidence(
                "j4125_calibrated",
                set(),
                {"sys_vendor": "QEMU", "product_name": "Standard PC"},
                "none",
                None,
            )

    def test_physical_j4125_requires_independent_positive_evidence(self) -> None:
        proof = benchmark.physicality_evidence(
            "j4125_calibrated",
            {"sse4_2"},
            {"sys_vendor": "Lab Vendor", "product_name": "J4125 Appliance"},
            "none",
            None,
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
        self.recipe = "5" * 64
        self.context_archive = "6" * 64
        self.labels = {
            "org.opencontainers.image.revision": self.commit,
            "dev.scrobble.fasti.source.tree": self.tree,
            "dev.scrobble.fasti.contracts": self.contract,
            "dev.scrobble.fasti.build.recipe.sha256": self.recipe,
            "dev.scrobble.fasti.build.context.archive.sha256": self.context_archive,
        }

    def _source(self) -> dict[str, str]:
        return {
            "git_commit": self.commit,
            "git_tree": self.tree,
            "contract_ref": self.contract,
            "build_recipe_sha256": self.recipe,
            "build_context_archive_sha256": self.context_archive,
        }

    def _saved_oci_fixture(
        self,
        *,
        layers: list[tuple[str, bytes]] | None = None,
        config: dict[str, object] | bytes | None = None,
        manifest: list[dict[str, object]] | bytes | None = None,
    ) -> tuple[str, list[tuple[str, bytes, bytes]]]:
        layers = layers or [("layer/layer.tar", b"layer bytes")]
        if config is None:
            config = {
                "architecture": "amd64",
                "os": "linux",
                "config": {"Labels": self.labels},
                "rootfs": {
                    "type": "layers",
                    "diff_ids": [
                        "sha256:" + hashlib.sha256(payload).hexdigest()
                        for _, payload in layers
                    ]
                },
            }
        config_bytes = (
            config
            if isinstance(config, bytes)
            else json.dumps(config, separators=(",", ":")).encode()
        )
        image_id = "sha256:" + hashlib.sha256(config_bytes).hexdigest()
        config_path = image_id.removeprefix("sha256:") + ".json"
        if manifest is None:
            manifest = [{"Config": config_path, "Layers": [name for name, _ in layers]}]
        manifest_bytes = (
            manifest
            if isinstance(manifest, bytes)
            else json.dumps(manifest, separators=(",", ":")).encode()
        )
        members = [
            ("manifest.json", manifest_bytes, tarfile.REGTYPE),
            (config_path, config_bytes, tarfile.REGTYPE),
            *((name, payload, tarfile.REGTYPE) for name, payload in layers),
        ]
        return image_id, members

    def _oci_layout_fixture(
        self,
        *,
        layer_media_types: list[str] | None = None,
        config_size_delta: int = 0,
        layer_size_delta: int = 0,
        manifest_size_delta: int = 0,
        target_size_delta: int = 0,
        diff_ids: list[str] | None = None,
        rootfs_type: str = "layers",
        config_os: str = "linux",
        nested_index: bool = False,
    ) -> tuple[str, list[tuple[str, bytes, bytes]], dict[str, str]]:
        uncompressed_layers = [b"plain layer", b"gzip layer"]
        media_types = layer_media_types or [
            "application/vnd.oci.image.layer.v1.tar",
            "application/vnd.oci.image.layer.v1.tar+gzip",
        ]
        blobs = [
            uncompressed_layers[0],
            gzip.compress(uncompressed_layers[1], mtime=0),
        ]

        def descriptor(payload: bytes, media_type: str, size_delta: int = 0) -> dict[str, object]:
            return {
                "mediaType": media_type,
                "digest": "sha256:" + hashlib.sha256(payload).hexdigest(),
                "size": len(payload) + size_delta,
            }

        config_bytes = json.dumps(
            {
                "architecture": "amd64",
                "os": config_os,
                "config": {"Labels": self.labels},
                "rootfs": {
                    "type": rootfs_type,
                    "diff_ids": (
                        diff_ids
                        if diff_ids is not None
                        else [
                            "sha256:" + hashlib.sha256(payload).hexdigest()
                            for payload in uncompressed_layers
                        ]
                    )
                },
            },
            separators=(",", ":"),
        ).encode()
        config_descriptor = descriptor(
            config_bytes,
            "application/vnd.oci.image.config.v1+json",
            config_size_delta,
        )
        layer_descriptors = [
            descriptor(
                payload,
                media_type,
                layer_size_delta if index == 0 else 0,
            )
            for index, (payload, media_type) in enumerate(zip(blobs, media_types, strict=True))
        ]
        manifest_bytes = json.dumps(
            {
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "config": config_descriptor,
                "layers": layer_descriptors,
            },
            separators=(",", ":"),
        ).encode()
        manifest_descriptor = descriptor(
            manifest_bytes,
            "application/vnd.oci.image.manifest.v1+json",
            manifest_size_delta,
        )

        def path(item: dict[str, object]) -> str:
            return "blobs/sha256/" + str(item["digest"]).removeprefix("sha256:")

        config_path = path(config_descriptor)
        layer_paths = [path(item) for item in layer_descriptors]
        target_descriptor = manifest_descriptor
        graph_members: list[tuple[str, bytes, bytes]] = []
        if nested_index:
            nested_bytes = json.dumps(
                {"schemaVersion": 2, "manifests": [manifest_descriptor]},
                separators=(",", ":"),
            ).encode()
            target_descriptor = descriptor(
                nested_bytes,
                "application/vnd.oci.image.index.v1+json",
                target_size_delta,
            )
            graph_members.append(
                (path(target_descriptor), nested_bytes, tarfile.REGTYPE)
            )
        image_id = str(target_descriptor["digest"])
        index_bytes = json.dumps(
            {"schemaVersion": 2, "manifests": [target_descriptor]},
            separators=(",", ":"),
        ).encode()
        legacy_manifest = json.dumps(
            [{"Config": config_path, "Layers": layer_paths}],
            separators=(",", ":"),
        ).encode()
        members = [
            ("manifest.json", legacy_manifest, tarfile.REGTYPE),
            ("oci-layout", b'{"imageLayoutVersion":"1.0.0"}', tarfile.REGTYPE),
            ("index.json", index_bytes, tarfile.REGTYPE),
            *graph_members,
            (path(manifest_descriptor), manifest_bytes, tarfile.REGTYPE),
            (config_path, config_bytes, tarfile.REGTYPE),
            *(
                (layer_path, payload, tarfile.REGTYPE)
                for layer_path, payload in zip(layer_paths, blobs, strict=True)
            ),
        ]
        return image_id, members, {
            "config": config_path,
            "layer": layer_paths[0],
            "manifest": path(manifest_descriptor),
            "target": path(target_descriptor),
        }

    @staticmethod
    def _write_saved_oci_archive(
        path: Path, members: list[tuple[str, bytes, bytes]]
    ) -> None:
        with tarfile.open(path, "w:gz") as archive:
            for member_name, payload, member_type in members:
                info = tarfile.TarInfo(member_name)
                info.type = member_type
                info.size = len(payload) if member_type == tarfile.REGTYPE else 0
                archive.addfile(info, io.BytesIO(payload) if info.size else None)

    def _assert_saved_oci_rejected(
        self,
        image_id: str,
        members: list[tuple[str, bytes, bytes]],
        message: str,
        exception: type[BaseException] = benchmark.CaptureError,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            archive_path = Path(temp_name) / "image.tar.gz"
            self._write_saved_oci_archive(archive_path, members)
            with self.assertRaisesRegex(exception, message):
                benchmark.saved_oci_layer_bytes(
                    archive_path, image_id, self._source()
                )

    def test_saved_oci_size_comes_from_bound_manifest_layers(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            archive_path = Path(temp_name) / "image.tar.gz"
            image_id, members = self._saved_oci_fixture()
            self._write_saved_oci_archive(archive_path, members)
            self.assertEqual(
                benchmark.saved_oci_layer_bytes(archive_path, image_id, self._source()),
                len(b"layer bytes"),
            )
            members[-1] = ("layer/layer.tar", b"substituted", tarfile.REGTYPE)
            self._write_saved_oci_archive(archive_path, members)
            with self.assertRaisesRegex(benchmark.CaptureError, "layer bytes"):
                benchmark.saved_oci_layer_bytes(
                    archive_path, image_id, self._source()
                )

    def test_saved_oci_accepts_containerd_layout_and_counts_unpacked_layers(self) -> None:
        for nested_index in (False, True):
            with self.subTest(nested_index=nested_index):
                image_id, members, _ = self._oci_layout_fixture(
                    nested_index=nested_index
                )
                with tempfile.TemporaryDirectory() as temp_name:
                    archive_path = Path(temp_name) / "image.tar.gz"
                    self._write_saved_oci_archive(archive_path, members)
                    self.assertEqual(
                        benchmark.saved_oci_layer_bytes(
                            archive_path, image_id, self._source()
                        ),
                        len(b"plain layer") + len(b"gzip layer"),
                    )

    def test_saved_oci_layout_binds_the_exported_target_image_id(self) -> None:
        _, members, _ = self._oci_layout_fixture()
        self._assert_saved_oci_rejected(
            "sha256:" + "0" * 64,
            members,
            "bind exactly one immutable image ID",
        )

    def test_saved_oci_layout_verifies_config_digest_and_size(self) -> None:
        image_id, members, paths = self._oci_layout_fixture()
        corrupted = [
            (name, b"x" * len(payload) if name == paths["config"] else payload, kind)
            for name, payload, kind in members
        ]
        self._assert_saved_oci_rejected(image_id, corrupted, "descriptor digest")

        sized_id, sized_members, _ = self._oci_layout_fixture(config_size_delta=1)
        self._assert_saved_oci_rejected(sized_id, sized_members, "descriptor size")

    def test_saved_oci_layout_verifies_layer_digest_and_size(self) -> None:
        image_id, members, paths = self._oci_layout_fixture()
        corrupted = [
            (name, b"substituted" if name == paths["layer"] else payload, kind)
            for name, payload, kind in members
        ]
        self._assert_saved_oci_rejected(image_id, corrupted, "descriptor digest")

        sized_id, sized_members, _ = self._oci_layout_fixture(layer_size_delta=1)
        self._assert_saved_oci_rejected(sized_id, sized_members, "descriptor size")

    def test_saved_oci_layout_rejects_unknown_layer_compression(self) -> None:
        image_id, members, _ = self._oci_layout_fixture(
            layer_media_types=[
                "application/vnd.oci.image.layer.v1.tar+zstd",
                "application/vnd.oci.image.layer.v1.tar+gzip",
            ]
        )
        self._assert_saved_oci_rejected(image_id, members, "layer compression")

        gzip_id, gzip_members, _ = self._oci_layout_fixture(
            layer_media_types=[
                "application/vnd.oci.image.layer.v1.tar+gzip",
                "application/vnd.oci.image.layer.v1.tar+gzip",
            ]
        )
        self._assert_saved_oci_rejected(gzip_id, gzip_members, "cannot decompress")

    def test_saved_oci_requires_layers_rootfs_type(self) -> None:
        legacy_id, legacy_members = self._saved_oci_fixture(
            config={
                "architecture": "amd64",
                "os": "linux",
                "config": {"Labels": self.labels},
                "rootfs": {
                    "diff_ids": [
                        "sha256:" + hashlib.sha256(b"layer bytes").hexdigest()
                    ]
                },
            }
        )
        self._assert_saved_oci_rejected(
            legacy_id, legacy_members, "target Linux layers"
        )

        oci_id, oci_members, _ = self._oci_layout_fixture(rootfs_type="other")
        self._assert_saved_oci_rejected(oci_id, oci_members, "invalid OCI image config")

    def test_saved_oci_requires_linux_config(self) -> None:
        legacy_id, legacy_members = self._saved_oci_fixture(
            config={
                "architecture": "amd64",
                "config": {"Labels": self.labels},
                "rootfs": {
                    "type": "layers",
                    "diff_ids": [
                        "sha256:" + hashlib.sha256(b"layer bytes").hexdigest()
                    ],
                },
            }
        )
        self._assert_saved_oci_rejected(
            legacy_id, legacy_members, "target Linux layers"
        )

        oci_id, oci_members, _ = self._oci_layout_fixture(config_os="windows")
        self._assert_saved_oci_rejected(
            oci_id, oci_members, "invalid OCI image config"
        )

    def test_saved_oci_layout_verifies_manifest_graph_descriptors(self) -> None:
        for nested_index, path_key, size_argument in [
            (False, "manifest", "manifest_size_delta"),
            (True, "target", "target_size_delta"),
        ]:
            with self.subTest(nested_index=nested_index, failure="digest"):
                image_id, members, paths = self._oci_layout_fixture(
                    nested_index=nested_index
                )
                corrupted = [
                    (
                        name,
                        b"x" * len(payload) if name == paths[path_key] else payload,
                        kind,
                    )
                    for name, payload, kind in members
                ]
                self._assert_saved_oci_rejected(
                    image_id, corrupted, "descriptor digest"
                )
            with self.subTest(nested_index=nested_index, failure="size"):
                arguments = {size_argument: 1, "nested_index": nested_index}
                image_id, members, _ = self._oci_layout_fixture(**arguments)
                self._assert_saved_oci_rejected(image_id, members, "descriptor size")

    def test_saved_oci_layout_checks_identity_and_gzip_diff_ids(self) -> None:
        valid = [
            "sha256:" + hashlib.sha256(payload).hexdigest()
            for payload in [b"plain layer", b"gzip layer"]
        ]
        for layer_index in range(2):
            with self.subTest(layer_index=layer_index):
                diff_ids = valid.copy()
                diff_ids[layer_index] = "sha256:" + "0" * 64
                image_id, members, _ = self._oci_layout_fixture(diff_ids=diff_ids)
                self._assert_saved_oci_rejected(
                    image_id, members, "layer bytes do not match"
                )

    def test_saved_oci_layout_has_a_separate_injectable_safety_ceiling(self) -> None:
        self.assertEqual(
            benchmark.SAVED_OCI_UNPACKED_SAFETY_CEILING_BYTES,
            4 * benchmark.ARTIFACT_LIMITS["oci_image_unpacked"],
        )
        image_id, members, _ = self._oci_layout_fixture()
        with tempfile.TemporaryDirectory() as temp_name:
            archive_path = Path(temp_name) / "image.tar.gz"
            self._write_saved_oci_archive(archive_path, members)
            with self.assertRaisesRegex(benchmark.CaptureError, "bounded unpacked size"):
                benchmark.saved_oci_layer_bytes(
                    archive_path,
                    image_id,
                    self._source(),
                    unpacked_safety_ceiling_bytes=20,
                )

    def test_saved_oci_rejects_outer_archive_size_and_entry_bombs(self) -> None:
        image_id, members, _ = self._oci_layout_fixture()
        with tempfile.TemporaryDirectory() as temp_name:
            archive_path = Path(temp_name) / "image.tar.gz"
            self._write_saved_oci_archive(archive_path, members)
            with (
                patch.object(
                    benchmark,
                    "SAVED_OCI_ARCHIVE_SAFETY_CEILING_BYTES",
                    archive_path.stat().st_size - 1,
                ),
                self.assertRaisesRegex(benchmark.CaptureError, "compressed archive"),
            ):
                benchmark.saved_oci_layer_bytes(
                    archive_path, image_id, self._source()
                )
            with (
                patch.object(benchmark, "SAVED_OCI_ENTRY_SAFETY_CEILING", 2),
                self.assertRaisesRegex(benchmark.CaptureError, "archive entry"),
            ):
                benchmark.saved_oci_layer_bytes(
                    archive_path, image_id, self._source()
                )

    def test_saved_oci_rejects_unsafe_and_duplicate_tar_paths(self) -> None:
        image_id, members = self._saved_oci_fixture()
        cases = [
            (
                "parent traversal",
                [*members, ("../escape", b"x", tarfile.REGTYPE)],
                "unsafe path",
            ),
            (
                "absolute path",
                [*members, ("/escape", b"x", tarfile.REGTYPE)],
                "unsafe path",
            ),
            ("duplicate path", [*members, members[0]], "duplicate path"),
        ]
        for name, candidate, message in cases:
            with self.subTest(name=name):
                self._assert_saved_oci_rejected(image_id, candidate, message)

    def test_saved_oci_requires_bounded_regular_manifest_and_config(self) -> None:
        image_id, members = self._saved_oci_fixture()
        config_path = image_id.removeprefix("sha256:") + ".json"
        oversized = b"x" * (1024 * 1024 + 1)
        cases = [
            (
                "missing manifest",
                [member for member in members if member[0] != "manifest.json"],
                "manifest.json",
            ),
            (
                "non-regular manifest",
                [("manifest.json", b"", tarfile.DIRTYPE), *members[1:]],
                "manifest.json",
            ),
            (
                "oversized manifest",
                [("manifest.json", oversized, tarfile.REGTYPE), *members[1:]],
                "manifest.json",
            ),
            (
                "missing config",
                [member for member in members if member[0] != config_path],
                config_path,
            ),
            (
                "non-regular config",
                [
                    members[0],
                    (config_path, b"", tarfile.DIRTYPE),
                    *members[2:],
                ],
                config_path,
            ),
            (
                "oversized config",
                [
                    members[0],
                    (config_path, oversized, tarfile.REGTYPE),
                    *members[2:],
                ],
                config_path,
            ),
        ]
        for name, candidate, message in cases:
            with self.subTest(name=name):
                self._assert_saved_oci_rejected(image_id, candidate, message)

    def test_saved_oci_rejects_malformed_or_mismatched_manifest(self) -> None:
        image_id, members = self._saved_oci_fixture()
        cases = [
            (
                "malformed JSON",
                [("manifest.json", b"{", tarfile.REGTYPE), *members[1:]],
                json.JSONDecodeError,
                "property name",
            ),
            (
                "multiple images",
                [
                    (
                        "manifest.json",
                        json.dumps([{}, {}]).encode(),
                        tarfile.REGTYPE,
                    ),
                    *members[1:],
                ],
                benchmark.CaptureError,
                "exactly one image manifest",
            ),
            (
                "config mismatch",
                [
                    (
                        "manifest.json",
                        json.dumps(
                            [{"Config": "other.json", "Layers": ["layer/layer.tar"]}]
                        ).encode(),
                        tarfile.REGTYPE,
                    ),
                    *members[1:],
                ],
                benchmark.CaptureError,
                "config does not match",
            ),
        ]
        for name, candidate, exception, message in cases:
            with self.subTest(name=name):
                self._assert_saved_oci_rejected(
                    image_id, candidate, message, exception
                )

    def test_saved_oci_rejects_invalid_layer_inventory(self) -> None:
        image_id, members = self._saved_oci_fixture()
        config_path = image_id.removeprefix("sha256:") + ".json"

        def manifest(layers: list[str]) -> tuple[str, bytes, bytes]:
            return (
                "manifest.json",
                json.dumps([{"Config": config_path, "Layers": layers}]).encode(),
                tarfile.REGTYPE,
            )

        cases = [
            ("empty", [manifest([]), *members[1:]], "empty or duplicated"),
            (
                "duplicate",
                [
                    manifest(["layer/layer.tar", "layer/layer.tar"]),
                    *members[1:],
                ],
                "empty or duplicated",
            ),
            (
                "extra layer",
                [*members, ("extra/layer.tar", b"extra", tarfile.REGTYPE)],
                "layer files do not match",
            ),
        ]
        for name, candidate, message in cases:
            with self.subTest(name=name):
                self._assert_saved_oci_rejected(image_id, candidate, message)

    def test_saved_oci_rejects_invalid_config_digest_json_and_labels(self) -> None:
        image_id, members = self._saved_oci_fixture()
        config_path = image_id.removeprefix("sha256:") + ".json"
        self._assert_saved_oci_rejected(
            image_id,
            [members[0], (config_path, b"{}", tarfile.REGTYPE), *members[2:]],
            "config digest",
        )

        malformed_id, malformed = self._saved_oci_fixture(config=b"{")
        self._assert_saved_oci_rejected(
            malformed_id,
            malformed,
            "property name",
            json.JSONDecodeError,
        )

        stale_labels = {**self.labels, "org.opencontainers.image.revision": "9" * 40}
        stale_id, stale = self._saved_oci_fixture(
            config={
                "architecture": "amd64",
                "os": "linux",
                "config": {"Labels": stale_labels},
                "rootfs": {
                    "type": "layers",
                    "diff_ids": [
                        "sha256:" + hashlib.sha256(b"layer bytes").hexdigest()
                    ]
                },
            }
        )
        self._assert_saved_oci_rejected(stale_id, stale, "config label is stale")

    def test_saved_oci_rejects_invalid_diff_ids_and_multilayer_binding(self) -> None:
        layer_one = ("layer/one.tar", b"one")
        layer_two = ("layer/two.tar", b"two")
        layers = [layer_one, layer_two]
        valid_ids = [
            "sha256:" + hashlib.sha256(payload).hexdigest()
            for _, payload in layers
        ]
        configs = [
            ("non-list", "invalid", "layer identity"),
            ("wrong count", valid_ids[:1], "layer identity"),
            ("multi-layer missing identity", [], "layer identity"),
            (
                "multi-layer wrong second digest",
                [valid_ids[0], "sha256:" + "0" * 64],
                "layer bytes",
            ),
        ]
        for name, diff_ids, message in configs:
            with self.subTest(name=name):
                image_id, members = self._saved_oci_fixture(
                    layers=layers,
                    config={
                        "architecture": "amd64",
                        "os": "linux",
                        "config": {"Labels": self.labels},
                        "rootfs": {"type": "layers", "diff_ids": diff_ids},
                    },
                )
                self._assert_saved_oci_rejected(image_id, members, message)

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
                        "build_recipe_sha256": self.recipe,
                        "build_context_archive_sha256": self.context_archive,
                    },
                )

    def test_image_build_avoids_an_unretained_provenance_index(self) -> None:
        inspection = json.dumps(
            [{"Id": self.image_id, "Config": {"Labels": self.labels}}]
        )
        with patch.object(
            benchmark, "run_checked", return_value=inspection
        ) as run_checked:
            observed = benchmark.inspect_bound_image(
                "fasti:mutable", self._source()
            )
        self.assertEqual(observed["id"], self.image_id)
        run_checked.assert_called_once_with(
            ["docker", "image", "inspect", "fasti:mutable"]
        )
        self.assertEqual(
            benchmark.GOVERNED_BUILD_PROVENANCE_ARG, "--provenance=false"
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
                "build_recipe_sha256": self.recipe,
                "build_context_archive_sha256": self.context_archive,
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
                    "build_recipe_sha256": self.recipe,
                    "build_context_archive_sha256": self.context_archive,
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
                "build_recipe_sha256": self.recipe,
                "build_context_archive_sha256": self.context_archive,
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
    def test_idle_wait_extends_until_raw_samples_span_the_locked_window(self) -> None:
        class PhaseOffsetSampler:
            interval_seconds = 1.0
            error = None

            def __init__(self) -> None:
                self.calls = 0

            def records_snapshot(self) -> list[dict[str, int]]:
                self.calls += 1
                final = 900_000_000_000 if self.calls == 1 else 900_800_000_000
                return [
                    {"at_ns": 800_000_000},
                    {"at_ns": final},
                ]

            def is_running(self) -> bool:
                return True

        sampler = PhaseOffsetSampler()
        with patch.object(benchmark.time, "monotonic", return_value=0), patch.object(
            benchmark.time, "sleep"
        ):
            benchmark.wait_for_observed_steady_span(
                sampler,
                lambda: True,
                ready_at_ns=0,
                required_seconds=900,
            )
        self.assertEqual(sampler.calls, 2)

    def test_steady_gate_uses_maximum_and_retains_statistics(self) -> None:
        records = [
            {
                "at_ns": 1_000_000_000,
                "rss_bytes": 10,
                "cpu_runtime_ns": 1_000_000,
                "process_count": 1,
                "cgroup_current_bytes": 20,
                "cgroup_peak_bytes": 40,
                "cgroup_cpu_runtime_ns": 1_000_000,
                "cgroup_memory_limit_bytes": 2147483648,
                "cgroup_swap_limit_bytes": 0,
                "cgroup_oom_kill_count": 0,
            },
            {
                "at_ns": 2_000_000_000,
                "rss_bytes": 100,
                "cpu_runtime_ns": 2_000_000,
                "process_count": 1,
                "cgroup_current_bytes": 200,
                "cgroup_peak_bytes": 220,
                "cgroup_cpu_runtime_ns": 2_000_000,
                "cgroup_memory_limit_bytes": 2147483648,
                "cgroup_swap_limit_bytes": 0,
                "cgroup_oom_kill_count": 0,
            },
            {
                "at_ns": 3_000_000_000,
                "rss_bytes": 20,
                "cpu_runtime_ns": 3_000_000,
                "process_count": 1,
                "cgroup_current_bytes": 30,
                "cgroup_peak_bytes": 220,
                "cgroup_cpu_runtime_ns": 3_000_000,
                "cgroup_memory_limit_bytes": 2147483648,
                "cgroup_swap_limit_bytes": 0,
                "cgroup_oom_kill_count": 0,
            },
        ]
        result = benchmark.metrics_from_records(
            records,
            started_at_ns=0,
            ready_at_ns=1_000_000_000,
            finished_at_ns=3_000_000_000,
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

    def test_idle_cpu_is_derived_only_from_the_measurement_window(self) -> None:
        records = [
            {"at_ns": 0, "rss_bytes": 10, "cpu_runtime_ns": 0, "process_count": 1},
            {"at_ns": 1_000_000_000, "rss_bytes": 10, "cpu_runtime_ns": 5_000_000, "process_count": 1},
            {"at_ns": 2_000_000_000, "rss_bytes": 10, "cpu_runtime_ns": 10_000_000, "process_count": 1},
        ]
        result = benchmark.metrics_from_records(
            records,
            started_at_ns=-600_000_000_000,
            ready_at_ns=0,
            finished_at_ns=2_000_000_000,
            with_cgroup=False,
            idle_cpu_window=True,
            startup_ready_at_ns=-599_900_000_000,
        )
        self.assertEqual(result["idle_cpu"]["average_percent_one_core"], 0.5)
        self.assertEqual(result["idle_cpu"]["p95_percent_one_core"], 0.5)
        self.assertLess(result["idle_cpu"]["p95_percent_one_core"], 1.0)

    def test_any_oom_kill_invalidates_cgroup_measurement(self) -> None:
        records = [
            {
                "at_ns": index * 1_000_000_000,
                "rss_bytes": 10,
                "cpu_runtime_ns": index * 1_000_000,
                "process_count": 1,
                "cgroup_current_bytes": 20,
                "cgroup_peak_bytes": 40,
                "cgroup_cpu_runtime_ns": index * 1_000_000,
                "cgroup_memory_limit_bytes": 2147483648,
                "cgroup_swap_limit_bytes": 0,
                "cgroup_oom_kill_count": 1 if index == 2 else 0,
            }
            for index in [1, 2, 3]
        ]
        with self.assertRaisesRegex(benchmark.CaptureError, "OOM kills"):
            benchmark.metrics_from_records(
                records,
                started_at_ns=0,
                ready_at_ns=1_000_000_000,
                finished_at_ns=3_000_000_000,
                with_cgroup=True,
            )


class LockedProfileTests(unittest.TestCase):
    def valid_storage(self) -> dict[str, object]:
        return {
            "transport": "sata",
            "storage_class": "ssd",
            "rotational": False,
            "usb_link_speed_mbps": None,
        }

    def test_j4125_requires_four_cores_and_ssd(self) -> None:
        with self.assertRaisesRegex(benchmark.CaptureError, "exactly four"):
            benchmark.validate_profile_requirements(
                "j4125_calibrated",
                {"id": "debian", "version_codename": "trixie"},
                "x86_64",
                8,
                8 * 1024**3,
                self.valid_storage(),
                {"file_name": "unknown.img.xz", "sha256": "a" * 64},
            )
        storage = self.valid_storage()
        storage["storage_class"] = "emmc_or_flash"
        with self.assertRaisesRegex(benchmark.CaptureError, "identified SSD"):
            benchmark.validate_profile_requirements(
                "j4125_calibrated",
                {"id": "debian", "version_codename": "trixie"},
                "x86_64",
                4,
                8 * 1024**3,
                storage,
                {"file_name": "unknown.img.xz", "sha256": "a" * 64},
            )

    def test_pi_requires_trixie_usb3_ssd_and_four_gb_class(self) -> None:
        storage = self.valid_storage()
        storage.update({"transport": "usb", "usb_link_speed_mbps": 5000})
        with self.assertRaisesRegex(benchmark.CaptureError, "runtime OS release"):
            benchmark.validate_profile_requirements(
                "raspberry_pi_5_champion",
                {"id": "raspbian", "version_codename": "bookworm"},
                "aarch64",
                4,
                4 * 1024**3,
                storage,
                {"sha256": "a" * 64},
            )
        with self.assertRaisesRegex(benchmark.CaptureError, "exactly 4"):
            benchmark.validate_profile_requirements(
                "raspberry_pi_5_champion",
                {"id": "raspbian", "version_codename": "trixie"},
                "aarch64",
                8,
                4 * 1024**3,
                storage,
                {"sha256": "a" * 64},
            )
        blocked_profile = json.loads(json.dumps(benchmark.PI_PROFILE))
        blocked_profile["approved_images"] = []
        blocked_profile["os_image_policy_status"] = "blocking_until_official_digest_pinned"
        with patch.object(benchmark, "PI_PROFILE", blocked_profile):
            with self.assertRaisesRegex(benchmark.CaptureError, "no official image digest"):
                benchmark.validate_profile_requirements(
                    "raspberry_pi_5_champion",
                    {"id": "raspbian", "version_codename": "trixie"},
                    "aarch64",
                    4,
                    4 * 1024**3,
                    storage,
                    {"file_name": "unknown.img.xz", "sha256": "a" * 64},
                )
        approved_image = benchmark.PI_PROFILE["approved_images"][0]
        approved_profile = json.loads(json.dumps(benchmark.PI_PROFILE))
        with patch.object(benchmark, "PI_PROFILE", approved_profile), patch.object(
            benchmark,
            "parse_pi_active_cooling",
            return_value={"status": "active"},
        ), patch.object(
            benchmark,
            "parse_pi_overclock_configuration",
            return_value={"status": "stock_no_overclock_detected"},
        ):
            result = benchmark.validate_profile_requirements(
                "raspberry_pi_5_champion",
                {"id": "raspbian", "version_codename": "trixie"},
                "aarch64",
                4,
                4 * 1024**3,
                storage,
                {
                    "file_name": approved_image["file_name"],
                    "sha256": approved_image["sha256"],
                },
            )
        self.assertEqual(result["storage"], "usb3_ssd")

    def test_pi_firmware_overclock_signal_fails_closed(self) -> None:
        with patch.object(benchmark, "require_command"), patch.object(
            benchmark,
            "run_checked",
            return_value="arm_freq=2400\nover_voltage=2\nforce_turbo=0",
        ):
            with self.assertRaisesRegex(benchmark.CaptureError, "not stock"):
                benchmark.parse_pi_overclock_configuration()

    def test_pi_requires_kernel_visible_active_fan(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            cooling_type = Path(temp_name) / "type"
            cooling_type.write_text("thermal-cpufreq-0\n", encoding="ascii")
            with patch.object(Path, "glob", return_value=[cooling_type]):
                with self.assertRaisesRegex(benchmark.CaptureError, "active fan"):
                    benchmark.parse_pi_active_cooling()
            cooling_type.write_text("pwm-fan\n", encoding="ascii")
            with patch.object(Path, "glob", return_value=[cooling_type]):
                proof = benchmark.parse_pi_active_cooling()
            self.assertEqual(proof["fan_types"], ["pwm-fan"])

    def test_j4125_oci_command_locks_two_gib_and_disables_swap(self) -> None:
        image_id = "sha256:" + "a" * 64
        command = benchmark.oci_run_command(
            "subject",
            image_id,
            "exec true",
            benchmark.J4125_CGROUP_LIMIT_BYTES,
        )
        self.assertIn("--memory", command)
        expected = str(benchmark.J4125_CGROUP_LIMIT_BYTES)
        self.assertEqual(command[command.index("--memory") + 1], expected)
        self.assertEqual(command[command.index("--memory-swap") + 1], expected)


class ProvenanceAndStorageTests(unittest.TestCase):
    def test_contract_sdk_build_uses_supported_pnpm_run_options(self) -> None:
        self.assertEqual(
            benchmark.CONTRACT_SDK_BUILD_COMMAND,
            ("pnpm", "--filter", "@fasti/sdk", "build"),
        )

    def test_storage_fingerprint_targets_root_mount(self) -> None:
        commands: list[list[str]] = []

        def fake_run(args: list[str], **_kwargs: object) -> str:
            commands.append(args)
            if args[0] == "findmnt":
                return "/dev/sda1 ext4 rw,noatime"
            if args[:3] == ["lsblk", "-dnro", "PKNAME"]:
                return "sda" if args[-1] == "/dev/sda1" else ""
            if args[:2] == ["lsblk", "-dnPb"]:
                return (
                    'NAME="/dev/sda" TYPE="disk" TRAN="sata" ROTA="0" '
                    'SIZE="1000000" MODEL="Fixture SSD" SERIAL="secret" WWN="wwn"'
                )
            if args[0] == "udevadm":
                return "ID_BUS=ata\nID_SSD=1"
            raise AssertionError(f"unexpected command: {args!r}")

        with patch.object(benchmark, "require_command"), patch.object(
            benchmark, "run_checked", side_effect=fake_run
        ):
            storage, _ = benchmark.parse_storage_identity()

        self.assertIn(
            ["findmnt", "-n", "-o", "SOURCE,FSTYPE,OPTIONS", "-T", "/"],
            commands,
        )
        self.assertEqual(storage["root_source"], "/dev/sda1")
        self.assertEqual(storage["storage_class"], "ssd")

    def test_flash_cannot_be_called_ssd(self) -> None:
        storage_class, proof = benchmark.classify_storage(
            "usb", False, {"ID_DRIVE_FLASH": "1"}
        )
        self.assertEqual(storage_class, "emmc_or_flash")
        self.assertIn("udev.ID_DRIVE_FLASH=1", proof)
        storage_class, proof = benchmark.classify_storage("usb", False, {})
        self.assertEqual(storage_class, "unknown_non_rotational")
        self.assertIn("no_exact_ssd_marker", proof)

    def test_multi_device_root_storage_is_refused_as_ambiguous(self) -> None:
        with patch.object(benchmark, "run_checked", return_value="sda\nsdb"):
            with self.assertRaisesRegex(benchmark.CaptureError, "multiple backing"):
                benchmark.top_level_block_device("/dev/mapper/root")

    def test_exact_ssd_markers_are_required(self) -> None:
        storage_class, proof = benchmark.classify_storage(
            "usb", False, {"ID_ATA_ROTATION_RATE_RPM": "0"}
        )
        self.assertEqual(storage_class, "ssd")
        self.assertIn("udev.ID_ATA_ROTATION_RATE_RPM=0", proof)
        self.assertEqual(benchmark.classify_storage("nvme", False, {})[0], "ssd")

    def test_retained_image_rejects_symlinks_and_missing_nofollow(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            root = Path(temp_name)
            image = root / "image.img"
            image.write_bytes(b"retained-image")
            link = root / "image-link.img"
            link.symlink_to(image)
            with self.assertRaisesRegex(benchmark.CaptureError, "without following"):
                benchmark.hash_retained_os_image(link)
            with patch.object(benchmark.os, "O_NOFOLLOW", None):
                with self.assertRaisesRegex(benchmark.CaptureError, "requires O_NOFOLLOW"):
                    benchmark.hash_retained_os_image(image)

    def test_retained_image_rejects_same_size_metadata_change_while_hashing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            image = Path(temp_name) / "image.img"
            image.write_bytes(b"same-size-image")
            common = {
                "st_mode": stat.S_IFREG | 0o600,
                "st_size": image.stat().st_size,
                "st_dev": 1,
                "st_ino": 2,
                "st_ctime_ns": 3,
            }
            before = SimpleNamespace(**common, st_mtime_ns=4)
            after = SimpleNamespace(**common, st_mtime_ns=5)
            with patch.object(benchmark.os, "fstat", side_effect=[before, after]):
                with self.assertRaisesRegex(benchmark.CaptureError, "changed while"):
                    benchmark.hash_retained_os_image(image)

    def test_exact_head_archive_excludes_ignored_and_uncommitted_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            root = Path(temp_name)
            repository = root / "repo"
            repository.mkdir()
            subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
            subprocess.run(
                ["git", "config", "user.email", "fixture@example.invalid"],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Fixture"], cwd=repository, check=True
            )
            recipe = repository / "benchmarks" / "b1" / "Dockerfile"
            recipe.parent.mkdir(parents=True)
            recipe.write_text("FROM scratch\n", encoding="utf-8")
            (repository / ".gitignore").write_text("ignored.txt\n", encoding="utf-8")
            (repository / "tracked.txt").write_text("tracked\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "fixture"], cwd=repository, check=True)
            (repository / "ignored.txt").write_text("ignored\n", encoding="utf-8")
            (repository / "uncommitted.txt").write_text("uncommitted\n", encoding="utf-8")

            destination = root / "context"
            provenance = benchmark.create_exact_git_archive_context(
                destination, repository
            )
            self.assertEqual(provenance["method"], "verifier_owned_git_archive_head")
            self.assertTrue((destination / "tracked.txt").is_file())
            self.assertFalse((destination / "ignored.txt").exists())
            self.assertFalse((destination / "uncommitted.txt").exists())

    def test_content_addressed_publication_never_overwrites(self) -> None:
        with tempfile.TemporaryDirectory() as temp_name:
            receipt_root = Path(temp_name)
            source = receipt_root / "source.tar.gz"
            source.write_bytes(b"governed-artifact")
            first = benchmark.publish_content_addressed_artifact(source, receipt_root)
            second = benchmark.publish_content_addressed_artifact(source, receipt_root)
            self.assertEqual(first, second)

            destination = receipt_root / first["path"]
            destination.unlink()
            destination.write_bytes(b"substituted")
            with self.assertRaisesRegex(
                benchmark.CaptureError, "contains different bytes"
            ):
                benchmark.publish_content_addressed_artifact(source, receipt_root)

    def test_harness_observations_round_trip_through_j4125_node_validator(self) -> None:
        emitted = subprocess.run(
            [
                "node",
                str(ROOT / "benchmarks" / "b1" / "validate-evidence.mjs"),
                "--emit-j4125-test-fixture",
            ],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=True,
        )
        evidence = json.loads(emitted.stdout)
        records = [
            {
                "at_ns": elapsed,
                "rss_bytes": rss,
                "cpu_runtime_ns": cpu,
                "process_count": 1,
            }
            for elapsed, rss, cpu in [
                (0, 9_000_001, 0),
                (600_000_000_000, 7_000_001, 1_000_000),
                (1_050_000_000_000, 7_500_001, 2_000_000),
                (1_500_000_000_000, 8_000_001, 3_000_000),
            ]
        ]
        sample = benchmark.metrics_from_records(
            records,
            started_at_ns=0,
            ready_at_ns=600_000_000_000,
            finished_at_ns=1_500_000_000_000,
            with_cgroup=False,
            idle_cpu_window=True,
            startup_ready_at_ns=1_000_000,
        )
        sample["run"] = 1
        scenario = next(
            item for item in evidence["scenarios"] if item["id"] == "native_fastid_idle"
        )
        scenario["samples"][0] = sample
        for field in [
            "startup_ms",
            "steady_process_tree_rss_bytes",
            "peak_process_tree_rss_bytes",
            "process_tree_cpu_seconds",
            "process_tree_cpu_percent",
            "process_count_peak",
        ]:
            scenario["summary"][field] = benchmark.summarize(
                [item[field] for item in scenario["samples"]]
            )
        verdict = next(
            item
            for item in evidence["idle_cpu_verdicts"]
            if item["scenario"] == "native_fastid_idle"
        )
        verdict["measured_worst_average_percent_one_core"] = max(
            item["idle_cpu"]["average_percent_one_core"]
            for item in scenario["samples"]
        )
        verdict["measured_worst_p95_percent_one_core"] = max(
            item["idle_cpu"]["p95_percent_one_core"] for item in scenario["samples"]
        )

        with tempfile.TemporaryDirectory() as temp_name:
            receipt_root = Path(temp_name)
            receipt = receipt_root / "j4125.json"
            artifact_root = receipt_root / "artifacts" / "sha256"
            artifact_root.mkdir(parents=True)
            artifact_bytes = {
                "oci_image_compressed": b"oci-image-bytes",
                "contract_pack_compressed": b"contract-pack-bytes",
            }
            size_fields = {
                "oci_image_compressed": (
                    "oci_image_compressed_bytes",
                    "oci_image_compressed_sha256",
                ),
                "contract_pack_compressed": (
                    "contract_pack_compressed_bytes",
                    "contract_pack_compressed_sha256",
                ),
            }
            for kind, payload in artifact_bytes.items():
                digest = hashlib.sha256(payload).hexdigest()
                path = artifact_root / f"{digest}.tar.gz"
                path.write_bytes(payload)
                evidence["retained_artifacts"][kind] = {
                    "path": f"artifacts/sha256/{digest}.tar.gz",
                    "sha256": digest,
                    "size_bytes": len(payload),
                }
                bytes_field, digest_field = size_fields[kind]
                evidence["artifact_sizes"][bytes_field] = len(payload)
                evidence["artifact_sizes"][digest_field] = digest
                budget_verdict = next(
                    item
                    for item in evidence["artifact_budget_verdicts"]
                    if item["budget"] == kind
                )
                budget_verdict["measured_bytes"] = len(payload)
                budget_verdict["status"] = "pass"
            receipt.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
            validator = ROOT / "benchmarks" / "b1" / "validate-evidence.mjs"
            subprocess.run(
                ["node", str(validator), str(receipt)],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=True,
            )

            image_reference = evidence["retained_artifacts"]["oci_image_compressed"]
            image_path = receipt_root / image_reference["path"]
            image_payload = image_path.read_bytes()
            image_path.unlink()
            real_path = receipt_root / "real-image.tar.gz"
            real_path.write_bytes(image_payload)
            image_path.symlink_to(real_path)
            refused = subprocess.run(
                ["node", str(validator), str(receipt)],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn("symbolic link", refused.stderr + refused.stdout)


class EvidenceInputTests(unittest.TestCase):
    def test_placeholders_are_rejected(self) -> None:
        for value in ["TBD", "placeholder runner", "unassigned", "example"]:
            with self.subTest(value=value), self.assertRaises(benchmark.CaptureError):
                benchmark.reject_placeholder("runner ID", value)
        self.assertEqual(
            benchmark.reject_placeholder("custodian", "Ryan Winkler"),
            "Ryan Winkler",
        )

    def test_machine_fingerprint_is_stable_and_does_not_expose_machine_id(self) -> None:
        with patch.object(benchmark, "read_machine_id", return_value=b"a" * 32):
            first = benchmark.stable_machine_fingerprint(
                "CPU", "Device", {"sys_vendor": "Vendor"}, "b" * 64
            )
            second = benchmark.stable_machine_fingerprint(
                "CPU", "Device", {"sys_vendor": "Vendor"}, "b" * 64
            )
        self.assertEqual(first, second)
        self.assertRegex(first, r"^[0-9a-f]{64}$")
        self.assertNotIn("a" * 32, first)

    def test_capture_arguments_enforce_five_runs_and_locked_idle_windows(self) -> None:
        base = {
            "command": "capture",
            "output": Path("evidence.json"),
            "runner_id": "pi5-lab-01",
            "custodian": "Ryan Winkler",
            "os_image": Path("retained.img"),
            "image": "fasti:b1-local",
            "repetitions": 4,
            "steady_window_seconds": 5.0,
            "idle_warmup_seconds": 600.0,
            "idle_measurement_seconds": 900.0,
            "sample_interval_ms": 1000,
            "startup_timeout_seconds": 15.0,
            "build_timeout_seconds": 1800.0,
        }
        with self.assertRaisesRegex(benchmark.CaptureError, "five|5"):
            benchmark.validate_arguments(argparse.Namespace(**base))
        base["repetitions"] = 5
        base["idle_warmup_seconds"] = 599.0
        with self.assertRaisesRegex(benchmark.CaptureError, "locked to 600"):
            benchmark.validate_arguments(argparse.Namespace(**base))

    def test_artifact_verdicts_mark_only_unbuilt_native_distribution_na(self) -> None:
        verdicts = benchmark.artifact_budget_verdicts(
            {
                "oci_image_compressed_bytes": 49 * 1024**2,
                "oci_image_bytes": 101 * 1024**2,
                "contract_pack_compressed_bytes": 4 * 1024**2,
            }
        )
        statuses = {verdict["budget"]: verdict["status"] for verdict in verdicts}
        self.assertEqual(statuses["native_runtime_installed"], "not_applicable")
        self.assertEqual(statuses["native_archive_compressed"], "not_applicable")
        self.assertEqual(statuses["oci_image_compressed"], "pass")
        self.assertEqual(statuses["oci_image_unpacked"], "fail")
        self.assertEqual(statuses["contract_pack_compressed"], "pass")


if __name__ == "__main__":
    unittest.main()
