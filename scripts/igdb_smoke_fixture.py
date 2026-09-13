"""Disposable dual-origin TLS input, never a substitute Fasti implementation."""

from __future__ import annotations

import hashlib
import hmac
import http.server
import json
import os
import secrets
import ssl
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from urllib.parse import parse_qs


API_HOST = "api.igdb.com"
TOKEN_HOST = "id.twitch.tv"
TITLE = "Fasti Fixture Game"
PROVIDER_IDS = (842001, 842002)
GAME_FIELDS = "fields name,summary,first_release_date,cover.image_id;"
SEARCH_BODY = f'{GAME_FIELDS} search "{TITLE}"; limit 10; offset 0;'
HEALTH_BODY = "fields id,name; limit 1;"
BODY_LIMIT = 4096
EVENT_LIMIT = 128


class IgdbSmokeFixture:
    """Exact synthetic requests with bounded, credential-free event evidence.

    hold/release control responses locally, not through an HTTP control route.
    Event times are server arrival times, not proof of client dispatch spacing.
    """

    def __init__(self, directory: Path):
        directory.mkdir(mode=0o700)
        self._client_id = secrets.token_hex(16)
        self._client_secret = secrets.token_hex(32)
        self._token = secrets.token_hex(32)
        self._condition = threading.Condition()
        self._events: list[dict[str, object]] = []
        self._http_requests = 0
        self._release = {name: threading.Event() for name in ("token", "metadata")}
        for event in self._release.values():
            event.set()
        self._token_mode = "valid"
        self._closed = False
        self._server = None
        self._thread = None
        ca, ca_key = directory / "ca.pem", directory / "ca.key"
        key, csr = directory / "server.key", directory / "server.csr"
        certificate = directory / "server.pem"
        commands = [
            ["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
             "-subj", "/CN=Fasti disposable IGDB smoke CA", "-keyout", str(ca_key),
             "-out", str(ca), "-addext", "basicConstraints=critical,CA:TRUE",
             "-addext", "keyUsage=critical,keyCertSign,cRLSign"],
            ["req", "-new", "-newkey", "rsa:2048", "-nodes", "-subj", f"/CN={API_HOST}",
             "-keyout", str(key), "-out", str(csr),
             "-addext", f"subjectAltName=DNS:{API_HOST},DNS:{TOKEN_HOST}",
             "-addext", "extendedKeyUsage=serverAuth"],
            ["x509", "-req", "-in", str(csr), "-CA", str(ca), "-CAkey", str(ca_key),
             "-CAcreateserial", "-days", "1", "-copy_extensions", "copy",
             "-out", str(certificate)],
        ]
        for arguments in commands:
            subprocess.run(  # nosec B603 -- fixed developer-owned fixture toolchain; no shell.
                ["openssl", *arguments], check=True, timeout=15,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
        for path in directory.iterdir():
            os.chmod(path, 0o600)
        self.ca_pem = ca.read_text(encoding="ascii")
        self.ca_path = ca
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version = ssl.TLSVersion.TLSv1_2
        context.load_cert_chain(certificate, key)

        def require_sni(connection, name, _context):
            if name not in (API_HOST, TOKEN_HOST):
                return ssl.ALERT_DESCRIPTION_UNRECOGNIZED_NAME
            connection.fixture_hostname = name
            return None

        context.set_servername_callback(require_sni)
        owner = self

        class Server(http.server.ThreadingHTTPServer):
            # ThreadingMixIn owns and joins the at-most-four request workers.
            # close() releases both holds before server_close joins them; socket
            # reads/writes retain their three-second timeout.
            daemon_threads = False
            block_on_close = True

            def process_request(self, request, address):
                if not self.slots.acquire(blocking=False):
                    self.shutdown_request(request)
                    return
                try:
                    super().process_request(request, address)
                except BaseException:
                    self.slots.release()
                    raise

            def process_request_thread(self, request, address):
                try:
                    super().process_request_thread(request, address)
                finally:
                    self.slots.release()

            def get_request(self):
                connection, address = super().get_request()
                connection.settimeout(3)
                try:
                    return context.wrap_socket(connection, server_side=True), address
                except BaseException:
                    connection.close()
                    raise

            def handle_error(self, *_args):
                return  # Never print request bodies, headers, or cancellation errors.

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_args):
                return

            def parse_request(self):
                with owner._condition:
                    # Saturating count includes rejected/malformed HTTP requests.
                    owner._http_requests = min(owner._http_requests + 1, EVENT_LIMIT + 1)
                if not super().parse_request():
                    return False
                # BaseHTTPRequestHandler normalizes leading // before setting
                # path. The fixture oracle admits only the exact raw targets.
                parts = self.raw_requestline.split()
                if len(parts) != 3 or parts[1] not in (b"/oauth2/token", b"/v4/games"):
                    self.reply(400, b'{"error":"fixture rejected"}')
                    return False
                return True

            def send_error(self, code, message=None, explain=None):
                # The stdlib error template reflects request lines/methods.
                # Never include those inputs in fixture errors or reason text.
                self.reply(code, b'{"error":"fixture rejected"}')

            def reply(self, status, body):
                self.close_connection = True
                self.send_response(status)
                self.send_header("content-type", "application/json")
                self.send_header("content-length", str(len(body)))
                self.send_header("cache-control", "private, max-age=300, stale-if-error=300"
                                 if getattr(self, "path", None) == "/v4/games" else "no-store")
                self.send_header("connection", "close")
                self.end_headers()
                try:
                    self.wfile.write(body)
                except OSError:
                    pass  # An intentionally cancelled client may have disconnected.

            def do_POST(self):
                rejected = b'{"error":"fixture rejected"}'
                lengths = self.headers.get_all("Content-Length", [])
                host = self.headers.get_all("Host", [])
                content_type = self.headers.get_all("Content-Type", [])
                if (len(lengths) != 1 or not lengths[0].isascii()
                        or not lengths[0].isdigit() or len(lengths[0]) > 5
                        or not 0 < int(lengths[0]) <= BODY_LIMIT
                        or self.headers.get_all("Transfer-Encoding")
                        or self.headers.get_all("Expect")
                        or host != [getattr(self.connection, "fixture_hostname", None)]
                        or self.headers.get_all("Accept") != ["application/json"]):
                    self.reply(400, rejected)
                    return
                body = self.rfile.read(int(lengths[0]))
                operation, payload = None, None
                if len(body) == int(lengths[0]):
                    if (host == [TOKEN_HOST] and self.path == "/oauth2/token"
                            and content_type == ["application/x-www-form-urlencoded"]
                            and not self.headers.get_all("Authorization")
                            and not self.headers.get_all("Client-ID")):
                        try:
                            form = parse_qs(body.decode("ascii"), strict_parsing=True,
                                            max_num_fields=3)
                        except (ValueError, UnicodeError):
                            form = {}
                        expected = {"client_id": [owner._client_id],
                                    "client_secret": [owner._client_secret],
                                    "grant_type": ["client_credentials"]}
                        if form.keys() == expected.keys() and all(
                            len(form[key]) == 1 and owner._same(form[key][0], values[0])
                            for key, values in expected.items()
                        ):
                            operation = "token"
                    elif (host == [API_HOST] and self.path == "/v4/games"
                          and content_type == ["text/plain"]
                          and self.headers.get_all("Client-ID") == [owner._client_id]
                          and self.headers.get_all("Authorization") == [f"Bearer {owner._token}"]):
                        if body == HEALTH_BODY.encode():
                            operation, payload = "health", [owner._game(PROVIDER_IDS[0])]
                        elif body == SEARCH_BODY.encode():
                            operation, payload = "search", [owner._game(value) for value in PROVIDER_IDS]
                        else:
                            for value in PROVIDER_IDS:
                                if body == f"{GAME_FIELDS} where id = {value}; limit 1;".encode():
                                    operation, payload = "detail", [owner._game(value)]
                with owner._condition:
                    if operation is None or owner._closed or len(owner._events) >= EVENT_LIMIT:
                        operation = None
                    else:
                        event = {"origin": host[0], "operation": operation,
                                 "monotonic_ns": time.monotonic_ns()}
                        owner._events.append(event)
                        mode = owner._token_mode
                        owner._condition.notify_all()
                if operation is None:
                    self.reply(400, rejected)
                    return
                gate = "token" if operation == "token" else "metadata"
                if not owner._release[gate].wait(timeout=10):
                    self.reply(503, b'{"error":"fixture hold expired"}')
                    return
                if operation == "token":
                    status, response = owner._token_response(mode)
                else:
                    status, response = 200, json.dumps(payload).encode()
                    with owner._condition:
                        event["response_body_sha256"] = hashlib.sha256(response).hexdigest()
                        if operation == "detail":
                            event["provider_record_id"] = str(payload[0]["id"])
                self.reply(status, response)

        self._server = Server(("127.0.0.1", 0), Handler)
        self._server.slots = threading.BoundedSemaphore(4)
        self.address = f"127.0.0.1:{self._server.server_address[1]}"
        self._thread = threading.Thread(
            target=self._server.serve_forever, kwargs={"poll_interval": 0.1}, daemon=True,
        )
        self._thread.start()

    @staticmethod
    def _same(left: str, right: str) -> bool:
        return hmac.compare_digest(hashlib.sha256(left.encode()).digest(),
                                   hashlib.sha256(right.encode()).digest())

    @staticmethod
    def _game(provider_id: int) -> dict[str, object]:
        return {"id": provider_id, "name": TITLE, "first_release_date": 1577836800,
                "summary": "Deterministic provider detail for the real Search journey."}

    def _token_response(self, mode: str) -> tuple[int, bytes]:
        if mode == "rejected":
            return 400, b'{"error":"fixture credentials rejected"}'
        if mode == "malformed":
            return 200, b'{'
        if mode == "oversized":
            return 200, b' ' * (16 * 1024 + 1)
        return 200, json.dumps({"access_token": self._token,
                                "expires_in": 0 if mode == "expired" else 60,
                                "token_type": "mac" if mode == "wrongtype" else "bearer"}).encode()

    def set_token_response(self, mode: str) -> None:
        if mode not in ("valid", "rejected", "malformed", "oversized", "expired", "wrongtype"):
            raise ValueError("unknown fixture token mode")
        with self._condition:
            self._token_mode = mode

    def child_environment(self) -> dict[str, str]:
        return {"IGDB_CLIENT_CREDENTIALS": json.dumps(
                    {"client_id": self._client_id, "client_secret": self._client_secret},
                    separators=(",", ":")),
                "FASTI_IGDB_SMOKE_RESOLVE": self.address,
                "FASTI_IGDB_SMOKE_CA_PEM": self.ca_pem}

    def events(self) -> list[dict[str, object]]:
        with self._condition:
            return [dict(event) for event in self._events]

    def http_request_count(self) -> int:
        """All parsed HTTP attempts, saturating at EVENT_LIMIT + 1 (129)."""
        with self._condition:
            return self._http_requests

    def wait_for(self, operation: str, count: int = 1, timeout: float = 3) -> bool:
        if operation not in ("token", "health", "search", "detail") or not 1 <= count <= EVENT_LIMIT:
            raise ValueError("invalid fixture event request")
        if not 0 < timeout <= 10:
            raise ValueError("invalid fixture wait bound")
        with self._condition:
            return self._condition.wait_for(
                lambda: sum(event["operation"] == operation for event in self._events) >= count,
                timeout=timeout,
            )

    def hold(self, stage: str) -> None:
        self._release[stage].clear()

    def release(self, stage: str) -> None:
        self._release[stage].set()

    def close(self) -> None:
        with self._condition:
            self._closed = True
        for event in self._release.values():
            event.set()
        if self._server is not None:
            self._server.shutdown()
            self._server.server_close()
        if self._thread is not None:
            self._thread.join(timeout=4)
            if self._thread.is_alive():
                raise RuntimeError("IGDB fixture did not stop")
        self._client_id = self._client_secret = self._token = ""


