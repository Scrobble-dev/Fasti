#!/usr/bin/env python3
"""Focused startup-boundary checks for the ordinary-browser smoke harness."""

from __future__ import annotations

import importlib.util
import json
import os
import sqlite3
import sys
import tempfile
import unittest
from contextlib import closing
from pathlib import Path
from unittest import mock


SCRIPTS = Path(__file__).resolve().parent
SCRIPT = SCRIPTS / "smoke-access-browser.py"
INHERITED_REMOTE_ENVIRONMENT = (
    "FASTI_INTEGRATION_LISTEN",
    "FASTI_INTEGRATION_TLS_TERMINATED",
    "FASTI_REMOTE_TRUSTED_PROXY",
    "FASTI_PUBLIC_URL",
    "FASTI_EXTERNAL_BIND_IP",
    "FASTI_BOUND_ADDR_FILE",
    "FASTI_INTEGRATION_BOUND_ADDR_FILE",
)
INHERITED_FIXTURE_ENVIRONMENT = (
    "FASTI_TMDB_SMOKE_RESOLVE",
    "FASTI_TMDB_SMOKE_CA_PEM",
)
RECORD_ID = "rec_0199a8e3a62c70008000000000000001"
OTHER_RECORD_ID = "rec_0199a8e3a62c70008000000000000002"
CREATE_OPERATION = "op_0199a8e3a62c70008000000000000001"
ATTACH_OPERATION = "op_0199a8e3a62c70008000000000000002"
FIRST_RECEIPT = "scr_0199a8e3a62c70008000000000000001"
SECOND_RECEIPT = "scr_0199a8e3a62c70008000000000000002"


