"""Structural custody regressions; synthetic data is not native qualification."""

import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import bundle


class CustodyTests(unittest.TestCase):
    def setUp(self):
        """Initialize test fixtures with synthetic payloads and expected values."""
        self.payloads = {"source/libsodium-stable/LICENSE": b"synthetic notice\n",
                         "metadata/selection.json": b'{"synthetic":true}\n'}
        self.expected = {name: (len(data), bundle.sha256(data))
                         for name, data in self.payloads.items()}
        self.good = bundle.deterministic_tar(self.payloads)

    def altered_tar(self, members, mode="w"):
        """Create tar archive with specified members for testing various structural scenarios."""
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode=mode, format=tarfile.USTAR_FORMAT) as archive:
            for name, payload, kind in members:
                member = tarfile.TarInfo(name)
                member.mode = 0o644
                member.type = kind
                member.size = len(payload) if kind == tarfile.REGTYPE else 0
                if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                    member.linkname = "LICENSE"
                archive.addfile(member, io.BytesIO(payload))
        return output.getvalue()

    def assert_invalid(self, data):
        """Assert that verify_bytes raises an error for invalid data."""
        with self.assertRaises((bundle.CustodyError, tarfile.TarError, EOFError)):
            bundle.verify_bytes(data, self.expected)

    def test_frozen_selection_and_original_45_entries(self):
        """Test that frozen selection contains 135 entries and original inventory has 45 entries."""
        entries, metadata = bundle.load_selection()
        self.assertEqual(len(entries), 135)
        self.assertEqual(sum(entry["bytes"] for entry in entries), 2_844_540)
        self.assertEqual(bundle.sha256(metadata["metadata/original-inventory.json"]),
                         bundle.ORIGINAL_SHA256)
        self.assertEqual(len(json.loads(metadata["metadata/original-inventory.json"])), 45)

    def test_synthetic_round_trip_and_determinism(self):
        """Test that synthetic bundle verifies successfully and deterministic tar produces identical output regardless of input order."""
        bundle.verify_bytes(self.good, self.expected)
        self.assertEqual(self.good, bundle.deterministic_tar(dict(reversed(list(self.payloads.items())))))

    def test_content_and_self_reported_manifest_cannot_approve_tampering(self):
        """Test that content changes are detected even if manifest is also modified."""
        changed = dict(self.payloads)
        changed["source/libsodium-stable/LICENSE"] = b"Synthetic notice\n"
        self.assert_invalid(bundle.deterministic_tar(changed))
        changed["metadata/selection.json"] = b'{"tampering":"approved"}\n'
        self.assert_invalid(bundle.deterministic_tar(changed))

    def test_missing_extra_duplicate_and_wrong_size(self):
        """Test that missing members, extra members, duplicates, and size mismatches are rejected."""
        rows = [(name, data, tarfile.REGTYPE) for name, data in self.payloads.items()]
        for altered in (rows[:-1], rows + [("extra", b"", tarfile.REGTYPE)],
                        rows + rows[:1], [(rows[0][0], b"", tarfile.REGTYPE)] + rows[1:]):
            with self.subTest(altered=altered):
                self.assert_invalid(self.altered_tar(altered))

    def test_unsafe_and_nonregular_members(self):
        """Test that unsafe paths and nonregular file types (symlinks, directories, FIFOs) are rejected."""
        name = next(iter(self.payloads))
        for path in ("/absolute", "../escape", "a/../escape", "a//b", "./a", "a\\b"):
            with self.subTest(path=path):
                self.assertFalse(bundle.safe_path(path))
                self.assert_invalid(self.altered_tar([(path, b"", tarfile.REGTYPE)]))
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.DIRTYPE, tarfile.FIFOTYPE):
            with self.subTest(kind=kind):
                self.assert_invalid(self.altered_tar([(name, b"", kind)]))

    def test_truncated_trailing_concatenated_and_modified_framing(self):
        """Test that truncated, trailing data, concatenated archives, and modified tar metadata are rejected."""
        for data in (self.good[:100], self.good[:800], self.good[:-1],
                     self.good + b"\x00", self.good + b"unexpected",
                     self.good + self.good):
            with self.subTest(length=len(data)):
                self.assert_invalid(data)
        # Valid tar with changed metadata must still fail canonical framing.
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode="w", format=tarfile.USTAR_FORMAT) as archive:
            for name, payload in sorted(self.payloads.items()):
                member = tarfile.TarInfo(name)
                member.size, member.mode, member.mtime = len(payload), 0o644, 1
                archive.addfile(member, io.BytesIO(payload))
        self.assert_invalid(output.getvalue())
        padding = bytearray(self.good)
        padding[-1] = 1
        self.assert_invalid(bytes(padding))

    def test_oversized_bundle(self):
        """Test that bundles exceeding the maximum size are rejected."""
        self.assert_invalid(b"\0" * (bundle.MAX_BUNDLE_BYTES + 1))

    def test_strict_json(self):
        """Test that strict JSON parsing rejects duplicate keys, numeric constants, and invalid syntax."""
        for data in (b'{"a":1,"a":2}', b'{"a":NaN}', b'{"a":Infinity}',
                     b'{"a":}', b'{} trailing', b'\xff'):
            with self.subTest(data=data):
                with self.assertRaises((ValueError, UnicodeError)):
                    bundle.strict_json(data)

    def test_changed_checked_in_metadata_is_rejected(self):
        """Test that modifications to checked-in selection or inventory files are detected."""
        with patch.object(bundle, "read_regular", return_value=b"{}"):
            with self.assertRaisesRegex(bundle.CustodyError, "changed"):
                bundle.load_selection()

    def test_duplicate_selected_entries_are_rejected(self):
        """Test that duplicate entries in selection or source members are rejected."""
        selection = json.loads((bundle.ROOT / "selection.json").read_bytes())
        selection["files"].append(selection["files"][0])
        changed = json.dumps(selection).encode()
        original = (bundle.ROOT / "original-inventory.json").read_bytes()
        with patch.object(bundle, "read_regular", side_effect=[changed, original]), \
                patch.object(bundle, "SELECTION_SHA256", bundle.sha256(changed)):
            with self.assertRaisesRegex(bundle.CustodyError, "invariant"):
                bundle.load_selection()
        _payload, rows, entries = self.source_fixture()
        with self.assertRaisesRegex(bundle.CustodyError, "duplicate selected"):
            bundle.source_members(self.altered_tar(rows, mode="w:gz"), entries + entries)

    def test_regular_input_bound_links_and_fifo(self):
        """Test that read_regular enforces size bounds and rejects symlinks and FIFOs."""
        with tempfile.TemporaryDirectory(prefix="fasti-notice-structural-") as directory:
            root = Path(directory)
            regular = root / "regular"
            regular.write_bytes(b"abc")
            self.assertEqual(bundle.read_regular(regular, 3), b"abc")
            with self.assertRaises(bundle.CustodyError):
                bundle.read_regular(regular, 2)
            link = root / "link"
            link.symlink_to(regular)
            with self.assertRaises(OSError):
                bundle.read_regular(link, 3)
            fifo = root / "fifo"
            os.mkfifo(fifo)
            with self.assertRaises(bundle.CustodyError):
                bundle.read_regular(fifo, 3)

    def source_fixture(self):
        """Create synthetic source fixture with payload, tar rows, and entry metadata."""
        payload = b"synthetic source, not native code\n"
        rows = [("libsodium-stable/LICENSE", payload, tarfile.REGTYPE)]
        entries = [{"path": "LICENSE", "sha256": bundle.sha256(payload), "bytes": len(payload)}]
        return payload, rows, entries

    def test_source_member_custody(self):
        """Test that source_members extracts valid members and rejects unsafe paths, wrong types, and content mismatches."""
        payload, rows, entries = self.source_fixture()
        data = self.altered_tar(rows, mode="w:gz")
        self.assertEqual(bundle.source_members(data, entries),
                         {bundle.SOURCE_PREFIX + "LICENSE": payload})
        for altered in ([], rows + rows, [("../LICENSE", payload, tarfile.REGTYPE)],
                        [(rows[0][0], payload, tarfile.SYMTYPE)],
                        [(rows[0][0], payload + b"x", tarfile.REGTYPE)],
                        [(rows[0][0], b"S" + payload[1:], tarfile.REGTYPE)]):
            with self.subTest(altered=altered):
                with self.assertRaises(bundle.CustodyError):
                    bundle.source_members(self.altered_tar(altered, mode="w:gz"), entries)

    def test_build_requires_pinned_archive(self):
        """Test that build rejects archives that do not match the pinned hash."""
        with tempfile.TemporaryDirectory(prefix="fasti-notice-structural-") as directory:
            source, output = Path(directory) / "source", Path(directory) / "output"
            source.write_bytes(b"not the pinned archive")
            with self.assertRaisesRegex(bundle.CustodyError, "pinned"):
                bundle.build(source, output)
            self.assertFalse(output.exists())

    def test_synthetic_build_verify_no_overwrite_and_retained_failed_output(self):
        """Test synthetic build/verify, that existing outputs are not overwritten, and failed outputs are retained."""
        _payload, rows, entries = self.source_fixture()
        source_bytes = self.altered_tar(rows, mode="w:gz")
        with tempfile.TemporaryDirectory(prefix="fasti-notice-structural-") as directory:
            root = Path(directory)
            source, output = root / "source", root / "bundle.tar"
            source.write_bytes(source_bytes)
            # Explicit synthetic pins exercise the real file path, not native evidence.
            with patch.object(bundle, "ARCHIVE_BYTES", len(source_bytes)), \
                    patch.object(bundle, "ARCHIVE_SHA256", bundle.sha256(source_bytes)), \
                    patch.object(bundle, "load_selection", return_value=(entries, {})):
                result = bundle.build(source, output)
                self.assertEqual(result["source_files"], 1)
                original = output.read_bytes()
                with self.assertRaises(FileExistsError):
                    bundle.build(source, output)
                self.assertEqual(output.read_bytes(), original)
                link, fifo = root / "output-link", root / "output-fifo"
                link.symlink_to(output)
                os.mkfifo(fifo)
                for occupied in (link, fifo):
                    with self.subTest(output=occupied.name):
                        with self.assertRaises(FileExistsError):
                            bundle.build(source, occupied)
                self.assertEqual(output.read_bytes(), original)
                failed = root / "failed.tar"
                with patch.object(bundle, "verify", side_effect=bundle.CustodyError("failure")):
                    with self.assertRaises(bundle.CustodyError):
                        bundle.build(source, failed)
                self.assertEqual(failed.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