def main() -> None:
    """Private bounded stdio bridge for an actual Rust runtime test subprocess."""
    def emit(value):
        print(json.dumps(value, separators=(",", ":")), flush=True)

    with tempfile.TemporaryDirectory(prefix="fasti-igdb-tls-") as directory:
        fixture = IgdbSmokeFixture(Path(directory) / "provider")
        try:
            emit({"ok": True, "address": fixture.address, "ca_path": str(fixture.ca_path),
                  "credential_json": fixture.child_environment()["IGDB_CLIENT_CREDENTIALS"]})
            while True:
                line = sys.stdin.buffer.readline(BODY_LIMIT + 1)
                if not line:
                    break
                if len(line) > BODY_LIMIT or not line.endswith(b"\n"):
                    emit({"ok": False, "error": "invalid fixture command"})
                    break
                try:
                    request = json.loads(line)
                    if not isinstance(request, dict):
                        raise ValueError("command must be an object")
                    command = request["command"]
                    result = {"ok": True}
                    if command == "stop":
                        fixture.close()
                        emit(result)
                        break
                    if command == "token_response":
                        fixture.set_token_response(request["mode"])
                    elif command in ("hold", "release"):
                        getattr(fixture, command)(request["stage"])
                    elif command == "events":
                        result["events"] = fixture.events()
                        result["http_request_count"] = fixture.http_request_count()
                    elif command == "wait_for":
                        result["reached"] = fixture.wait_for(
                            request["operation"], request.get("count", 1), request.get("timeout", 3),
                        )
                    else:
                        raise ValueError("unknown fixture command")
                    emit(result)
                except (ValueError, KeyError, TypeError):
                    emit({"ok": False, "error": "invalid fixture command"})
        finally:
            fixture.close()


if __name__ == "__main__":
    main()
