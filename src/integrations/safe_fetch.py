"""Bounded outbound JSON fetching for untrusted declarative integrations."""

from __future__ import annotations

import http.client
import ipaddress
import json
import socket
import ssl
import time
from dataclasses import dataclass
from typing import Any, NoReturn
from urllib.parse import quote, urljoin, urlsplit

_MAX_URL_LENGTH = 2048
_MAX_HOST_LENGTH = 253
_MAX_PORT = 65_535
_HTTPS_PORT = 443
_MAX_REDIRECTS = 3
_MAX_BODY_BYTES = 1_048_576
_MAX_JSON_DEPTH = 32
_MAX_JSON_NODES = 20_000
_DEFAULT_TIMEOUT_SECONDS = 5.0
_MAX_TIMEOUT_SECONDS = 30.0
_HTTP_SUCCESS_MIN = 200
_HTTP_SUCCESS_MAX = 300
_REDIRECT_STATUSES = {301, 302, 303, 307, 308}
_JSON_CONTENT_TYPES = {"application/json", "text/json"}


@dataclass(frozen=True, slots=True)
class SafeFetchError(Exception):
    """Stable public-safe failure for an outbound integration request."""

    code: str
    message: str

    def __str__(self) -> str:
        """Return only the stable redacted public message."""
        return self.message


@dataclass(frozen=True, slots=True)
class SafeFetchResult:
    """One bounded remote response with secret-free provenance."""

    status_code: int
    content_type: str
    body: bytes
    origin: str


@dataclass(frozen=True, slots=True)
class _Target:
    scheme: str
    host: str
    port: int
    request_target: str
    origin: str


def _fail(code: str, message: str) -> NoReturn:
    """Raise one stable error without retaining low-level or secret-bearing details."""
    raise SafeFetchError(code=code, message=message) from None


def safe_origin(raw_url: str) -> str:
    """Return a display-safe origin without credentials, path, query, or fragment."""
    target = _parse_target(raw_url)
    return target.origin


def _ascii_hostname(host: str) -> str:
    """Return one normalized ASCII hostname or IP literal."""
    if "%" in host:
        _fail("invalid_url", "Remote hostname contains an unsupported zone identifier.")
    if ":" in host:
        try:
            ipaddress.ip_address(host)
        except ValueError:
            _fail("invalid_url", "Remote hostname is invalid.")
        return host.lower()
    try:
        return host.encode("idna").decode("ascii").lower()
    except UnicodeError:
        _fail("invalid_url", "Remote hostname is invalid.")


def _parse_target(raw_url: str) -> _Target:
    """Parse one HTTPS target while rejecting credential-bearing or ambiguous URLs."""
    if not isinstance(raw_url, str):
        _fail("invalid_url", "Remote URL must be a string.")
    raw_url = raw_url.strip()
    if not raw_url or len(raw_url) > _MAX_URL_LENGTH:
        _fail("invalid_url", "Remote URL is empty or too long.")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in raw_url):
        _fail("invalid_url", "Remote URL contains control characters.")

    try:
        parsed = urlsplit(raw_url)
        port = parsed.port
    except ValueError:
        _fail("invalid_url", "Remote URL is invalid.")

    if parsed.scheme.lower() != "https":
        _fail("unsupported_scheme", "Remote integrations must use HTTPS.")
    if parsed.username is not None or parsed.password is not None:
        _fail("credentials_in_url", "Remote URLs must not contain credentials.")
    host = (parsed.hostname or "").strip().rstrip(".")
    if not host:
        _fail("invalid_url", "Remote URL must include a hostname.")
    host = _ascii_hostname(host)
    if len(host) > _MAX_HOST_LENGTH:
        _fail("invalid_url", "Remote hostname is too long.")

    port = port or _HTTPS_PORT
    if port < 1 or port > _MAX_PORT:
        _fail("invalid_url", "Remote URL port is invalid.")

    path = quote(parsed.path or "/", safe="/%:@!$&'()*+,;=-._~")
    request_target = path
    if parsed.query:
        query = quote(parsed.query, safe="=&%:@!$'()*+,;/?-._~")
        request_target = f"{path}?{query}"
    display_host = f"[{host}]" if ":" in host else host
    origin = f"https://{display_host}" + (
        f":{port}" if port != _HTTPS_PORT else ""
    )
    return _Target(
        scheme="https",
        host=host,
        port=port,
        request_target=request_target,
        origin=origin,
    )


def _is_public_address(value: str) -> bool:
    """Return True only for globally routable IP addresses."""
    try:
        address = ipaddress.ip_address(value)
    except ValueError:
        return False
    return address.is_global


def _resolve_public_addresses(host: str, port: int) -> tuple[str, ...]:
    """Resolve a hostname and reject the whole answer if any target is non-public."""
    try:
        answers = socket.getaddrinfo(
            host,
            port,
            family=socket.AF_UNSPEC,
            type=socket.SOCK_STREAM,
        )
    except OSError:
        _fail("dns_failure", "Remote hostname could not be resolved.")

    addresses = []
    for answer in answers:
        address = str(answer[4][0])
        if address not in addresses:
            addresses.append(address)
    if not addresses:
        _fail("dns_failure", "Remote hostname did not resolve to an address.")
    if any(not _is_public_address(address) for address in addresses):
        _fail(
            "blocked_destination",
            "Remote hostname resolves to a non-public network address.",
        )
    return tuple(addresses)


def _host_header(target: _Target) -> str:
    display_host = f"[{target.host}]" if ":" in target.host else target.host
    return (
        display_host
        if target.port == _HTTPS_PORT
        else f"{display_host}:{target.port}"
    )


