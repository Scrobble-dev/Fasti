"""Bounded TLS input for the opt-in real-process Search smoke journey."""

from __future__ import annotations

import hashlib
import hmac
import http.server
import json
import os
import secrets
import ssl
import subprocess
import threading
from pathlib import Path
from urllib.parse import parse_qs, urlsplit


HOST = "api.themoviedb.org"
TITLE = "Fasti Fixture Film"
PROVIDER_IDS = (842001, 842002)


class TmdbSmokeFixture:
    """Own a disposable provider server, not a substitute Fasti implementation."""

    def __init__(self, directory: Path):
        directory.mkdir(mode=0o700)
        self._credential = secrets.token_hex(32)
        self._header_digest = hashlib.sha256(
            f"Bearer {self._credential}".encode()
        ).digest()
        self._requests: list[str] = []
        self._lock = threading.Lock()
        self._server = None
        self._thread = None
        ca = directory / "ca.pem"
        ca_key = directory / "ca.key"
        key = directory / "server.key"
        csr = directory / "server.csr"
        certificate = directory / "server.pem"
        commands = [
            ["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
             "-subj", "/CN=Fasti disposable smoke CA", "-keyout", str(ca_key),
             "-out", str(ca), "-addext", "basicConstraints=critical,CA:TRUE",
             "-addext", "keyUsage=critical,keyCertSign,cRLSign"],
            ["req", "-new", "-newkey", "rsa:2048", "-nodes", "-subj", f"/CN={HOST}",
             "-keyout", str(key), "-out", str(csr),
             "-addext", f"subjectAltName=DNS:{HOST}",
             "-addext", "extendedKeyUsage=serverAuth"],
            ["x509", "-req", "-in", str(csr), "-CA", str(ca), "-CAkey", str(ca_key),
             "-CAcreateserial", "-days", "1", "-copy_extensions", "copy",
             "-out", str(certificate)],
        ]
        for arguments in commands:
            subprocess.run(  # nosec B603 -- fixed local fixture tooling; no shell.
                ["openssl", *arguments], check=True, timeout=15,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
        for path in directory.iterdir():
            os.chmod(path, 0o600)
        self.ca_pem = ca.read_text(encoding="ascii")
        owner = self

        class Server(http.server.ThreadingHTTPServer):
            daemon_threads = True

            def handle_error(self, *_args):
                # Requests and credentials must not escape through fixture logs.
                return

        class Handler(http.server.BaseHTTPRequestHandler):
            def setup(self):
                self.request.settimeout(3)
                super().setup()

            def log_message(self, *_args):
                return

            def do_GET(self):
                payload = None
                parsed = urlsplit(self.path)
                try:
                    query = parse_qs(parsed.query, strict_parsing=True, max_num_fields=8)
                except ValueError:
                    query = {"invalid": []}
                valid = (
                    not parsed.scheme and not parsed.netloc and not parsed.fragment
                    and self.headers.get_all("Host") == [HOST]
                    and len(self.headers.get_all("Authorization", [])) == 1
                    and hmac.compare_digest(
                        hashlib.sha256(self.headers.get("Authorization", "").encode()).digest(),
                        owner._header_digest,
                    )
                    and not (set(query) - {"query", "page", "language", "region", "include_adult"})
                    and all(len(values) == 1 for values in query.values())
                )
                if valid:
                    if parsed.path == "/3/configuration" and not query:
                        payload = {"images": {"secure_base_url": "https://image.tmdb.org/t/p/"}}
                    elif (parsed.path == "/3/search/multi"
                          and query.get("query") == [TITLE]
                          and query.get("page", ["1"]) == ["1"]):
                        payload = {"page": 1, "total_pages": 1,
                                   "results": [owner._movie(value) for value in PROVIDER_IDS]}
                    elif parsed.path in [f"/3/movie/{value}" for value in PROVIDER_IDS]:
                        payload = owner._movie(int(parsed.path.rsplit("/", 1)[1]))
                with owner._lock:
                    if len(owner._requests) >= 128:
                        payload = None
                    elif payload is not None:
                        # Only known non-secret paths, never headers or raw query strings.
                        owner._requests.append(parsed.path)
                body = json.dumps(payload if payload is not None else {"error": "fixture rejected"}).encode()
                self.send_response(200 if payload is not None else 400)
                self.send_header("content-type", "application/json")
                self.send_header("content-length", str(len(body)))
                self.send_header("cache-control", "private, max-age=300, stale-if-error=300")
                self.end_headers()
                self.wfile.write(body)

        server = Server(("127.0.0.1", 0), Handler)
        try:
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.minimum_version = ssl.TLSVersion.TLSv1_2
            context.load_cert_chain(certificate, key)

            def require_sni(_socket, name, _context):
                if name != HOST:
                    return ssl.ALERT_DESCRIPTION_UNRECOGNIZED_NAME
                return None

            context.set_servername_callback(require_sni)
            server.socket = context.wrap_socket(server.socket, server_side=True)
        except BaseException:
            server.server_close()
            raise
        self._server = server
        self.address = f"127.0.0.1:{server.server_address[1]}"
        self._thread = threading.Thread(
            target=server.serve_forever, kwargs={"poll_interval": 0.1}, daemon=True,
        )
        self._thread.start()

    @staticmethod
    def _movie(provider_id: int) -> dict[str, object]:
        return {"id": provider_id, "media_type": "movie", "title": TITLE,
                "original_title": TITLE, "release_date": "2020-01-01",
                "overview": "Deterministic provider detail for the real Search journey.",
                "poster_path": None, "adult": False}

    def child_environment(self) -> dict[str, str]:
        return {"TMDB_API_READ_ACCESS_TOKEN": self._credential,
                "FASTI_TMDB_SMOKE_RESOLVE": self.address,
                "FASTI_TMDB_SMOKE_CA_PEM": self.ca_pem}

    def requests(self) -> list[str]:
        with self._lock:
            return list(self._requests)

    def close(self) -> None:
        if self._server is not None:
            self._server.shutdown()
            self._server.server_close()
        if self._thread is not None:
            self._thread.join(timeout=3)
            if self._thread.is_alive():
                raise RuntimeError("TMDB fixture did not stop")
        self._credential = ""
        self._header_digest = b""
