from unittest.mock import MagicMock, patch

from django.test import SimpleTestCase

from integrations.safe_fetch import (
    SafeFetchError,
    SafeFetchResult,
    _fetch_once,
    _parse_target,
    _read_bounded,
    _resolve_public_addresses,
    fetch_bytes,
    fetch_json,
    safe_origin,
)


class SafeFetchSecurityTests(SimpleTestCase):
    """Verify the generic remote fetch boundary fails closed."""

    def test_requires_https_and_rejects_embedded_credentials(self):
        with self.assertRaisesRegex(SafeFetchError, "must use HTTPS"):
            _parse_target("http://example.com/manifest.json")
        with self.assertRaisesRegex(SafeFetchError, "must not contain credentials"):
            _parse_target("https://user:secret@example.com/manifest.json")

    def test_rejects_control_character_request_injection(self):
        with self.assertRaisesRegex(SafeFetchError, "control characters"):
            _parse_target("https://example.com/manifest.json\r\nX-Injected: yes")

    def test_safe_origin_strips_path_query_and_fragment(self):
        self.assertEqual(
            safe_origin("https://Example.COM:8443/path?token=secret#fragment"),
            "https://example.com:8443",
        )

    @patch("integrations.safe_fetch.socket.getaddrinfo")
    def test_private_and_mixed_dns_answers_are_blocked(self, getaddrinfo):
        getaddrinfo.return_value = [
            (2, 1, 6, "", ("93.184.216.34", 443)),
            (2, 1, 6, "", ("127.0.0.1", 443)),
        ]

        with self.assertRaisesRegex(SafeFetchError, "non-public network address") as raised:
            _resolve_public_addresses("example.com", 443)

        self.assertEqual(raised.exception.code, "blocked_destination")

    @patch("integrations.safe_fetch.socket.getaddrinfo")
    def test_global_dns_answer_is_accepted(self, getaddrinfo):
        getaddrinfo.return_value = [
            (2, 1, 6, "", ("93.184.216.34", 443)),
            (2, 1, 6, "", ("93.184.216.34", 443)),
        ]

        self.assertEqual(
            _resolve_public_addresses("example.com", 443),
            ("93.184.216.34",),
        )

    @patch("integrations.safe_fetch._fetch_once")
    @patch("integrations.safe_fetch.socket.getaddrinfo")
    def test_redirect_to_private_address_is_revalidated_before_connection(
        self,
        getaddrinfo,
        fetch_once,
    ):
        def resolve(host, port, **_kwargs):
            if host == "example.com":
                return [(2, 1, 6, "", ("93.184.216.34", port))]
            return [(2, 1, 6, "", ("127.0.0.1", port))]

        getaddrinfo.side_effect = resolve
        fetch_once.return_value = (
            302,
            {"location": "https://127.0.0.1/internal"},
            b"",
        )

        with self.assertRaisesRegex(SafeFetchError, "non-public network address"):
            fetch_bytes("https://example.com/manifest.json")

        fetch_once.assert_called_once()

    @patch("integrations.safe_fetch.http.client.HTTPResponse")
    @patch("integrations.safe_fetch.ssl.create_default_context")
    @patch("integrations.safe_fetch.socket.create_connection")
    def test_fetch_once_connects_to_validated_ip_with_hostname_sni(
        self,
        create_connection,
        create_default_context,
        response_class,
    ):
        raw_socket = MagicMock()
        tls_socket = MagicMock()
        context = create_default_context.return_value
        context.wrap_socket.return_value = tls_socket
        create_connection.return_value = raw_socket
        response = response_class.return_value
        response.status = 200
        response.getheaders.return_value = [("Content-Type", "application/json")]
        response.getheader.side_effect = lambda name: {
            "Content-Length": "2",
            "Content-Encoding": None,
        }.get(name)
        response.read.return_value = b"{}"
        target = _parse_target("https://example.com/path?x=1")

        status, headers, body = _fetch_once(
            target,
            "93.184.216.34",
            2.0,
        )

        self.assertEqual(status, 200)
        self.assertEqual(headers["content-type"], "application/json")
        self.assertEqual(body, b"{}")
        create_connection.assert_called_once_with(("93.184.216.34", 443), timeout=2.0)
        context.wrap_socket.assert_called_once_with(raw_socket, server_hostname="example.com")
        sent_request = tls_socket.sendall.call_args.args[0].decode("ascii")
        self.assertIn("Host: example.com\r\n", sent_request)
        self.assertIn("GET /path?x=1 HTTP/1.1\r\n", sent_request)
        self.assertNotIn("Authorization:", sent_request)
        self.assertNotIn("Cookie:", sent_request)

    def test_response_size_and_compression_are_bounded(self):
        too_large = MagicMock()
        too_large.getheader.side_effect = lambda name: {
            "Content-Length": "1048577",
            "Content-Encoding": None,
        }.get(name)
        with self.assertRaisesRegex(SafeFetchError, "size limit"):
            _read_bounded(too_large)

        compressed = MagicMock()
        compressed.getheader.side_effect = lambda name: {
            "Content-Length": "2",
            "Content-Encoding": "gzip",
        }.get(name)
        with self.assertRaisesRegex(SafeFetchError, "Compressed remote responses"):
            _read_bounded(compressed)

    @patch("integrations.safe_fetch.fetch_bytes")
    def test_json_requires_json_content_type_and_bounded_depth(self, fetch):
        fetch.return_value = SafeFetchResult(
            status_code=200,
            content_type="text/html",
            body=b"{}",
            origin="https://example.com",
        )
        with self.assertRaisesRegex(SafeFetchError, "not JSON"):
            fetch_json("https://example.com/data")

        nested = {}
        cursor = nested
        for index in range(40):
            child = {"index": index}
            cursor["child"] = child
            cursor = child
        fetch.return_value = SafeFetchResult(
            status_code=200,
            content_type="application/json",
            body=__import__("json").dumps(nested).encode(),
            origin="https://example.com",
        )
        with self.assertRaisesRegex(SafeFetchError, "nested too deeply") as raised:
            fetch_json("https://example.com/data")
        self.assertEqual(raised.exception.code, "json_too_deep")
