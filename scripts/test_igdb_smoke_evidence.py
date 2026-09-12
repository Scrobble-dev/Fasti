#!/usr/bin/env python3
"""Minimal SQLite verifier tests, not evidence of a real fastid journey."""

from __future__ import annotations

import hashlib
from contextlib import closing
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest

from igdb_smoke_evidence import FIELDS, IDS, assert_secrets_absent, search_database_evidence
from igdb_smoke_fixture import API_HOST, TITLE


SCHEMA = """
CREATE TABLE records(record_id TEXT,workspace_id TEXT,grain TEXT,status TEXT);
CREATE TABLE external_identifiers(workspace_id TEXT,record_id TEXT,namespace TEXT,grain TEXT,value TEXT);
CREATE TABLE search_pages(sequence INTEGER,workspace_id TEXT,profile_id TEXT,actor_client_id TEXT,
 actor_subject_id TEXT,grant_id TEXT,provider_id TEXT,upstream_page INTEGER,next_page INTEGER,
 candidate_count INTEGER,candidate_bytes INTEGER,response_digest TEXT,context_json TEXT,
 partition_json TEXT,created_at TEXT,fresh_until TEXT,stale_until TEXT,expires_at TEXT);
CREATE TABLE search_candidate_receipts(candidate_receipt_id TEXT,page_sequence INTEGER,ordinal INTEGER,
 kind TEXT,provider_record_id TEXT,candidate_json TEXT);
CREATE TABLE search_action_receipts(workspace_id TEXT,profile_id TEXT,actor_client_id TEXT,
 actor_subject_id TEXT,operation_id TEXT,record_id TEXT,receipt_json TEXT);
CREATE TABLE metadata_field_claims(workspace_id TEXT,record_id TEXT,field_key TEXT,source TEXT,value TEXT,
 locale TEXT,fetched_at TEXT,expires_at TEXT);
CREATE TABLE metadata_claim_provenance(claim_id TEXT,workspace_id TEXT,record_id TEXT,field_key TEXT,
 source TEXT,fetched_at TEXT,provider_id TEXT,source_record_id TEXT,evidence_digest TEXT,
 provenance_state TEXT,initial_status TEXT,region TEXT,source_version TEXT);
CREATE TABLE metadata_claims(claim_id TEXT,workspace_id TEXT,record_id TEXT,claim_kind TEXT,response_policy_json TEXT);
"""
AT = "2026-09-08T12:00:00.000000Z"
UNTIL = "2026-09-08T12:05:00.000000Z"
LATER = "2026-09-08T12:10:00.000000Z"
POLICY = {"reuse": "reusable", "received_at": AT,
          "corrected_initial_age": {"secs": 0, "nanos": 0},
          "source_freshness": {"secs": 300, "nanos": 0},
          "source_stale_if_error": {"secs": 300, "nanos": 0}}
SCOPE = {"workspace_id": "workspace", "profile_id": "profile",
         "actor_client_id": "client", "actor_subject_id": "subject"}
DIGEST = "sha256:" + "a" * 64
CONTEXT_DIGEST = "sha256:" + "b" * 64


def fixture(database):
    events = [{"origin": API_HOST, "operation": "search", "response_body_sha256": "a" * 64}]
    with closing(sqlite3.connect(database)) as connection, connection:
        connection.executescript(SCHEMA)
        connection.execute("INSERT INTO records VALUES ('record','workspace','game_release','active')")
        candidate_bytes = 0
        for ordinal, source in enumerate(IDS):
            digest = "sha256:" + str(ordinal + 1) * 64
            events.append({"origin": API_HOST, "operation": "detail", "provider_record_id": source,
                           "response_body_sha256": digest[7:]})
            connection.execute("INSERT INTO external_identifiers VALUES (?,?,?,?,?)",
                               ("workspace", "record", "igdb.game", "game_release", source))
            candidate = json.dumps({"provider": "igdb", "provider_id": source, "kind": "game",
                                    "title": TITLE, "release_year": 2020,
                                    "overview": FIELDS["core.overview"]})
            candidate_bytes += len(candidate.encode())
            connection.execute("INSERT INTO search_candidate_receipts VALUES (?,1,?,'game',?,?)",
                               (f"candidate{ordinal}", ordinal, source, candidate))
            action = {"kind": "create"} if ordinal == 0 else {"kind": "attach", "record_id": "record"}
            provenance = {"provider_id": "igdb", "source_namespace": "igdb.game",
                          "source_identifier": source, "evidence_digest": digest,
                          "locale": None, "region": None, "source_version": None}
            receipt = {**SCOPE, "operation_id": f"operation{ordinal}",
                       "candidate_receipt_id": f"candidate{ordinal}", "provider": "igdb",
                       "grain": "game_release", "action": action, "evidence_mode": "refetch",
                       "record_id": "record", "disposition": "created" if ordinal == 0 else "attached",
                       "search_context_digest": CONTEXT_DIGEST, "search_response_digest": DIGEST,
                       "provenance": provenance, "fetched_at": AT, "expires_at": UNTIL,
                       "initial_status": "fresh", "committed_at": AT}
            connection.execute("INSERT INTO search_action_receipts VALUES (?,?,?,?,?,?,?)",
                               (*SCOPE.values(), f"operation{ordinal}", "record", json.dumps(receipt)))
            for field, value in FIELDS.items():
                claim_id = source + field
                connection.execute("INSERT INTO metadata_field_claims VALUES (?,?,?,?,?,?,?,?)",
                                   ("workspace", "record", field, "igdb.game", value, None,
                                    AT if ordinal == 0 else "2026-09-08T12:00:01.000000Z", UNTIL))
                # Distinct observations are required by the real field primary key.
                fetched = AT if ordinal == 0 else "2026-09-08T12:00:01.000000Z"
                policy = {**POLICY, "received_at": fetched}
                connection.execute("INSERT INTO metadata_claim_provenance VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
                                   (claim_id, "workspace", "record", field, "igdb.game", fetched,
                                    "igdb", source, digest, "complete", "fresh", None, None))
                connection.execute("INSERT INTO metadata_claims VALUES (?,?,?,?,?)",
                                   (claim_id, "workspace", "record", "field", json.dumps(policy)))
            if ordinal:
                receipt["fetched_at"] = "2026-09-08T12:00:01.000000Z"
                connection.execute("UPDATE search_action_receipts SET receipt_json=? WHERE operation_id=?",
                                   (json.dumps(receipt), f"operation{ordinal}"))
        context = {"provider": "igdb", "page": 1, "response_policy": POLICY}
        partition = {**SCOPE, "grant_id": "grant", "context_digest": CONTEXT_DIGEST}
        connection.execute("INSERT INTO search_pages VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                           (1, *SCOPE.values(), "grant", "igdb", 1, None, 2, candidate_bytes,
                            DIGEST, json.dumps(context), json.dumps(partition), AT, UNTIL, LATER, LATER))
    return events


class IgdbSmokeEvidenceTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fasti-igdb-evidence-test-")
        self.database = Path(self.temporary.name) / "fixture.sqlite3"
        self.events = fixture(self.database)

    def tearDown(self):
        self.temporary.cleanup()

    def test_complete_evidence_is_stable_and_read_only(self):
        before = hashlib.sha256(self.database.read_bytes()).digest()
        proof = search_database_evidence(self.database, "record", self.events)
        self.assertEqual(proof, search_database_evidence(self.database, "record", self.events))
        self.assertEqual(hashlib.sha256(self.database.read_bytes()).digest(), before)
        self.assertEqual(proof["pageCount"], 1)
        self.assertEqual(proof["candidateCount"], 2)
        self.assertEqual(len(proof["fieldCustody"]), 6)
        assert_secrets_absent(self.database, [b"synthetic-token-not-present"])

    def test_missing_extra_or_crossed_rows_fail_closed(self):
        changes = [
            "DELETE FROM metadata_claims WHERE claim_id=(SELECT MIN(claim_id) FROM metadata_claims)",
            "UPDATE metadata_claim_provenance SET source_record_id='wrong'",
            "UPDATE metadata_claim_provenance SET workspace_id='other'",
            "UPDATE metadata_claims SET response_policy_json=NULL",
            "UPDATE metadata_claims SET response_policy_json=json_set(response_policy_json,'$.reuse','no_store')",
            "UPDATE metadata_claim_provenance SET evidence_digest='sha256:' || printf('%064d',0)",
            "UPDATE metadata_field_claims SET value='wrong' WHERE field_key='core.title'",
            "UPDATE search_candidate_receipts SET page_sequence=99",
            "UPDATE search_pages SET candidate_bytes=0",
            "UPDATE search_action_receipts SET receipt_json=json_set(receipt_json,'$.evidence_mode','cached')",
            "UPDATE search_action_receipts SET receipt_json=json_set(receipt_json,'$.provenance.source_identifier','wrong')",
            "UPDATE search_action_receipts SET receipt_json=json_set(receipt_json,'$.actor_subject_id','other')",
            "INSERT INTO records VALUES ('extra','workspace','game_release','active')",
            "UPDATE external_identifiers SET record_id='other'",
            "UPDATE search_candidate_receipts SET candidate_json=json_set(candidate_json,'$.access_token','sentinel')",
        ]
        for ordinal, change in enumerate(changes):
            with self.subTest(change=ordinal):
                database = Path(self.temporary.name) / f"negative-{ordinal}.sqlite3"
                events = fixture(database)
                with closing(sqlite3.connect(database)) as connection, connection:
                    connection.execute(change)
                with self.assertRaisesRegex(RuntimeError, "IGDB durable Search evidence differs"):
                    search_database_evidence(database, "record", events)

    def test_source_event_join_cannot_swap_or_omit_detail_id(self):
        for events in [self.events[:2],
                       [self.events[0], {**self.events[1], "provider_record_id": IDS[1]}, self.events[2]],
                       [{**self.events[0], "response_body_sha256": "c" * 64}, *self.events[1:]]]:
            with self.assertRaises(RuntimeError):
                search_database_evidence(self.database, "record", events)

    def test_secret_scan_covers_chunk_boundary_and_sidecars_without_disclosure(self):
        sentinel = b"synthetic-private-token-sentinel"
        sidecar = Path(f"{self.database}-wal")
        # Test-owned diagnostic file, not a live SQLite WAL.
        sidecar.write_bytes(b"x" * (65536 - 7) + sentinel)
        with self.assertRaises(RuntimeError) as caught:
            assert_secrets_absent(self.database, [sentinel])
        self.assertNotIn(sentinel.decode(), str(caught.exception))
        with self.assertRaises(RuntimeError):
            assert_secrets_absent(self.database, [])


if __name__ == "__main__":
    unittest.main()