def _read_bounded(response: http.client.HTTPResponse) -> bytes:
    """Read at most the configured response budget."""
    length = response.getheader("Content-Length")
    if length:
        try:
            parsed_length = int(length)
        except ValueError:
            _fail("invalid_response", "Remote response has an invalid content length.")
        if parsed_length > _MAX_BODY_BYTES:
            _fail("response_too_large", "Remote response exceeds the size limit.")
    encoding = (response.getheader("Content-Encoding") or "identity").strip().lower()
    if encoding not in {"", "identity"}:
        _fail(
            "unsupported_content_encoding",
            "Compressed remote responses are not accepted by this integration boundary.",
        )
    body = response.read(_MAX_BODY_BYTES + 1)
    if len(body) > _MAX_BODY_BYTES:
        _fail("response_too_large", "Remote response exceeds the size limit.")
    return body


def _fetch_once(target: _Target, address: str, timeout: float):
    """Connect to the validated IP and issue one HTTPS GET with verified SNI."""
    raw_socket = None
    tls_socket = None
    try:
        raw_socket = socket.create_connection((address, target.port), timeout=timeout)
        raw_socket.settimeout(timeout)
        context = ssl.create_default_context()
        tls_socket = context.wrap_socket(raw_socket, server_hostname=target.host)
        raw_socket = None
        request = (
            f"GET {target.request_target} HTTP/1.1\r\n"
            f"Host: {_host_header(target)}\r\n"
            "Accept: application/json\r\n"
            "Accept-Encoding: identity\r\n"
            "User-Agent: Floppy-SafeFetch/1\r\n"
            "Connection: close\r\n"
            "\r\n"
        ).encode("ascii")
        tls_socket.sendall(request)
        response = http.client.HTTPResponse(tls_socket)
        response.begin()
        body = _read_bounded(response)
        headers = {key.lower(): value for key, value in response.getheaders()}
        return response.status, headers, body
    except SafeFetchError:
        raise
    except (OSError, ssl.SSLError, http.client.HTTPException):
        _fail("connection_failed", "Remote request failed.")
    finally:
        if tls_socket is not None:
            tls_socket.close()
        if raw_socket is not None:
            raw_socket.close()


def fetch_bytes(
    raw_url: str,
    *,
    timeout_seconds: float = _DEFAULT_TIMEOUT_SECONDS,
    max_redirects: int = _MAX_REDIRECTS,
) -> SafeFetchResult:
    """Fetch one bounded HTTPS resource without DNS-rebinding or credential forwarding."""
    if timeout_seconds <= 0 or timeout_seconds > _MAX_TIMEOUT_SECONDS:
        _fail("invalid_timeout", "Remote timeout is outside the allowed range.")
    if max_redirects < 0 or max_redirects > _MAX_REDIRECTS:
        _fail(
            "invalid_redirect_limit",
            "Remote redirect limit is outside the allowed range.",
        )

    deadline = time.monotonic() + timeout_seconds
    current_url = raw_url
    for redirect_count in range(max_redirects + 1):
        target = _parse_target(current_url)
        addresses = _resolve_public_addresses(target.host, target.port)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            _fail("request_timeout", "Remote request exceeded the time limit.")
        status, headers, body = _fetch_once(
            target,
            addresses[0],
            min(remaining, timeout_seconds),
        )
        if status in _REDIRECT_STATUSES:
            if redirect_count >= max_redirects:
                _fail(
                    "too_many_redirects",
                    "Remote request exceeded the redirect limit.",
                )
            location = headers.get("location")
            if not location:
                _fail(
                    "invalid_redirect",
                    "Remote redirect did not include a location.",
                )
            current_url = urljoin(current_url, location)
            continue
        if status < _HTTP_SUCCESS_MIN or status >= _HTTP_SUCCESS_MAX:
            _fail(
                "remote_http_error",
                "Remote server returned an unsuccessful status.",
            )
        content_type = headers.get("content-type", "").split(";", 1)[0].strip().lower()
        return SafeFetchResult(
            status_code=status,
            content_type=content_type,
            body=body,
            origin=target.origin,
        )

    _fail("too_many_redirects", "Remote request exceeded the redirect limit.")


def _validate_json_complexity(value: Any) -> None:
    """Reject extremely deep or wide JSON after the byte budget has been enforced."""
    stack = [(value, 1)]
    nodes = 0
    while stack:
        current, depth = stack.pop()
        nodes += 1
        if nodes > _MAX_JSON_NODES:
            _fail("json_too_complex", "Remote JSON contains too many values.")
        if depth > _MAX_JSON_DEPTH:
            _fail("json_too_deep", "Remote JSON is nested too deeply.")
        if isinstance(current, dict):
            stack.extend((child, depth + 1) for child in current.values())
        elif isinstance(current, list):
            stack.extend((child, depth + 1) for child in current)


def fetch_json(
    raw_url: str,
    *,
    timeout_seconds: float = _DEFAULT_TIMEOUT_SECONDS,
) -> tuple[Any, SafeFetchResult]:
    """Fetch and parse bounded JSON from an untrusted public integration endpoint."""
    result = fetch_bytes(raw_url, timeout_seconds=timeout_seconds)
    content_type = result.content_type
    if content_type not in _JSON_CONTENT_TYPES and not content_type.endswith("+json"):
        _fail("invalid_content_type", "Remote response is not JSON.")
    try:
        payload = json.loads(result.body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        _fail("invalid_json", "Remote response contains invalid JSON.")
    _validate_json_complexity(payload)
    return payload, result
