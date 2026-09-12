"""Read-only evidence for the fixed two-game IGDB real-process smoke journey.

The caller must stop fastid before reading, and supply actual fixture events.
This verifier does not implement domain admission or authorize stored receipts.
"""

from __future__ import annotations

from contextlib import closing
from datetime import datetime
import json
from pathlib import Path
import re
import sqlite3

from igdb_smoke_fixture import API_HOST, PROVIDER_IDS, TITLE


ERROR = "IGDB durable Search evidence differs"
IDS = tuple(str(value) for value in PROVIDER_IDS)
FIELDS = {"core.title": TITLE, "core.release_year": "2020",
          "core.overview": "Deterministic provider detail for the real Search journey."}
SCOPE = ("workspace_id", "profile_id", "actor_client_id", "actor_subject_id")


def _require(condition):
    if not condition:
        raise RuntimeError(ERROR)


def _json(text):
    def safe(value):
        if isinstance(value, dict):
            _require(not {"access_token", "client_secret", "authorization", "grant_type"}
                     .intersection(key.lower() for key in value))
            for child in value.values():
                safe(child)
        elif isinstance(value, list):
            for child in value:
                safe(child)
    _require(isinstance(text, str) and len(text.encode()) <= 65536)
    value = json.loads(text)
    _require(isinstance(value, dict))
    safe(value)
    return value


def _time(value):
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    _require(parsed.tzinfo is not None)
    return parsed  # Store timestamps preserve microseconds, not nanoseconds.


def _policy(policy, fetched_at):
    _require(set(policy) == {"reuse", "received_at", "corrected_initial_age",
                             "source_freshness", "source_stale_if_error"})
    _require(policy["reuse"] == "reusable")
    _require(_time(policy["received_at"]) == _time(fetched_at))
    age = policy["corrected_initial_age"]
    _require(set(age) == {"secs", "nanos"} and type(age["secs"]) is int
             and type(age["nanos"]) is int and 0 <= age["secs"] < 300
             and 0 <= age["nanos"] < 1_000_000_000)
    for key in ("source_freshness", "source_stale_if_error"):
        _require(policy[key] == {"secs": 300, "nanos": 0})


def _rows(connection, query, limit):
    rows = connection.execute(query).fetchmany(limit + 1)
    _require(len(rows) <= limit)
    return rows


def search_database_evidence(database: Path, record_id: str, events) -> dict[str, object]:
    """Verify one page, two receipts and six independently joined field claims."""
    try:
        return _search_database_evidence(Path(database), record_id, events)
    except (KeyError, TypeError, ValueError, AttributeError, RecursionError, sqlite3.Error):
        raise RuntimeError(ERROR) from None