def load_harness():
    spec = importlib.util.spec_from_file_location("fasti_smoke_access_browser", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError("ordinary-browser smoke harness could not be loaded")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sqlite_update(database: Path, statement: str, parameters: tuple = ()) -> None:
    with closing(sqlite3.connect(database)) as connection:
        connection.execute(statement, parameters)
        connection.commit()


class FixtureProvider:
    def child_environment(self) -> dict[str, str]:
        return {
            "TMDB_API_READ_ACCESS_TOKEN": "fixture-header-only-token",
            "FASTI_TMDB_SMOKE_RESOLVE": "127.0.0.1:443",
            "FASTI_TMDB_SMOKE_CA_PEM": "fixture-ca",
        }


class SmokeAccessBrowserStartupTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.harness = load_harness()
        cls.original_start = cls.harness.runtime.start_managed_process_group
        cls.original_stop = cls.harness.runtime.stop_managed_process_group

    def start_harmless_process(self, captured):
        def start(_command, *, environment, stdout, stderr, text=False):
            captured["environment"] = dict(environment)
            process = type(self).original_start(
                [sys.executable, "-c", "import time; time.sleep(60)"],
                environment=environment,
                stdout=stdout,
                stderr=stderr,
                text=text,
            )
            captured["process"] = process
            self.addCleanup(self.stop_if_running, process)
            return process

        return start

    def stop_if_running(self, process) -> None:
        if process.poll() is None:
            type(self).original_stop(process)

    def assert_process_group_absent(self, process) -> None:
        self.assertIsNotNone(process.poll())
        with self.assertRaises(ProcessLookupError):
            os.killpg(process.pid, 0)

    def inherited_environment(self) -> dict[str, str]:
        return {
            name: "inherited-test-value"
            for name in (*INHERITED_REMOTE_ENVIRONMENT, *INHERITED_FIXTURE_ENVIRONMENT)
        }

    def assert_remote_environment_absent(self, environment) -> None:
        for name in INHERITED_REMOTE_ENVIRONMENT:
            self.assertNotIn(name, environment)
        self.assertEqual(environment["FASTI_LISTEN"], "127.0.0.1:8420")
        self.assertEqual(environment["FASTI_PORT_FALLBACK"], "fail")

    def assert_fixture_environment(self, environment) -> None:
        self.assert_remote_environment_absent(environment)
        self.assertEqual(
            environment["TMDB_API_READ_ACCESS_TOKEN"],
            "fixture-header-only-token",
        )
        self.assertEqual(environment["FASTI_TMDB_SMOKE_RESOLVE"], "127.0.0.1:443")
        self.assertEqual(environment["FASTI_TMDB_SMOKE_CA_PEM"], "fixture-ca")

    def test_fixture_start_removes_inherited_remote_listener_configuration(self) -> None:
        captured = {}
        process = None
        with tempfile.TemporaryDirectory(prefix="fasti-browser-start-test-") as directory:
            root = Path(directory)
            with (
                mock.patch.dict(os.environ, self.inherited_environment(), clear=False),
                mock.patch.object(
                    self.harness.runtime,
                    "start_managed_process_group",
                    side_effect=self.start_harmless_process(captured),
                ),
                mock.patch.object(self.harness, "_wait_health", return_value=None),
            ):
                process = self.harness._start_fastid(
                    root / "data", root / "trailbase", FixtureProvider()
                )
        try:
            self.assertIs(process, captured["process"])
            self.assertIsNone(process.poll())
            self.assert_fixture_environment(captured["environment"])
        finally:
            if process is not None:
                type(self).original_stop(process)
        self.assert_process_group_absent(process)

    def test_default_start_removes_inherited_fixture_configuration(self) -> None:
        captured = {}
        process = None
        with tempfile.TemporaryDirectory(prefix="fasti-browser-default-test-") as directory:
            root = Path(directory)
            with (
                mock.patch.dict(os.environ, self.inherited_environment(), clear=False),
                mock.patch.object(
                    self.harness.runtime,
                    "start_managed_process_group",
                    side_effect=self.start_harmless_process(captured),
                ),
                mock.patch.object(self.harness, "_wait_health", return_value=None),
            ):
                process = self.harness._start_fastid(
                    root / "data", root / "trailbase"
                )
        try:
            self.assert_remote_environment_absent(captured["environment"])
            for name in INHERITED_FIXTURE_ENVIRONMENT:
                self.assertNotIn(name, captured["environment"])
        finally:
            if process is not None:
                type(self).original_stop(process)
        self.assert_process_group_absent(process)

    def test_health_failure_stops_managed_process_and_rethrows(self) -> None:
        captured = {}
        health_failure = RuntimeError("focused health failure")
        with tempfile.TemporaryDirectory(prefix="fasti-browser-health-test-") as directory:
            root = Path(directory)
            with (
                mock.patch.dict(os.environ, self.inherited_environment(), clear=False),
                mock.patch.object(
                    self.harness.runtime,
                    "start_managed_process_group",
                    side_effect=self.start_harmless_process(captured),
                ),
                mock.patch.object(
                    self.harness.runtime,
                    "stop_managed_process_group",
                    wraps=type(self).original_stop,
                ) as stop,
                mock.patch.object(
                    self.harness, "_wait_health", side_effect=health_failure
                ),
            ):
                with self.assertRaises(RuntimeError) as raised:
                    self.harness._start_fastid(
                        root / "data", root / "trailbase", FixtureProvider()
                    )

        self.assertIs(raised.exception, health_failure)
        stop.assert_called_once_with(captured["process"])
        self.assert_process_group_absent(captured["process"])
        self.assert_fixture_environment(captured["environment"])


class SearchDatabaseEvidenceTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.harness = load_harness()

    @staticmethod
    def action_receipt(
        operation_id: str,
        candidate_receipt_id: str,
        kind: str,
        disposition: str,
    ) -> dict[str, object]:
        action: dict[str, str] = {"kind": kind}
        if kind == "attach":
            action["record_id"] = RECORD_ID
        return {
            "operation_id": operation_id,
            "candidate_receipt_id": candidate_receipt_id,
            "provider": "tmdb",
            "grain": "film",
            "record_id": RECORD_ID,
            "evidence_mode": "refetch",
            "disposition": disposition,
            "actor_subject_id": "sub_0199a8e3a62c70008000000000000001",
            "actor_client_id": "cli_0199a8e3a62c70008000000000000001",
            "action": action,
        }

    def create_database(self, database: Path) -> None:
        with closing(sqlite3.connect(database)) as connection:
            connection.executescript(
                """
                CREATE TABLE records(record_id TEXT, grain TEXT, status TEXT);
                CREATE TABLE external_identifiers(
                    record_id TEXT, namespace TEXT, grain TEXT, value TEXT
                );
                CREATE TABLE search_action_receipts(
                    record_id TEXT, operation_id TEXT, receipt_json TEXT
                );
                CREATE TABLE search_pages(sequence INTEGER, provider_id TEXT);
                CREATE TABLE search_candidate_receipts(
                    candidate_receipt_id TEXT, provider_record_id TEXT,
                    page_sequence INTEGER, kind TEXT, candidate_json TEXT
                );
                CREATE TABLE metadata_claim_provenance(
                    record_id TEXT, provider_id TEXT, provenance_state TEXT,
                    evidence_digest TEXT, source_record_id TEXT
                );
                """
            )
            connection.execute(
                "INSERT INTO records VALUES (?, 'film', 'active')", (RECORD_ID,)
            )
            connection.executemany(
                "INSERT INTO external_identifiers VALUES (?, 'tmdb.movie', 'film', ?)",
                [(RECORD_ID, "842001"), (RECORD_ID, "842002")],
            )
            connection.executemany(
                "INSERT INTO search_pages VALUES (?, ?)", [(1, "tmdb")]
            )
            connection.executemany(
                "INSERT INTO search_candidate_receipts VALUES (?, ?, 1, 'movie', ?)",
                [
                    (
                        FIRST_RECEIPT,
                        "842001",
                        json.dumps(
                            {
                                "provider": "tmdb",
                                "kind": "movie",
                                "provider_id": "842001",
                            }
                        ),
                    ),
                    (
                        SECOND_RECEIPT,
                        "842002",
                        json.dumps(
                            {
                                "provider": "tmdb",
                                "kind": "movie",
                                "provider_id": "842002",
                            }
                        ),
                    ),
                ],
            )
            connection.executemany(
                "INSERT INTO metadata_claim_provenance "
                "VALUES (?, 'tmdb', 'complete', 'sha256:fixture', ?)",
                [(RECORD_ID, "842001"), (RECORD_ID, "842002")],
            )
            receipts = [
                self.action_receipt(
                    CREATE_OPERATION, FIRST_RECEIPT, "create", "created"
                ),
                self.action_receipt(
                    ATTACH_OPERATION, SECOND_RECEIPT, "attach", "attached"
                ),
            ]
            connection.executemany(
                "INSERT INTO search_action_receipts VALUES (?, ?, ?)",
                [
                    (RECORD_ID, receipt["operation_id"], json.dumps(receipt))
                    for receipt in receipts
                ],
            )
            connection.commit()

    @staticmethod
    def mutate_receipt(database: Path, operation_id: str, mutation) -> None:
        with closing(sqlite3.connect(database)) as connection:
            encoded = connection.execute(
                "SELECT receipt_json FROM search_action_receipts WHERE operation_id = ?",
                (operation_id,),
            ).fetchone()[0]
            receipt = json.loads(encoded)
            mutation(receipt)
            connection.execute(
                "UPDATE search_action_receipts SET receipt_json = ? WHERE operation_id = ?",
                (json.dumps(receipt), operation_id),
            )
            connection.commit()

    def test_accepts_exact_create_and_attach_evidence(self) -> None:
        with tempfile.TemporaryDirectory(prefix="fasti-search-oracle-valid-") as directory:
            database = Path(directory) / "fasti.sqlite3"
            self.create_database(database)
            evidence = self.harness._search_database_evidence(database, RECORD_ID)
        self.assertEqual(evidence["recordId"], RECORD_ID)
        self.assertEqual(evidence["candidateCount"], 2)
        self.assertEqual(evidence["completeProvenanceSourceIds"], ["842001", "842002"])

    def test_rejects_action_semantic_drift(self) -> None:
        def swap_candidate_receipts(database: Path) -> None:
            with closing(sqlite3.connect(database)) as connection:
                rows = connection.execute(
                    "SELECT operation_id, receipt_json FROM search_action_receipts"
                ).fetchall()
                receipts = {operation: json.loads(value) for operation, value in rows}
                receipts[CREATE_OPERATION]["candidate_receipt_id"] = SECOND_RECEIPT
                receipts[ATTACH_OPERATION]["candidate_receipt_id"] = FIRST_RECEIPT
                connection.executemany(
                    "UPDATE search_action_receipts SET receipt_json = ? "
                    "WHERE operation_id = ?",
                    [
                        (json.dumps(receipt), operation)
                        for operation, receipt in receipts.items()
                    ],
                )
                connection.commit()

        cases = [
            ("swapped candidate receipts", swap_candidate_receipts),
            (
                "provider",
                lambda database: self.mutate_receipt(
                    database,
                    CREATE_OPERATION,
                    lambda receipt: receipt.__setitem__("provider", "google-books"),
                ),
            ),
            (
                "grain",
                lambda database: self.mutate_receipt(
                    database,
                    CREATE_OPERATION,
                    lambda receipt: receipt.__setitem__("grain", "series"),
                ),
            ),
            (
                "record",
                lambda database: self.mutate_receipt(
                    database,
                    CREATE_OPERATION,
                    lambda receipt: receipt.__setitem__("record_id", OTHER_RECORD_ID),
                ),
            ),
            (
                "evidence mode",
                lambda database: self.mutate_receipt(
                    database,
                    CREATE_OPERATION,
                    lambda receipt: receipt.__setitem__("evidence_mode", "cached"),
                ),
            ),
            (
                "create disposition",
                lambda database: self.mutate_receipt(
                    database,
                    CREATE_OPERATION,
                    lambda receipt: receipt.__setitem__("disposition", "reused"),
                ),
            ),
            (
                "attach disposition",
                lambda database: self.mutate_receipt(
                    database,
                    ATTACH_OPERATION,
                    lambda receipt: receipt.__setitem__(
                        "disposition", "already_attached"
                    ),
                ),
            ),
        ]
        for label, mutation in cases:
            with self.subTest(label=label):
                with tempfile.TemporaryDirectory(
                    prefix="fasti-search-oracle-invalid-"
                ) as directory:
                    database = Path(directory) / "fasti.sqlite3"
                    self.create_database(database)
                    mutation(database)
                    with self.assertRaises(RuntimeError):
                        self.harness._search_database_evidence(database, RECORD_ID)

    def test_rejects_candidate_ownership_and_extra_actions(self) -> None:
        def extra_action(database: Path) -> None:
            receipt = self.action_receipt(
                "op_0199a8e3a62c70008000000000000003",
                SECOND_RECEIPT,
                "attach",
                "attached",
            )
            with closing(sqlite3.connect(database)) as connection:
                connection.execute(
                    "INSERT INTO search_action_receipts VALUES (?, ?, ?)",
                    (RECORD_ID, receipt["operation_id"], json.dumps(receipt)),
                )
                connection.commit()

        cases = [
            (
                "candidate page provider",
                lambda database: sqlite_update(
                    database,
                    "UPDATE search_pages SET provider_id = 'google-books'",
                ),
            ),
            (
                "candidate kind",
                lambda database: sqlite_update(
                    database,
                    "UPDATE search_candidate_receipts SET kind = 'tv' "
                    "WHERE candidate_receipt_id = ?",
                    (FIRST_RECEIPT,),
                ),
            ),
            ("extra action", extra_action),
        ]
        for label, mutation in cases:
            with self.subTest(label=label):
                with tempfile.TemporaryDirectory(
                    prefix="fasti-search-owner-invalid-"
                ) as directory:
                    database = Path(directory) / "fasti.sqlite3"
                    self.create_database(database)
                    mutation(database)
                    with self.assertRaises(RuntimeError):
                        self.harness._search_database_evidence(database, RECORD_ID)


if __name__ == "__main__":
    unittest.main()
