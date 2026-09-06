#!/usr/bin/env python3
"""Prove the ordinary-browser TrailBase-to-Fasti session boundary."""

from __future__ import annotations

import argparse
from collections import Counter
from contextlib import closing
import hashlib
import http.server
import importlib.util
import json
import os
import pty
import queue
import re
import select
import signal
import sqlite3
import subprocess  # nosec B404 -- this gate launches exact local test artifacts.
import tempfile
import threading
import time
import urllib.request
from pathlib import Path
from typing import Any, Callable

import trailbase_runtime as runtime
from tmdb_smoke_fixture import (
    NO_STORE_OVERVIEW,
    NO_STORE_PROVIDER_IDS,
    TmdbSmokeFixture,
    PROVIDER_IDS,
)


ROOT = Path(__file__).resolve().parents[1]
FASTI_ORIGIN = "http://127.0.0.1:8420"


def _load(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"required smoke helper could not be loaded: {path.name}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


smoke = _load("fasti_smoke_trailbase", ROOT / "scripts/smoke-trailbase.py")
fixture = _load(
    "fasti_desktop_fixture", ROOT / "scripts/smoke-desktop-access-webdriver.py"
)


def _post(path: str, payload: dict[str, object] | None, bearer: str | None = None):
    headers = {"content-type": "application/json"} if payload is not None else {}
    if bearer:
        headers["authorization"] = f"Bearer {bearer}"
    request = urllib.request.Request(
        FASTI_ORIGIN + path,
        json.dumps(payload).encode() if payload is not None else None,
        headers,
        method="POST",
    )
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(
        request, timeout=5
    ) as response:
        return response.status, json.load(response)


def _wait_health(process: subprocess.Popen[bytes]) -> None:
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    for _ in range(100):
        if process.poll() is not None:
            raise RuntimeError("fastid exited before health")
        try:
            with opener.open(f"{FASTI_ORIGIN}/api/v1/health", timeout=1) as response:
                if response.status == 200:
                    return
        except OSError:
            time.sleep(0.1)
    raise RuntimeError("fastid health timed out")


def _start_fastid(
    data_root: Path, trailbase_root: Path, provider: TmdbSmokeFixture | None = None,
) -> subprocess.Popen[bytes]:
    environment = dict(os.environ)
    for variable in (
        "FASTI_INTEGRATION_LISTEN", "FASTI_INTEGRATION_TLS_TERMINATED",
        "FASTI_REMOTE_TRUSTED_PROXY", "FASTI_PUBLIC_URL", "FASTI_EXTERNAL_BIND_IP",
        "FASTI_BOUND_ADDR_FILE", "FASTI_INTEGRATION_BOUND_ADDR_FILE",
        "FASTI_TMDB_SMOKE_RESOLVE", "FASTI_TMDB_SMOKE_CA_PEM",
    ):
        environment.pop(variable, None)
    environment.update(
        FASTI_LISTEN="127.0.0.1:8420",
        FASTI_PORT_FALLBACK="fail",
        FASTI_DATA_ROOT=str(data_root),
        FASTI_TRAILBASE_ROOT=str(trailbase_root),
        FASTI_STATIC_DIR=str(ROOT / "apps/web/dist"),
    )
    executable = ROOT / "target/debug/fastid"
    if provider is not None:
        environment.pop("GOOGLE_BOOKS_API_KEY", None)
        environment.update(provider.child_environment())
        executable = ROOT / "target/tmdb-smoke-fixture/debug/fastid"
    process = runtime.start_managed_process_group(
        [executable],
        environment=environment,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        _wait_health(process)
    except BaseException:
        runtime.stop_managed_process_group(process)
        raise
    return process


def _browser(
    payload: dict[str, object],
    checkpoint: tuple[Path, Path, Callable[[], None]] | None = None,
) -> dict[str, object]:
    timeout = 180 if payload.get("m4SearchJourney") or checkpoint else 60
    if checkpoint is not None:
        ready, continuation, validate = checkpoint
        process = subprocess.Popen(  # nosec B603 -- fixed local script and no shell.
            ["node", ROOT / "scripts/smoke-access-browser.mjs"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            assert process.stdin is not None
            process.stdin.write(json.dumps(payload).encode())
            process.stdin.close()
            deadline = time.monotonic() + timeout
            while not ready.exists():
                if process.poll() is not None:
                    assert process.stderr is not None
                    raise RuntimeError(process.stderr.read().decode(errors="replace")[-4000:])
                if time.monotonic() >= deadline:
                    raise RuntimeError("ordinary-browser durability checkpoint timed out")
                time.sleep(0.05)
            try:
                validate()
            except BaseException:
                continuation.write_text("abort\n", encoding="ascii")
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    pass
                raise
            continuation.write_text("continue\n", encoding="ascii")
            remaining = max(1.0, deadline - time.monotonic())
            process.wait(timeout=remaining)
            assert process.stdout is not None and process.stderr is not None
            output = process.stdout.read()
            error = process.stderr.read()
            if process.returncode != 0:
                raise RuntimeError(error.decode(errors="replace")[-4000:])
            result = json.loads(output)
            if not isinstance(result, dict):
                raise RuntimeError("ordinary-browser helper returned a non-object result")
            return result
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
    completed = subprocess.run(  # nosec B603 -- fixed local script and no shell.
        ["node", ROOT / "scripts/smoke-access-browser.mjs"],
        input=json.dumps(payload).encode(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=timeout,
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stderr.decode(errors="replace")[-4000:])
    result = json.loads(completed.stdout)
    if not isinstance(result, dict):
        raise RuntimeError("ordinary-browser helper returned a non-object result")
    return result


class _Callback(http.server.BaseHTTPRequestHandler):
    seen: queue.Queue[str] = queue.Queue(maxsize=1)

    def do_GET(self) -> None:
        self.seen.put_nowait(FASTI_ORIGIN + self.path)
        self.send_response(200)
        self.end_headers()

    def log_message(self, *_args: object) -> None:
        return


def _execv_or_exit(executable: Path, arguments: list[str]) -> None:
    try:
        os.execv(executable, arguments)
    finally:
        os._exit(127)


def _execv_failure_self_test() -> None:
    pid = os.fork()
    if pid == 0:
        _execv_or_exit(ROOT / "target/fasti-missing", ["fasti-missing"])
    _, status = os.waitpid(pid, 0)
    if os.waitstatus_to_exitcode(status) != 127:
        raise RuntimeError("failed exec did not terminate only its child")


def _bootstrap_cli(
    data_root: Path, trailbase_root: Path, email: str, password: str
) -> None:
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 8420), _Callback)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    pid, terminal = pty.fork()
    if pid == 0:
        _execv_or_exit(
            ROOT / "target/debug/fasti",
            [
                "fasti",
                "access",
                "bootstrap-administrator",
                "--data-root",
                str(data_root),
                "--trailbase-root",
                str(trailbase_root),
            ],
        )
    output = bytearray()
    child_reaped = False
    try:
        deadline = time.monotonic() + 30
        authorization = None
        while time.monotonic() < deadline:
            readable, _, _ = select.select([terminal], [], [], 0.5)
            if not readable:
                continue
            output.extend(os.read(terminal, 4096))
            match = re.search(
                rb"Open this TrailBase authorization URL:\r?\n(https?://[^\r\n]+)",
                output,
            )
            if match:
                authorization = match.group(1).decode()
                break
        if authorization is None:
            raise RuntimeError("bootstrap CLI did not emit its authorization URL")
        result = _browser(
            {
                "mode": "bootstrap",
                "authorizationUrl": authorization,
                "email": email,
                "password": password,
            }
        )
        callback = result["callbackUrl"]
        if _Callback.seen.get(timeout=5) != callback:
            raise RuntimeError("browser callback capture differs")
        os.write(terminal, str(callback).encode() + b"\n")
        deadline = time.monotonic() + 30
        status = None
        while time.monotonic() < deadline:
            try:
                readable, _, _ = select.select([terminal], [], [], 0.5)
                if readable:
                    output.extend(os.read(terminal, 4096))
            except OSError:
                pass
            completed_pid, status = os.waitpid(pid, os.WNOHANG)
            if completed_pid == pid:
                child_reaped = True
                break
        if not child_reaped:
            raise RuntimeError("trusted bootstrap CLI timed out")
        if status != 0 or b"first Fasti administrator is established" not in output:
            raise RuntimeError("trusted bootstrap CLI did not complete")
    finally:
        server.shutdown()
        server.server_close()
        try:
            os.close(terminal)
        except OSError:
            pass
        if not child_reaped:
            try:
                os.kill(pid, signal.SIGTERM)
                os.waitpid(pid, 0)
            except ProcessLookupError:
                pass
        password = ""
        output.clear()


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT / ".dev-trailbase")
    parser.add_argument(
        "--m4-search-journey", action="store_true",
        help="also prove real-process Search/Create/Attach/cache/restart with isolated provider TLS",
    )
    parser.add_argument(
        "--receipt",
        type=Path,
        default=ROOT / "target/fasti-receipts/access-c1-ordinary-browser.json",
    )
    return parser.parse_args()


def main() -> None:
    runtime.install_termination_cleanup()
    arguments = _arguments()
    _execv_failure_self_test()
    if subprocess.check_output(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=ROOT
    ).strip():
        raise RuntimeError("ordinary-browser proof requires a clean tree")
    fixture._require_free_port(4000, "ordinary-browser TrailBase proof")
    fixture._require_free_port(4001, "ordinary-browser TrailBase admin proof")
    fixture._require_free_port(8420, "ordinary-browser Fasti proof")
    if arguments.m4_search_journey:
        for command in [
            ["cargo", "build", "--locked", "--offline", "-p", "fasti-cli"],
            ["cargo", "build", "--locked", "--offline", "-p", "fastid",
             "--features", "tmdb-smoke-fixture", "--target-dir", "target/tmdb-smoke-fixture"],
        ]:
            subprocess.run(command, cwd=ROOT, check=True, timeout=900)

    trailbase_process = None
    fastid = None
    smtp = None
    provider = None
    with tempfile.TemporaryDirectory(
        prefix="fasti-c1-browser-", dir=Path.home()
    ) as directory:
        workspace = Path(directory)
        os.chmod(workspace, 0o700)
        trailbase_root = workspace / "trailbase"
        data_root = workspace / "fasti-data"
        fixture._private_directory(data_root)
        executable = fixture._copy_exact_release_input(
            arguments.root.resolve(), trailbase_root
        )
        fixture._bootstrap_disposable_installation(smoke, executable, trailbase_root)
        smtp = smoke.SmtpServer(("127.0.0.1", 0))
        threading.Thread(target=smtp.serve_forever, daemon=True).start()
        try:
            fixture._write_fixture_config(trailbase_root, int(smtp.server_address[1]))
            runtime.prepare_runtime_lock(trailbase_root)
            runtime.prepare_installation(trailbase_root, "native")
            runtime.verify_installation(trailbase_root)
            trailbase_process, _ = smoke.start_fixture_release(
                executable, trailbase_root, 4000
            )
            email, password = fixture._register_verified_human(smoke, smtp.messages)
            if arguments.m4_search_journey:
                provider = TmdbSmokeFixture(workspace / "tmdb")
            fastid = _start_fastid(data_root, trailbase_root, provider)
            secret = (data_root / "bootstrap.secret").read_text().strip()
            status, initialized = _post("/api/v1/node/initialization", {}, secret)
            if status != 200:
                raise RuntimeError("node initialization failed")
            status, enrolled = _post(
                "/api/v1/client-enrollments",
                {"initialization_proof": initialized["initialization_proof"]},
            )
            if status != 200:
                raise RuntimeError("node enrollment failed")
            secret = ""
            initialized.clear()
            if provider is not None:
                bearer = enrolled["credential"]
                try:
                    for capability in ("metadata.search", "metadata.read"):
                        status, checked = _post(
                            f"/api/v1/providers/tmdb/credentials/{capability}/tests", None, bearer,
                        )
                        selected = next(row for row in checked["capabilities"]
                                        if row["capability_id"] == capability)
                        if (status != 200 or selected["credential_state"] != "valid"
                                or selected["credential_test"]["state"] != "passed"):
                            raise RuntimeError("real provider credential test did not pass")
                finally:
                    bearer = ""
            enrolled.clear()
            runtime.stop_managed_process_group(fastid)
            fastid = None

            _bootstrap_cli(data_root, trailbase_root, email, password)
            fastid = _start_fastid(data_root, trailbase_root, provider)
            evidence = _browser(
                {"mode": "sign-in", "email": email, "password": password,
                 "m4SearchJourney": arguments.m4_search_journey}
            )
            runtime.stop_managed_process_group(fastid)
            fastid = None
            database = data_root / "current/fasti.sqlite3"
            with closing(sqlite3.connect(f"file:{database}?mode=ro", uri=True)) as connection:
                session_count = connection.execute(
                    "SELECT COUNT(*) FROM fasti_browser_sessions WHERE revoked_at IS NULL"
                ).fetchone()[0]
                administrator_count = connection.execute(
                    "SELECT COUNT(*) FROM workspace_memberships "
                    "WHERE lifecycle = 'active' AND role = 'administrator'"
                ).fetchone()[0]
            if session_count != 1:
                raise RuntimeError("ordinary browser did not create one active Fasti session")
            if administrator_count != 1:
                raise RuntimeError("trusted CLI did not create one active administrator")
            if provider is not None:
                journey = evidence["m4SearchJourney"]
                requests = provider.requests()
                if Counter(requests) != Counter({
                    "/3/configuration": 2, "/3/search/multi": 1,
                    **{f"/3/movie/{value}": 2 for value in PROVIDER_IDS},
                }):
                    raise RuntimeError("real provider exchange/cache request evidence differs")
                before = _search_database_evidence(database, journey["recordId"])
                fastid = _start_fastid(data_root, trailbase_root, provider)
                restarted = _browser({
                    "mode": "restart-record", "email": email, "password": password,
                    "recordId": journey["recordId"], "recordPath": journey["recordPath"],
                })
                runtime.stop_managed_process_group(fastid)
                fastid = None
                after = _search_database_evidence(database, journey["recordId"])
                if before != after or provider.requests() != requests:
                    raise RuntimeError("Record restart changed durable Search evidence or queried TMDB")
                evidence["m4SearchRestart"] = restarted
                evidence["m4SearchDatabase"] = after
                evidence["m4ProviderRequests"] = requests
                evidence["m4ProviderInput"] = {
                    "classification": "disposable_loopback_tmdb_tls_fixture",
                    "public_provider_acceptance": False,
                    "build_feature": "tmdb-smoke-fixture",
                    "daemon_sha256": runtime.sha256_file(
                        ROOT / "target/tmdb-smoke-fixture/debug/fastid"
                    ),
                }
                no_store_baseline = _content_database_snapshot(database)
                no_store_requests = provider.requests()
                ready = workspace / "m4-no-store-details-ready.json"
                continuation = workspace / "m4-no-store-continue"
                detail_evidence: dict[str, object] = {}

                def validate_no_store_detail_checkpoint() -> None:
                    staged = json.loads(ready.read_text(encoding="utf-8"))
                    if (staged.get("stage") != "details_only"
                            or staged.get("providerRecordId") !=
                            str(NO_STORE_PROVIDER_IDS[0])
                            or staged.get("route") !=
                            f"/explore/live/tmdb/film/{NO_STORE_PROVIDER_IDS[0]}/candidate"):
                        raise RuntimeError("browser no-store detail checkpoint differs")
                    current = _content_database_snapshot(database)
                    if current != no_store_baseline:
                        raise RuntimeError("no-store Search/details changed durable content")
                    _require_no_store_payload_absent(database)
                    delta = provider.requests()[len(no_store_requests):]
                    if Counter(delta) != Counter({
                        "/3/search/multi": 1,
                        f"/3/movie/{NO_STORE_PROVIDER_IDS[0]}": 2,
                    }):
                        raise RuntimeError("no-store Search/details provider evidence differs")
                    detail_evidence.update({
                        "content_digest": _snapshot_digest(current),
                        "provider_requests": delta,
                        "sqlite_and_wal_payload_absent": True,
                    })

                fastid = _start_fastid(data_root, trailbase_root, provider)
                no_store = _browser(
                    {
                        "mode": "m4-no-store",
                        "email": email,
                        "password": password,
                        "noStoreCheckpoint": {
                            "readyPath": str(ready),
                            "continuePath": str(continuation),
                        },
                        "attachRecordId": journey["recordId"],
                        "attachRecordPath": journey["recordPath"],
                    },
                    (ready, continuation, validate_no_store_detail_checkpoint),
                )
                runtime.stop_managed_process_group(fastid)
                fastid = None
                no_store_journey = no_store["m4NoStoreJourney"]
                no_store_after = _no_store_database_evidence(
                    database,
                    no_store_journey["recordId"],
                    journey["recordId"],
                    no_store_baseline,
                )
                _require_no_store_payload_absent(database)
                delta = provider.requests()[len(no_store_requests):]
                if Counter(delta) != Counter({
                    "/3/search/multi": 1,
                    f"/3/movie/{NO_STORE_PROVIDER_IDS[0]}": 3,
                    f"/3/movie/{NO_STORE_PROVIDER_IDS[1]}": 2,
                }):
                    raise RuntimeError("complete no-store provider exchange differs")
                requests_after_actions = provider.requests()
                content_after_actions = _content_database_snapshot(database)
                fastid = _start_fastid(data_root, trailbase_root, provider)
                no_store_restart = _browser({
                    "mode": "restart-no-store-record",
                    "email": email,
                    "password": password,
                    "recordId": no_store_journey["recordId"],
                    "recordPath": no_store_journey["recordPath"],
                    "attachRecordId": journey["recordId"],
                    "attachRecordPath": journey["recordPath"],
                })
                runtime.stop_managed_process_group(fastid)
                fastid = None
                if (_content_database_snapshot(database) != content_after_actions
                        or provider.requests() != requests_after_actions):
                    raise RuntimeError(
                        "no-store Record restart changed durable content or queried TMDB"
                    )
                _require_no_store_payload_absent(database)
                evidence["m4NoStoreJourney"] = no_store_journey
                evidence["m4NoStoreDetailCheckpoint"] = detail_evidence
                evidence["m4NoStoreDatabase"] = no_store_after
                evidence["m4NoStoreProviderRequests"] = delta
                evidence["m4NoStoreRestart"] = no_store_restart
            receipt = {
                "schema_version": "fasti.access-ordinary-browser.v1",
                "source": {
                    "git_commit": subprocess.check_output(
                        ["git", "rev-parse", "HEAD"], cwd=ROOT
                    )
                    .decode()
                    .strip(),
                    "git_tree": subprocess.check_output(
                        ["git", "rev-parse", "HEAD^{tree}"], cwd=ROOT
                    )
                    .decode()
                    .strip(),
                    "dirty": False,
                },
                "trailbase_release": runtime.load_release()["version"],
                "checks": evidence,
                "active_browser_sessions": session_count,
                "active_administrators": administrator_count,
                "packaged_tauri_authentication": "deferred_not_exercised",
            }
            path = arguments.receipt
            if not path.is_absolute():
                path = ROOT / path
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(
                json.dumps(receipt, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            print(
                "PASS: ordinary Chromium TrailBase -> Fasti session; "
                f"receipt={path}"
            )
        finally:
            password = ""
            if fastid is not None:
                runtime.stop_managed_process_group(fastid)
            if trailbase_process is not None:
                smoke.stop_process(trailbase_process, None)
            if smtp is not None:
                smtp.shutdown()
                smtp.server_close()
            if provider is not None:
                provider.close()


def _search_database_evidence(database: Path, record_id: str) -> dict[str, object]:
    with closing(sqlite3.connect(f"file:{database}?mode=ro", uri=True)) as connection:
        films = connection.execute(
            "SELECT record_id FROM records WHERE grain = 'film' AND status = 'active'"
        ).fetchall()
        identifiers = connection.execute(
            "SELECT namespace, grain, value FROM external_identifiers WHERE record_id = ? ORDER BY value",
            (record_id,),
        ).fetchall()
        actions = connection.execute(
            "SELECT record_id, receipt_json FROM search_action_receipts ORDER BY operation_id"
        ).fetchall()
        candidates = connection.execute(
            "SELECT c.candidate_receipt_id, c.provider_record_id, c.kind, c.candidate_json, p.provider_id "
            "FROM search_candidate_receipts c LEFT JOIN search_pages p ON p.sequence = c.page_sequence "
            "ORDER BY c.provider_record_id"
        ).fetchall()
        provenance = connection.execute(
            "SELECT DISTINCT source_record_id FROM metadata_claim_provenance "
            "WHERE record_id = ? AND provider_id = 'tmdb' AND provenance_state = 'complete' "
            "AND evidence_digest IS NOT NULL ORDER BY source_record_id", (record_id,),
        ).fetchall()
    expected_ids = [str(value) for value in PROVIDER_IDS]
    receipts = [json.loads(row[1]) for row in actions]
    candidate_ids = {row[0]: row[1] for row in candidates}
    expected_actions = {
        "create": (expected_ids[0], "created"),
        "attach": (expected_ids[1], "attached"),
    }
    if (films != [(record_id,)]
            or identifiers != [("tmdb.movie", "film", value) for value in expected_ids]
            or [row[1] for row in candidates] != expected_ids
            or any(row[0] != record_id for row in actions)
            or provenance != [(value,) for value in expected_ids]
            or len(receipts) != 2
            or sorted(row["action"]["kind"] for row in receipts) != ["attach", "create"]
            or any(row["action"].get("record_id") != record_id for row in receipts
                   if row["action"]["kind"] == "attach")
            or any(not row["actor_subject_id"] or not row["actor_client_id"] for row in receipts)
            or len({row["operation_id"] for row in receipts}) != 2):
        raise RuntimeError("durable real-process Search identity/provenance/action evidence differs")
    for _, provider_record_id, kind, candidate_json, provider in candidates:
        candidate = json.loads(candidate_json)
        if (provider != "tmdb" or kind != "movie"
                or candidate["provider"] != "tmdb" or candidate["kind"] != "movie"
                or candidate["provider_id"] != provider_record_id):
            raise RuntimeError("durable Search candidate differs from its owning provider page")
    for row in receipts:
        provider_record_id, disposition = expected_actions[row["action"]["kind"]]
        if (candidate_ids.get(row["candidate_receipt_id"]) != provider_record_id
                or row["provider"] != "tmdb" or row["grain"] != "film"
                or row["record_id"] != record_id or row["evidence_mode"] != "refetch"
                or row["disposition"] != disposition):
            raise RuntimeError("durable Search action is not bound to its expected candidate")
    return {"recordId": record_id, "identifiers": identifiers, "candidateCount": len(candidates),
            "actions": receipts, "completeProvenanceSourceIds": expected_ids}


_CONTENT_TABLES = (
    "records",
    "namespace_definitions",
    "external_identifiers",
    "search_pages",
    "search_candidate_receipts",
    "search_action_receipts",
    "metadata_field_claims",
    "metadata_claims",
    "metadata_claim_provenance",
    "metadata_projections",
    "metadata_attributions",
    "metadata_cache_entries",
    "metadata_cache_claims",
    "local_search_grams",
)


def _content_database_snapshot(database: Path) -> dict[str, list[tuple[object, ...]]]:
    snapshot: dict[str, list[tuple[object, ...]]] = {}
    with closing(sqlite3.connect(f"file:{database}?mode=ro", uri=True)) as connection:
        for table in _CONTENT_TABLES:
            columns = connection.execute(f"PRAGMA table_info({table})").fetchall()
            order = ", ".join(str(index) for index in range(1, len(columns) + 1))
            snapshot[table] = connection.execute(
                f"SELECT * FROM {table} ORDER BY {order}"
            ).fetchall()
    return snapshot


def _snapshot_digest(snapshot: dict[str, list[tuple[object, ...]]]) -> str:
    return "sha256:" + hashlib.sha256(repr(snapshot).encode()).hexdigest()


def _require_no_store_payload_absent(database: Path) -> None:
    sentinel = NO_STORE_OVERVIEW.encode()
    for path in (database, Path(f"{database}-wal")):
        if path.exists() and sentinel in path.read_bytes():
            raise RuntimeError(f"no-store provider payload reached {path.name}")


def _no_store_database_evidence(
    database: Path,
    record_id: str,
    attach_record_id: str,
    before: dict[str, list[tuple[object, ...]]],
) -> dict[str, object]:
    after = _content_database_snapshot(database)
    unchanged = set(_CONTENT_TABLES) - {
        "records", "external_identifiers", "search_action_receipts"
    }
    if any(after[table] != before[table] for table in unchanged):
        raise RuntimeError("no-store action invented durable Search or metadata content")
    if (len(after["records"]) != len(before["records"]) + 1
            or len(after["external_identifiers"]) !=
            len(before["external_identifiers"]) + 2
            or len(after["search_action_receipts"]) !=
            len(before["search_action_receipts"]) + 2):
        raise RuntimeError("no-store action changed an unexpected content row count")
    expected_ids = [str(value) for value in NO_STORE_PROVIDER_IDS]
    with closing(sqlite3.connect(f"file:{database}?mode=ro", uri=True)) as connection:
        created_identifiers = connection.execute(
            "SELECT namespace, grain, value FROM external_identifiers "
            "WHERE record_id = ? ORDER BY value", (record_id,),
        ).fetchall()
        attached_identifiers = connection.execute(
            "SELECT namespace, grain, value FROM external_identifiers "
            "WHERE record_id = ? ORDER BY value", (attach_record_id,),
        ).fetchall()
        receipts = [
            receipt for receipt in (
                json.loads(row[0]) for row in connection.execute(
                    "SELECT receipt_json FROM search_action_receipts "
                    "ORDER BY operation_id"
                ).fetchall()
            )
            if (receipt.get("provider") == "tmdb"
                and receipt.get("provider_record_id") in expected_ids)
        ]
        record = connection.execute(
            "SELECT grain, status FROM records WHERE record_id = ?", (record_id,),
        ).fetchone()
    actions = {row["action"]["kind"]: row for row in receipts}
    if (record != ("film", "active")
            or created_identifiers != [("tmdb.movie", "film", expected_ids[0])]
            or attached_identifiers != [
                ("tmdb.movie", "film", value)
                for value in [str(item) for item in PROVIDER_IDS] + [expected_ids[1]]
            ]
            or len(receipts) != 2
            or set(actions) != {"attach", "create"}
            or actions["create"]["provider_record_id"] != expected_ids[0]
            or actions["create"]["record_id"] != record_id
            or actions["create"]["disposition"] not in {"created", "reused"}
            or actions["attach"]["provider_record_id"] != expected_ids[1]
            or actions["attach"]["record_id"] != attach_record_id
            or actions["attach"]["action"].get("record_id") != attach_record_id
            or actions["attach"]["disposition"] not in
            {"attached", "already_attached"}
            or any(row["provider"] != "tmdb" or row["grain"] != "film"
                   or row["record_id"] not in {record_id, attach_record_id}
                   or row["origin"] != "UserSelectedProviderIdentifier"
                   or row["provider_record_id"] not in expected_ids
                   or "candidate_receipt_id" in row or "snapshot" in row
                   for row in receipts)):
        raise RuntimeError("durable no-store identifier/action evidence differs")
    return {
        "recordId": record_id,
        "createdIdentifiers": created_identifiers,
        "attachRecordId": attach_record_id,
        "attachedIdentifiers": attached_identifiers,
        "providerIdentifierActions": receipts,
        "retainedSearchRowsUnchanged": True,
        "metadataRowsUnchanged": True,
        "providerPayloadAbsent": True,
        "contentDigest": _snapshot_digest(after),
    }


if __name__ == "__main__":
    main()
