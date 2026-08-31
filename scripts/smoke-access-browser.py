#!/usr/bin/env python3
"""Prove the ordinary-browser TrailBase-to-Fasti session boundary."""

from __future__ import annotations

import argparse
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
from typing import Any

import trailbase_runtime as runtime


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


def _post(path: str, payload: dict[str, object], bearer: str | None = None):
    headers = {"content-type": "application/json"}
    if bearer:
        headers["authorization"] = f"Bearer {bearer}"
    request = urllib.request.Request(
        FASTI_ORIGIN + path,
        json.dumps(payload).encode(),
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


def _start_fastid(data_root: Path, trailbase_root: Path) -> subprocess.Popen[bytes]:
    environment = dict(os.environ)
    environment.update(
        FASTI_LISTEN="127.0.0.1:8420",
        FASTI_PORT_FALLBACK="fail",
        FASTI_DATA_ROOT=str(data_root),
        FASTI_TRAILBASE_ROOT=str(trailbase_root),
        FASTI_STATIC_DIR=str(ROOT / "apps/web/dist"),
    )
    process = runtime.start_managed_process_group(
        [ROOT / "target/debug/fastid"],
        environment=environment,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    _wait_health(process)
    return process


def _browser(payload: dict[str, object]) -> dict[str, object]:
    completed = subprocess.run(  # nosec B603 -- fixed local script and no shell.
        ["node", ROOT / "scripts/smoke-access-browser.mjs"],
        input=json.dumps(payload).encode(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=60,
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


def _bootstrap_cli(
    data_root: Path, trailbase_root: Path, email: str, password: str
) -> None:
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 8420), _Callback)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    pid, terminal = pty.fork()
    if pid == 0:
        os.execv(
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
        if status is None:
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
        "--receipt",
        type=Path,
        default=ROOT / "target/fasti-receipts/access-c1-ordinary-browser.json",
    )
    return parser.parse_args()


def main() -> None:
    arguments = _arguments()
    if subprocess.check_output(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=ROOT
    ).strip():
        raise RuntimeError("ordinary-browser proof requires a clean tree")
    fixture._require_free_port(4000, "ordinary-browser TrailBase proof")
    fixture._require_free_port(8420, "ordinary-browser Fasti proof")

    trailbase_process = None
    fastid = None
    smtp = None
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
        fixture._write_fixture_config(trailbase_root, int(smtp.server_address[1]))
        runtime.prepare_runtime_lock(trailbase_root)
        runtime.prepare_installation(trailbase_root, "native")
        runtime.verify_installation(trailbase_root)
        threading.Thread(target=smtp.serve_forever, daemon=True).start()
        trailbase_process, _ = smoke.start_fixture_release(
            executable, trailbase_root, 4000
        )
        email, password = fixture._register_verified_human(smoke, smtp.messages)
        try:
            fastid = _start_fastid(data_root, trailbase_root)
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
            enrolled.clear()
            runtime.stop_managed_process_group(fastid)
            fastid = None

            _bootstrap_cli(data_root, trailbase_root, email, password)
            fastid = _start_fastid(data_root, trailbase_root)
            evidence = _browser(
                {"mode": "sign-in", "email": email, "password": password}
            )
            database = data_root / "current/fasti.sqlite3"
            with sqlite3.connect(f"file:{database}?mode=ro", uri=True) as connection:
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


if __name__ == "__main__":
    main()
