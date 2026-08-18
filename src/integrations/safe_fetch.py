"""Bounded outbound JSON fetching for untrusted declarative integrations."""

from __future__ import annotations

import http.client
import ipaddress
import json
import socket
import ssl
import time
from dataclasses import dataclass
from typing import Any
from urllib.parse import urljoin, urlsplit

_MAX_URL_LENGTH = 2048
_MAX_REDIRECTS = 3
_MAX_BODY_BYTES = 1_048_576
_MAX_JSON_DEPTH = 32
_MAX_JSON_NODES = 20_000
_DEFAULT_TIMEOUT_SECONDS = 5.0
_REDIRECT_STATUSES = {301, 302, 303, 307, 308}
_JSON_CONTENT_TYPES = {"application/json", "text/json"}


@dataclass(frozen=True, slots=True)
class SafeFetchError(Exception):
    """Stable public-safe failure for an outbound integration request."""

    code: str
    message: str

    def __str__(self) -> str:
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


def _error(code: str, message: str) -> SafeFetchError:
    return SafeFetchError(code=code, message=message)


def safe_origin(raw_url: str) -> str:
    """Return a display-safe origin without credentials, path, query, or fragment."""
    target = _parse_target(raw_url)
    return target.origin


def _parse_target(raw_url: str) -> _Target:
    """Parse one HTTPS target while rejecting credential-bearing or ambiguous URLs."""
    if not isinstance(raw_url, str):
        raise _error("invalid_url", "Remote URL must be a string.")
    raw_url = raw_url.strip()
    if not raw_url or len(raw_url) > _MAX_URL_LENGTH:
        raise _error("invalid_url", "Remote URL is empty or too long.")

    try:
        parsed = urlsplit(raw_url)
        port = parsed.port
    except ValueError as exc:
        raise _error("invalid_url", "Remote URL is invalid.") from exc

    if parsed.scheme.lower() != "https":
        raise _error("unsupported_scheme", "Remote integrations must use HTTPS.")
    if parsed.username is not None or parsed.password is not None:
        raise _error("credentials_in_url", "Remote URLs must not contain credentials.")
    host = (parsed.hostname or "").strip().rstrip(".")
    if not host:
        raise _error("invalid_url", "Remote URL must include a hostname.")
    if len(host) > 253:
        raise _error("invalid_url", "Remote hostname is too long.")

    port = port or 443
    if port < 1 or port > 65535:
        raise _error("invalid_url", "Remote URL port is invalid.")

    path = parsed.path or "/"
    request_target = path
    if parsed.query:
        request_target = f"{path}?{parsed.query}"
    display_host = f"[{host}]" if ":" in host else host
    origin = f"https://{display_host}" + (f":{port}" if port != 443 else "")
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
    except OSError as exc:
        raise _error("dns_failure", "Remote hostname could not be resolved.") from exc

    addresses = []
    for answer in answers:
        address = str(answer[4][0])
        if address not in addresses:
            addresses.append(address)
    if not addresses:
        raise _error("dns_failure", "Remote hostname did not resolve to an address.")
    if any(not _is_public_address(address) for address in addresses):
        raise _error(
            "blocked_destination",
            "Remote hostname resolves to a non-public network address.",
        )
    return tuple(addresses)


def _host_header(target: _Target) -> str:
    display_host = f"[{target.host}]" if ":" in target.host else target.host
    return display_host if target.port == 443 else f"{display_host}:{target.port}"


def _read_bounded(response: http.client.HTTPResponse) -> bytes:
    """Read at most the configured response budget."""
    length = response.getheader("Content-Length")
    if length:
        try:
            if int(length) > _MAX_BODY_BYTES:
                raise _error("response_too_large", "Remote response exceeds the size limit.")
        except ValueError:
            raise _error("invalid_response", "Remote response has an invalid content length.") from None
    encoding = (response.getheader("Content-Encoding") or "identity").strip().lower()
    if encoding not in {"", "identity"}:
        raise _error(
            "unsupported_content_encoding",
            "Compressed remote responses are not accepted by this integration boundary.",
        )
    body = response.read(_MAX_BODY_BYTES + 1)
    if len(body) > _MAX_BODY_BYTES:
        raise _error("response_too_large", "Remote response exceeds the size limit.")
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
        return response.status, dict(response.getheaders()), body
    except SafeFetchError:
        raise
    except (OSError, ssl.SSLError, http.client.HTTPException) as exc:
        raise _error("connection_failed", "Remote request failed.") from exc
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
    if timeout_seconds <= 0 or timeout_seconds > 30:
        raise _error("invalid_timeout", "Remote timeout is outside the allowed range.")
    if max_redirects < 0 or max_redirects > _MAX_REDIRECTS:
        raise _error("invalid_redirect_limit", "Remote redirect limit is outside the allowed range.")

    deadline = time.monotonic() + timeout_seconds
    current_url = raw_url
    for redirect_count in range(max_redirects + 1):
        target = _parse_target(current_url)
        addresses = _resolve_public_addresses(target.host, target.port)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise _error("request_timeout", "Remote request exceeded the time limit.")
        status, headers, body = _fetch_once(
            target,
            addresses[0],
            min(remaining, timeout_seconds),
        )
        if status in _REDIRECT_STATUSES:
            if redirect_count >= max_redirects:
                raise _error("too_many_redirects", "Remote request exceeded the redirect limit.")
            location = headers.get("Location") or headers.get("location")
            if not location:
                raise _error("invalid_redirect", "Remote redirect did not include a location.")
            current_url = urljoin(current_url, location)
            continue
        if status < 200 or status >= 300:
            raise _error("remote_http_error", "Remote server returned an unsuccessful status.")
        content_type = (headers.get("Content-Type") or headers.get("content-type") or "").split(";", 1)[0].strip().lower()
        return SafeFetchResult(
            status_code=status,
            content_type=content_type,
            body=body,
            origin=target.origin,
        )

    raise _error("too_many_redirects", "Remote request exceeded the redirect limit.")


def _validate_json_complexity(value: Any) -> None:
    """Reject extremely deep or wide JSON after the byte budget has been enforced."""
    stack = [(value, 1)]
    nodes = 0
    while stack:
        current, depth = stack.pop()
        nodes += 1
        if nodes > _MAX_JSON_NODES:
            raise _error("json_too_complex", "Remote JSON contains too many values.")
        if depth > _MAX_JSON_DEPTH:
            raise _error("json_too_deep", "Remote JSON is nested too deeply.")
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
        raise _error("invalid_content_type", "Remote response is not JSON.")
    try:
        payload = json.loads(result.body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise _error("invalid_json", "Remote response contains invalid JSON.") from exc
    _validate_json_complexity(payload)
    return payload, result