def _search_database_evidence(database, record_id, events):
    _require(isinstance(events, list) and len(events) <= 128)
    search_digests, detail_digests = set(), {value: set() for value in IDS}
    for event in events:
        if event.get("operation") not in ("search", "detail"):
            continue
        _require(event.get("origin") == API_HOST)
        digest = event.get("response_body_sha256")
        _require(isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest))
        if event["operation"] == "search":
            search_digests.add("sha256:" + digest)
        else:
            _require(event.get("provider_record_id") in IDS)
            detail_digests[event["provider_record_id"]].add("sha256:" + digest)
    _require(len(search_digests) == 1 and all(len(values) == 1 for values in detail_digests.values()))
    with closing(sqlite3.connect(database.resolve().as_uri() + "?mode=ro", uri=True)) as connection:
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA query_only=ON")
        connection.execute("BEGIN")
        records = _rows(connection, "SELECT record_id,workspace_id FROM records WHERE grain='game_release' AND status='active'", 1)
        _require(len(records) == 1 and records[0]["record_id"] == record_id)
        workspace = records[0]["workspace_id"]
        identifiers = _rows(connection, "SELECT workspace_id,record_id,namespace,grain,value FROM external_identifiers ORDER BY value", 2)
        _require([tuple(row) for row in identifiers] == [
            (workspace, record_id, "igdb.game", "game_release", value) for value in IDS])
        pages = _rows(connection, "SELECT * FROM search_pages ORDER BY sequence", 1)
        candidates = _rows(connection, "SELECT * FROM search_candidate_receipts ORDER BY ordinal", 2)
        actions = _rows(connection, "SELECT * FROM search_action_receipts ORDER BY operation_id", 2)
        _require(len(pages) == 1 and len(candidates) == len(actions) == 2)
        page = pages[0]
        context, partition = _json(page["context_json"]), _json(page["partition_json"])
        _require(page["provider_id"] == context["provider"] == "igdb"
                 and page["upstream_page"] == context["page"] == 1
                 and page["next_page"] is None and page["candidate_count"] == 2
                 and page["workspace_id"] == workspace
                 and page["response_digest"] in search_digests)
        for key in SCOPE:
            _require(page[key] and partition[key] == page[key])
        _require(partition["grant_id"] == page["grant_id"] and page["grant_id"])
        _policy(context["response_policy"], page["created_at"])
        _require(_time(page["created_at"]) < _time(page["fresh_until"])
                 <= _time(page["stale_until"]) <= _time(page["expires_at"]))
        _require(page["candidate_bytes"] == sum(len(row["candidate_json"].encode()) for row in candidates))
        candidate_ids = {}
        for ordinal, row in enumerate(candidates):
            candidate = _json(row["candidate_json"])
            _require(row["page_sequence"] == page["sequence"] and row["ordinal"] == ordinal
                     and row["kind"] == candidate["kind"] == "game"
                     and row["provider_record_id"] == candidate["provider_id"] == IDS[ordinal]
                     and candidate["provider"] == "igdb" and candidate["title"] == TITLE
                     and candidate["release_year"] == 2020
                     and candidate["overview"] == FIELDS["core.overview"])
            _require(row["candidate_receipt_id"] not in candidate_ids)
            candidate_ids[row["candidate_receipt_id"]] = IDS[ordinal]
        receipts, by_source = [], {}
        for row in actions:
            receipt = _json(row["receipt_json"])
            for key in (*SCOPE, "operation_id", "record_id"):
                _require(receipt[key] == row[key])
            for key in SCOPE:
                _require(receipt[key] == page[key])
            action = receipt["action"]
            _require(action["kind"] in ("create", "attach"))
            source_id = IDS[0 if action["kind"] == "create" else 1]
            _require(candidate_ids.get(receipt["candidate_receipt_id"]) == source_id
                     and receipt["provider"] == "igdb" and receipt["grain"] == "game_release"
                     and receipt["record_id"] == record_id and receipt["evidence_mode"] == "refetch"
                     and receipt["disposition"] == ("created" if action["kind"] == "create" else "attached")
                     and receipt["search_response_digest"] == page["response_digest"]
                     and receipt["search_context_digest"] == partition["context_digest"]
                     and receipt["initial_status"] == "fresh")
            _require(action == ({"kind": "create"} if source_id == IDS[0]
                                else {"kind": "attach", "record_id": record_id}))
            provenance = receipt["provenance"]
            _require(provenance["provider_id"] == "igdb"
                     and provenance["source_namespace"] == "igdb.game"
                     and provenance["source_identifier"] == source_id
                     and provenance["evidence_digest"] in detail_digests[source_id]
                     and source_id not in by_source)
            by_source[source_id] = receipt
            receipts.append(receipt)
        _require(len({row["operation_id"] for row in receipts}) == 2)
        # LEFT JOINs preserve missing custody rows so omissions cannot pass.
        claims = _rows(connection, """
            SELECT f.*, p.claim_id, p.workspace_id AS provenance_workspace,
              p.record_id AS provenance_record, p.provider_id, p.source_record_id,
              p.evidence_digest, p.provenance_state, p.initial_status, p.region,
              p.source_version, m.workspace_id AS registered_workspace,
              m.record_id AS registered_record, m.claim_kind, m.response_policy_json
            FROM metadata_field_claims f
            LEFT JOIN metadata_claim_provenance p ON p.record_id=f.record_id
              AND p.field_key=f.field_key AND p.source=f.source AND p.fetched_at=f.fetched_at
            LEFT JOIN metadata_claims m ON m.claim_id=p.claim_id
            ORDER BY p.source_record_id,f.field_key
        """, 6)
        _require(len(claims) == 6)
        for table in ("metadata_claim_provenance", "metadata_claims"):
            _require(connection.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0] == 6)
        seen, source_policies = set(), {}
        custody = []
        for claim in claims:
            source_id, key = claim["source_record_id"], claim["field_key"]
            _require(source_id in by_source and key in FIELDS and (source_id, key) not in seen)
            seen.add((source_id, key))
            receipt = by_source[source_id]
            _require(claim["workspace_id"] == claim["provenance_workspace"] == claim["registered_workspace"] == workspace
                     and claim["record_id"] == claim["provenance_record"] == claim["registered_record"] == record_id
                     and claim["claim_id"] and claim["claim_kind"] == "field"
                     and claim["source"] == "igdb.game" and claim["provider_id"] == "igdb"
                     and claim["value"] == FIELDS[key] and claim["provenance_state"] == "complete"
                     and claim["initial_status"] == receipt["initial_status"]
                     and claim["evidence_digest"] == receipt["provenance"]["evidence_digest"])
            for key_name in ("locale", "region", "source_version"):
                _require(claim[key_name] == receipt["provenance"][key_name])
            for time_key in ("fetched_at", "expires_at"):
                _require(_time(claim[time_key]) == _time(receipt[time_key]))
            policy = _json(claim["response_policy_json"])
            _policy(policy, claim["fetched_at"])
            _require(source_policies.setdefault(source_id, policy) == policy)
            _require(0 < (_time(claim["expires_at"]) - _time(claim["fetched_at"])).total_seconds() <= 300)
            custody.append({"claimId": claim["claim_id"], "sourceId": source_id,
                            "fieldKey": key, "digest": claim["evidence_digest"], "policy": policy})
    return {"recordId": record_id, "identifiers": [tuple(row)[2:] for row in identifiers],
            "pageCount": 1, "candidateCount": 2, "actions": receipts,
            "completeProvenanceSourceIds": list(IDS), "fieldCustody": custody,
            "searchResponseDigest": page["response_digest"]}


def assert_secrets_absent(database: Path, secrets) -> None:
    """Scan stopped SQLite/sidecar bytes, including unused pages, with bounded memory."""
    values = tuple(secrets)
    _require(1 <= len(values) <= 8 and all(isinstance(value, bytes) and 8 <= len(value) <= 4096
                                         for value in values))
    overlap = max(map(len, values)) - 1
    for path in (Path(database), Path(f"{database}-wal"), Path(f"{database}-journal")):
        if not path.exists():
            _require(path != Path(database))
            continue
        _require(path.stat().st_size <= 128 * 1024 * 1024)
        with path.open("rb") as stream:
            tail = b""
            while block := stream.read(65536):
                data = tail + block
                if any(value in data for value in values):
                    raise RuntimeError("Synthetic IGDB credential material reached durable storage")
                tail = data[-overlap:]
