#!/usr/bin/env python3
"""Focused loopback TLS checks for the bounded TMDB smoke fixture."""

from __future__ import annotations

import http.client
import json
import socket
import ssl
import tempfile
import unittest
from pathlib import Path
from urllib.parse import quote

from tmdb_smoke_fixture import HOST, PROVIDER_IDS, TITLE, TmdbSmokeFixture


class TmdbSmokeFixtureTest(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory(prefix="fasti-tmdb-fixture-test-")
        self.fixture = TmdbSmokeFixture(
            Path(self._temporary.name) / "provider-fixture"
        )
        environment = self.fixture.child_environment()
        self.authorization = f"Bearer {environment['TMDB_API_READ_ACCESS_TOKEN']}"
        self.context = ssl.create_default_context(cadata=self.fixture.ca_pem)

    def tearDown(self) -> None:
        if self.fixture is not None:
            self.fixture.close()
        self.authorization = ""
        self._temporary.cleanup()

    def request(
        self,
        target: str,
        *,
        context: ssl.SSLContext | None = None,
        server_hostname: str = HOST,
        host_header: str = HOST,
        authorization: str | None = None,
        extra_headers: tuple[tuple[str, str], ...] = (),
    ) -> tuple[int, http.client.HTTPMessage, bytes]:
        address, port = self.fixture.address.rsplit(":", 1)
        plain = socket.create_connection((address, int(port)), timeout=3)
        try:
            tls = (context or self.context).wrap_socket(
                plain, server_hostname=server_hostname
            )
        except BaseException:
            plain.close()
            raise
        try:
            headers = [
                f"GET {target} HTTP/1.1",
                f"Host: {host_header}",
                f"Authorization: {authorization or self.authorization}",
                *(f"{name}: {value}" for name, value in extra_headers),
                "Connection: close",
                "",
                "",
            ]
            tls.sendall("\r\n".join(headers).encode("ascii"))
            response = http.client.HTTPResponse(tls)
            response.begin()
            return response.status, response.headers, response.read()
        finally:
            tls.close()

    def assert_json_response(
        self, target: str, expected_status: int = 200
    ) -> dict[str, object]:
        status, headers, body = self.request(target)
        self.assertEqual(status, expected_status)
        self.assertEqual(headers.get_content_type(), "application/json")
        self.assertEqual(
            headers.get("cache-control"),
            "private, max-age=300, stale-if-error=300",
        )
        self.assertEqual(int(headers["content-length"]), len(body))
        payload = json.loads(body)
        self.assertIsInstance(payload, dict)
        return payload

    def test_real_tls_routes_and_rejections(self) -> None:
        configuration = self.assert_json_response("/3/configuration")
        self.assertEqual(
            configuration,
            {"images": {"secure_base_url": "https://image.tmdb.org/t/p/"}},
        )

        search = self.assert_json_response(
            "/3/search/multi?"
            f"query={quote(TITLE)}&page=1&language=en-US&include_adult=false"
        )
        self.assertEqual(search["page"], 1)
        self.assertEqual(search["total_pages"], 1)
        self.assertEqual(
            [candidate["id"] for candidate in search["results"]],
            list(PROVIDER_IDS),
        )
        for candidate in search["results"]:
            self.assertEqual(candidate["title"], TITLE)
            self.assertEqual(candidate["release_date"], "2020-01-01")
            self.assertEqual(candidate["media_type"], "movie")

        for provider_id in PROVIDER_IDS:
            details = self.assert_json_response(f"/3/movie/{provider_id}")
            self.assertEqual(details["id"], provider_id)
            self.assertEqual(details["title"], TITLE)
            self.assertEqual(
                details["overview"],
                "Deterministic provider detail for the real Search journey.",
            )

        accepted = [
            "/3/configuration",
            "/3/search/multi",
            "/3/movie/842001",
            "/3/movie/842002",
        ]
        self.assertEqual(self.fixture.requests(), accepted)

        rejected = [
            self.request("/3/configuration", host_header="example.invalid"),
            self.request("/3/configuration", authorization="Bearer rejected"),
            self.request(
                "/3/configuration",
                extra_headers=(("Authorization", self.authorization),),
            ),
            self.request(
                "/3/configuration?api_key=not-a-credential",
            ),
            self.request("/3/unknown"),
        ]
        for status, headers, body in rejected:
            self.assertEqual(status, 400)
            self.assertEqual(headers.get_content_type(), "application/json")
            self.assertEqual(json.loads(body), {"error": "fixture rejected"})
        self.assertEqual(self.fixture.requests(), accepted)

        with self.assertRaises(ssl.SSLError):
            self.request(
                "/3/configuration",
                context=ssl.create_default_context(),
            )
        with self.assertRaises(ssl.SSLError):
            self.request(
                "/3/configuration",
                server_hostname="example.invalid",
            )
        self.assertEqual(self.fixture.requests(), accepted)

    def test_request_bound_and_cleanup(self) -> None:
        for _ in range(128):
            status, _, _ = self.request("/3/configuration")
            self.assertEqual(status, 200)
        self.assertEqual(len(self.fixture.requests()), 128)

        status, _, body = self.request("/3/configuration")
        self.assertEqual(status, 400)
        self.assertEqual(json.loads(body), {"error": "fixture rejected"})
        self.assertEqual(len(self.fixture.requests()), 128)

        address, port = self.fixture.address.rsplit(":", 1)
        self.fixture.close()
        self.fixture = None
        with self.assertRaises(OSError):
            socket.create_connection((address, int(port)), timeout=1)


if __name__ == "__main__":
    unittest.main()
