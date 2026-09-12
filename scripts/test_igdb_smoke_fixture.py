#!/usr/bin/env python3
"""Focused real TLS fixture checks; these do not exercise the Rust runtime."""

from __future__ import annotations

import concurrent.futures
import hashlib
import http.client
import json
import socket
import ssl
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from urllib.parse import urlencode

from igdb_smoke_fixture import (
    API_HOST, TOKEN_HOST, BODY_LIMIT, EVENT_LIMIT, GAME_FIELDS, HEALTH_BODY,
    SEARCH_BODY, PROVIDER_IDS, IgdbSmokeFixture,
)


class IgdbSmokeFixtureTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fasti-igdb-fixture-test-")
        self.fixture = IgdbSmokeFixture(Path(self.temporary.name) / "provider")
        self.context = ssl.create_default_context(cadata=self.fixture.ca_pem)
        self.credentials = json.loads(self.fixture.child_environment()["IGDB_CLIENT_CREDENTIALS"])
        self.token = None

    def tearDown(self):
        self.fixture.close()
        self.temporary.cleanup()

    def request(self, host=TOKEN_HOST, path="/oauth2/token", body=None, *, method="POST",
                sni=None, context=None, extras=(), headers=None, version="HTTP/1.1"):
        if body is None:
            body = urlencode({**self.credentials, "grant_type": "client_credentials"}).encode()
        if headers is None:
            headers = [("Content-Type", "application/x-www-form-urlencoded")]
            if host == API_HOST:
                headers = [("Content-Type", "text/plain"),
                           ("Client-ID", self.credentials["client_id"]),
                           ("Authorization", f"Bearer {self.token}")]
        address, port = self.fixture.address.rsplit(":", 1)
        plain = socket.create_connection((address, int(port)), timeout=3)
        try:
            tls = (context or self.context).wrap_socket(plain, server_hostname=sni or host)
        except BaseException:
            plain.close()
            raise
        try:
            lines = [f"{method} {path} {version}", f"Host: {host}",
                     "Accept: application/json", f"Content-Length: {len(body)}",
                     *(f"{key}: {value}" for key, value in (*headers, *extras)),
                     "Connection: close", "", ""]
            tls.sendall("\r\n".join(lines).encode() + body)
            response = http.client.HTTPResponse(tls)
            response.begin()
            self.assertEqual(response.reason, http.client.responses[response.status])
            return response.status, response.headers, response.read()
        finally:
            tls.close()

    def authorize(self):
        status, headers, body = self.request()
        self.assertEqual(status, 200)
        self.assertEqual(headers["cache-control"], "no-store")
        self.token = json.loads(body)["access_token"]

    def test_two_origins_search_health_detail_and_nonsecret_evidence(self):
        self.authorize()
        for body, expected in [(HEALTH_BODY, [PROVIDER_IDS[0]]),
                               (SEARCH_BODY, list(PROVIDER_IDS)),
                               *[(f"{GAME_FIELDS} where id = {value}; limit 1;", [value])
                                 for value in PROVIDER_IDS]]:
            status, headers, raw = self.request(API_HOST, "/v4/games", body.encode())
            self.assertEqual(status, 200)
            self.assertEqual(headers.get_content_type(), "application/json")
            self.assertEqual(int(headers["content-length"]), len(raw))
            self.assertEqual([row["id"] for row in json.loads(raw)], expected)
            self.assertEqual(self.fixture.events()[-1]["response_body_sha256"],
                             hashlib.sha256(raw).hexdigest())
        events = self.fixture.events()
        self.assertEqual([event["operation"] for event in events],
                         ["token", "health", "search", "detail", "detail"])
        self.assertEqual([event["origin"] for event in events], [TOKEN_HOST] + [API_HOST] * 4)
        self.assertEqual([event.get("provider_record_id") for event in events],
                         [None, None, None, "842001", "842002"])
        self.assertEqual([event["monotonic_ns"] for event in events],
                         sorted(event["monotonic_ns"] for event in events))
        for secret in [*self.credentials.values(), self.token]:
            self.assertNotIn(secret, json.dumps(events))
        events[0]["operation"] = "modified"
        self.assertEqual(self.fixture.events()[0]["operation"], "token")

    def test_exact_tls_origins_and_request_bounds(self):
        self.authorize()
        accepted = self.fixture.events()
        for kwargs in [
            {"sni": API_HOST},
            {"path": "/oauth2/token?client_secret=sentinel"},
            {"path": "//oauth2/token"},
            {"path": "https://id.twitch.tv/oauth2/token"},
            {"path": "/v4/games"},
            {"body": b"client_id=wrong&client_secret=wrong&grant_type=client_credentials"},
            {"body": urlencode({**self.credentials, "grant_type": "client_credentials"}).encode()
                     + b"&client_id=duplicate"},
            {"body": b"x" * (BODY_LIMIT + 1)},
            {"extras": (("Host", TOKEN_HOST),)},
            {"extras": (("Content-Length", "1"),)},
            {"extras": (("Transfer-Encoding", "chunked"),)},
            {"extras": (("Authorization", "Bearer unexpected"),)},
            {"host": API_HOST, "path": "/v4/games", "body": b"fields *; limit 500;"},
            {"host": API_HOST, "path": "/v4/games", "body": SEARCH_BODY.encode(),
             "extras": (("Authorization", "Bearer duplicate"),)},
        ]:
            with self.subTest(kwargs=tuple(kwargs)):
                self.assertEqual(self.request(**kwargs)[0], 400)
        self.assertEqual(self.request(method="GET")[0], 501)
        with self.assertRaises(ssl.SSLError):
            self.request(sni="example.invalid")
        with self.assertRaises(ssl.SSLError):
            self.request(context=ssl.create_default_context())
        self.assertEqual(self.fixture.events(), accepted)
        self.assertEqual(self.fixture.http_request_count(), 16)

    def test_parser_errors_do_not_reflect_input(self):
        sentinel = "fixture-sensitive-input-sentinel"
        for arguments, expected in [({"method": sentinel}, 501),
                                    ({"version": f"HTTP/{sentinel}"}, 400)]:
            status, headers, body = self.request(**arguments)
            self.assertEqual(status, expected)
            self.assertEqual(body, b'{"error":"fixture rejected"}')
            self.assertNotIn(sentinel, str(headers))
            self.assertNotIn(sentinel.encode(), body)
        self.assertEqual(self.fixture.events(), [])
        self.assertEqual(self.fixture.http_request_count(), 2)

    def test_token_failure_modes_and_held_response_release(self):
        for mode, expected in [("rejected", 400), ("malformed", 200),
                               ("oversized", 200), ("expired", 200), ("wrongtype", 200)]:
            self.fixture.set_token_response(mode)
            status, _, body = self.request()
            self.assertEqual(status, expected)
            if mode == "oversized":
                self.assertEqual(len(body), 16 * 1024 + 1)
            elif mode == "expired":
                self.assertEqual(json.loads(body)["expires_in"], 0)
            elif mode == "malformed":
                with self.assertRaises(json.JSONDecodeError):
                    json.loads(body)
            elif mode == "wrongtype":
                self.assertEqual(json.loads(body)["token_type"], "mac")
        self.fixture.set_token_response("valid")
        self.fixture.hold("token")
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            pending = pool.submit(self.request)
            self.assertTrue(self.fixture.wait_for("token", count=6))
            self.assertFalse(pending.done())
            self.fixture.release("token")
            self.assertEqual(pending.result(timeout=3)[0], 200)
        self.authorize()
        self.fixture.hold("metadata")
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            pending = pool.submit(self.request, API_HOST, "/v4/games", SEARCH_BODY.encode())
            self.assertTrue(self.fixture.wait_for("search"))
            self.assertFalse(pending.done())
            self.fixture.close()  # Cleanup must release held responses as well.
            self.assertEqual(pending.result(timeout=3)[0], 200)

    def test_event_capacity_and_listener_cleanup(self):
        for _ in range(EVENT_LIMIT):
            self.assertEqual(self.request()[0], 200)
        self.assertEqual(self.request()[0], 400)
        self.assertEqual(len(self.fixture.events()), EVENT_LIMIT)
        self.assertEqual(self.fixture.http_request_count(), EVENT_LIMIT + 1)
        address, port = self.fixture.address.rsplit(":", 1)
        self.fixture.close()
        with self.assertRaises(OSError):
            socket.create_connection((address, int(port)), timeout=1)

    def test_close_joins_held_token_worker_before_clearing_credentials(self):
        expected_token = self.fixture._token
        self.fixture.hold("token")
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            pending = pool.submit(self.request)
            self.assertTrue(self.fixture.wait_for("token"))
            self.assertFalse(pending.done())
            # Inspect the stdlib lifecycle owner, not a second worker registry.
            workers = tuple(self.fixture._server._threads)
            self.assertEqual(len(workers), 1)
            self.assertTrue(all(worker.is_alive() and not worker.daemon for worker in workers))
            self.fixture.close()
            self.assertTrue(all(not worker.is_alive() for worker in workers))
            self.assertFalse(self.fixture._thread.is_alive())
            status, _, body = pending.result(timeout=3)
            self.assertEqual(status, 200)
            self.assertTrue(expected_token)
            self.assertEqual(json.loads(body)["access_token"], expected_token)
            self.assertEqual(self.fixture._token, "")


class IgdbFixtureBridgeTest(unittest.TestCase):
    def test_bounded_stdio_commands_and_stop_cleanup(self):
        process = subprocess.Popen(
            [sys.executable, str(Path(__file__).with_name("igdb_smoke_fixture.py"))],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        try:
            output, error = process.communicate(
                b'{"command":"hold","stage":"token"}\n'
                b'{"command":"release","stage":"token"}\n'
                b'{"command":"token_response","mode":"expired"}\n'
                b'{"command":"events"}\n'
                b'{"command":"invalid"}\n'
                b'{"command":"stop"}\n', timeout=25,
            )
            self.assertEqual(process.returncode, 0)
            self.assertEqual(error, b"")
            rows = [json.loads(line) for line in output.splitlines()]
            startup = rows[0]
            self.assertTrue(startup["ok"])
            self.assertIn("client_id", json.loads(startup["credential_json"]))
            self.assertFalse(Path(startup["ca_path"]).exists())
            self.assertEqual([row["ok"] for row in rows[1:]], [True] * 4 + [False, True])
            self.assertEqual(rows[4]["events"], [])
            self.assertEqual(rows[4]["http_request_count"], 0)
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate(timeout=5)


if __name__ == "__main__":
    unittest.main()
