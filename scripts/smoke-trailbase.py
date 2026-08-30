#!/usr/bin/env python3
"""Run the hermetic TrailBase v0.33.5 account-lifecycle fixture."""

from __future__ import annotations

import argparse
import base64
import email
import hashlib
import hmac
import http.cookiejar
import http.server
import json
import os
import queue
import re
import socketserver
import struct
import subprocess  # nosec B404 -- this hermetic conformance runner must launch the pinned fixture.
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

import trailbase_runtime as runtime


JWT = re.compile(r"\beyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b")
OIDC_CALLBACK = "http://127.0.0.1:24500/api/auth/v1/oauth/oidc0/callback"


class SmtpHandler(socketserver.StreamRequestHandler):
    def handle(self) -> None:
        self.wfile.write(b"220 fasti.test ESMTP\r\n")
        data_mode = False
        message = bytearray()
        while line := self.rfile.readline(1024 * 1024):
            if data_mode:
                if line == b".\r\n":
                    self.server.messages.put(bytes(message))  # type: ignore[attr-defined]
                    self.wfile.write(b"250 queued\r\n")
                    data_mode = False
                    message.clear()
                else:
                    message.extend(line[1:] if line.startswith(b"..") else line)
                continue
            command = line.split(maxsplit=1)[0].upper()
            if command in {b"EHLO", b"HELO"}:
                self.wfile.write(b"250-fasti.test\r\n250 8BITMIME\r\n")
            elif command in {b"MAIL", b"RCPT", b"RSET", b"NOOP"}:
                self.wfile.write(b"250 ok\r\n")
            elif command == b"DATA":
                self.wfile.write(b"354 end with dot\r\n")
                data_mode = True
            elif command == b"QUIT":
                self.wfile.write(b"221 bye\r\n")
                return
            else:
                self.wfile.write(b"502 unsupported\r\n")


class SmtpServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, address: tuple[str, int]):
        self.messages: queue.Queue[bytes] = queue.Queue()
        super().__init__(address, SmtpHandler)


class OidcHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:  # pylint: disable=redefined-builtin
        del format, args
        return

    def send_json(self, status: int, payload: dict[str, object]) -> None:
        body = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        parsed = urllib.parse.urlsplit(self.path)
        if parsed.path == "/authorize":
            query = urllib.parse.parse_qs(parsed.query)
            required = {
                "client_id": "fixture-client",
                "redirect_uri": OIDC_CALLBACK,
                "response_type": "code",
                "code_challenge_method": "S256",
            }
            if any(query.get(key, [""])[0] != value for key, value in required.items()):
                self.send_error(400)
                return
            if not {"openid", "email", "profile"}.issubset(
                set(query.get("scope", [""])[0].split())
            ):
                self.send_error(400)
                return
            server = self.server  # type: ignore[assignment]
            code = f"fixture-code-{server.next_code}"  # type: ignore[attr-defined]
            server.next_code += 1  # type: ignore[attr-defined]
            server.codes[code] = (  # type: ignore[attr-defined]
                query["code_challenge"][0],
                dict(server.profile),  # type: ignore[attr-defined]
            )
            location = OIDC_CALLBACK + "?" + urllib.parse.urlencode(
                {"code": code, "state": query["state"][0]}
            )
            self.send_response(302)
            self.send_header("Location", location)
            self.end_headers()
            return
        if parsed.path == "/userinfo":
            token = self.headers.get("Authorization", "").removeprefix("Bearer ")
            profile = self.server.tokens.get(token)  # type: ignore[attr-defined]
            if profile is None:
                self.send_error(401)
            else:
                self.send_json(200, profile)
            return
        self.send_error(404)

    def do_POST(self) -> None:
        if urllib.parse.urlsplit(self.path).path != "/token":
            self.send_error(404)
            return
        if self.server.outage:  # type: ignore[attr-defined]
            self.server.outage_observed = True  # type: ignore[attr-defined]
            self.send_error(503)
            return
        expected_auth = "Basic " + base64.b64encode(b"fixture-client:fixture-secret").decode()
        length = int(self.headers.get("Content-Length", "0"))
        form = urllib.parse.parse_qs(self.rfile.read(length).decode())
        code = form.get("code", [""])[0]
        pending = self.server.codes.pop(code, None)  # type: ignore[attr-defined]
        verifier = form.get("code_verifier", [""])[0]
        challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
        if (
            self.headers.get("Authorization") != expected_auth
            or form.get("grant_type", [""])[0] != "authorization_code"
            or pending is None
            or not hmac.compare_digest(challenge, pending[0])
        ):
            self.send_error(400)
            return
        token = f"fixture-access-{code}"
        self.server.tokens[token] = pending[1]  # type: ignore[attr-defined]
        self.send_json(
            200,
            {"access_token": token, "token_type": "Bearer", "expires_in": 300},  # nosec B105 -- OAuth token type, not a password.
        )


class OidcServer(http.server.ThreadingHTTPServer):
    def __init__(self, address: tuple[str, int]):
        self.next_code = 1
        self.codes: dict[str, tuple[str, dict[str, object]]] = {}
        self.tokens: dict[str, dict[str, object]] = {}
        self.profile: dict[str, object] = {}
        self.outage = False
        self.outage_observed = False
        super().__init__(address, OidcHandler)


def request(
    base_url: str,
    method: str,
    path: str,
    payload: dict[str, object] | None = None,
    token: str | None = None,
) -> tuple[int, bytes]:
    headers: dict[str, str] = {}
    body = None
    if payload is not None:
        headers["Content-Type"] = "application/json"
        body = json.dumps(payload).encode()
    if token:
        headers["Authorization"] = f"Bearer {token}"
    value = urllib.request.Request(base_url + path, body, headers, method=method)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        with opener.open(value, timeout=5) as response:
            return response.status, response.read(1024 * 1024)
    except urllib.error.HTTPError as error:
        return error.code, error.read(1024 * 1024)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args: object, **_kwargs: object) -> None:
        return None


def redirect_response(url: str) -> tuple[int, str | None]:
    opener = urllib.request.build_opener(
        urllib.request.ProxyHandler({}),
        NoRedirect(),
    )
    try:
        with opener.open(url, timeout=5) as response:
            return response.status, response.headers.get("Location")
    except urllib.error.HTTPError as error:
        return error.code, error.headers.get("Location")


def browser() -> urllib.request.OpenerDirector:
    return urllib.request.build_opener(
        urllib.request.ProxyHandler({}),
        urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()),
    )


def browser_get(opener: urllib.request.OpenerDirector, url: str) -> tuple[int, bytes]:
    try:
        with opener.open(url, timeout=5) as response:
            return response.status, response.read(1024 * 1024)
    except urllib.error.HTTPError as error:
        return error.code, error.read(1024 * 1024)


def json_object(body: bytes) -> dict[str, object]:
    value = json.loads(body)
    if not isinstance(value, dict):
        raise AssertionError("TrailBase response is not a JSON object")
    return value


def jwt_parts(token: str) -> tuple[dict[str, object], dict[str, object]]:
    parts = token.split(".")
    if len(parts) != 3:
        raise AssertionError("TrailBase token is not a compact three-part JWT")

    def decode(value: str) -> dict[str, object]:
        payload = base64.urlsafe_b64decode(value + "=" * ((4 - len(value) % 4) % 4))
        return json_object(payload)

    return decode(parts[0]), decode(parts[1])


def message_text(raw: bytes) -> str:
    parsed = email.message_from_bytes(raw)
    if parsed.is_multipart():
        return "\n".join(
            part.get_payload(decode=True).decode(errors="replace")
            for part in parsed.walk()
            if part.get_content_maintype() == "text" and not part.is_multipart()
        )
    payload = parsed.get_payload(decode=True)
    return (payload or b"").decode(errors="replace")


def email_token(messages: queue.Queue[bytes]) -> str:
    matches = set(JWT.findall(message_text(messages.get(timeout=10))))
    if len(matches) != 1:
        raise AssertionError("fixture email does not contain exactly one token")
    return matches.pop()


def totp_code(url: str) -> str:
    query = urllib.parse.parse_qs(urllib.parse.urlsplit(url).query)
    secret = query.get("secret", [""])[0]
    key = base64.b32decode(secret.upper() + "=" * ((8 - len(secret) % 8) % 8))
    counter = int(time.time()) // 30
    digest = hmac.new(key, struct.pack(">Q", counter), hashlib.sha1).digest()
    offset = digest[-1] & 15
    value = struct.unpack(">I", digest[offset : offset + 4])[0] & 0x7FFFFFFF
    return f"{value % 1_000_000:06d}"


def assert_status(actual: int, expected: int, label: str, checks: list[dict[str, object]]) -> None:
    if actual != expected:
        raise AssertionError(f"{label}: expected HTTP {expected}, got {actual}")
    checks.append({"id": label, "status": "pass", "http_status": actual})


def stop_process(process: subprocess.Popen[bytes], reader: threading.Thread | None = None) -> None:
    if process.poll() is None:
        os.killpg(process.pid, 15)
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, 9)
            process.wait(timeout=5)
    if reader is not None:
        reader.join(timeout=2)


def start_fixture_release(
    executable: Path,
    root: Path,
    port: int,
    initial_password: list[str] | None = None,
) -> tuple[subprocess.Popen[bytes], threading.Thread | None]:
    environment = {key: value for key, value in os.environ.items() if not key.startswith("TRAIL_")}
    environment["RUST_LOG"] = "info" if initial_password is not None else "warn"
    output: int | None = subprocess.PIPE if initial_password is not None else subprocess.DEVNULL
    old_umask = os.umask(0o077)
    try:
        process = subprocess.Popen(  # nosec -- exact digest-verified fixture; fixed local argv, no shell.
            [  # nosemgrep -- exact digest-verified executable and fixed loopback arguments.
                executable,
                "--depot",
                root / "depot",
                "--public-url",
                f"http://127.0.0.1:{port}",
                "run",
                "--address",
                f"127.0.0.1:{port}",
                "--admin-address",
                f"127.0.0.1:{port + 1}",
                "--cors-allowed-origins",
                f"http://127.0.0.1:{port}",
                "--runtime-threads",
                "1",
                "--stderr-logging",
            ],
            env=environment,
            stdout=output,
            stderr=subprocess.STDOUT if initial_password is not None else subprocess.DEVNULL,
            start_new_session=True,
        )
    finally:
        os.umask(old_umask)
    reader = None
    if initial_password is not None:
        output_stream = process.stdout
        if output_stream is None:
            stop_process(process)
            raise AssertionError("TrailBase bootstrap fixture has no output pipe")

        def read_bootstrap() -> None:
            for line in output_stream:
                match = runtime.BOOTSTRAP_PASSWORD.fullmatch(line.decode(errors="replace").rstrip())
                if match and not initial_password:
                    initial_password.append(match.group(1))

        reader = threading.Thread(target=read_bootstrap, daemon=True)
        reader.start()
    base = f"http://127.0.0.1:{port}"
    for _ in range(300):
        if process.poll() is not None:
            stop_process(process, reader)
            raise AssertionError("TrailBase upgrade fixture exited during startup")
        try:
            status, body = request(base, "GET", "/api/healthcheck")
        except urllib.error.URLError:
            time.sleep(0.1)
            continue
        if status == 200 and body == b"Ok" and (initial_password is None or initial_password):
            return process, reader
        time.sleep(0.1)
    stop_process(process, reader)
    raise AssertionError("TrailBase upgrade fixture liveness timed out")


def write_bootstrap_receipt(root: Path, version: str) -> None:
    path = root / "bootstrap.json"
    temporary = root / ".bootstrap.json.tmp"
    temporary.write_text(
        json.dumps(
            {
                "schema_version": "fasti.trailbase-bootstrap.v1",
                "release": version,
                "admin": "admin@localhost",
                "initial_password_rotated": True,  # nosec B105 -- boolean receipt field, not a password.
                "completed_at": "2026-08-30T00:00:00+00:00",
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    os.chmod(temporary, 0o600)
    temporary.replace(path)


def run_upgrade_fixture(
    source_root: Path,
    fixture: Path,
    smtp: SmtpServer,
    current_executable: Path,
    checks: list[dict[str, object]],
) -> dict[str, object]:
    release = runtime.load_release()
    old_version = str(release["upgrade_fixture"]["version"])
    current_version = str(release["version"])
    old_executable = runtime.prepare_upgrade_fixture(source_root, offline=True)
    old_root = fixture / "upgrade-old"
    old_depot = old_root / "depot"
    old_depot.mkdir(mode=0o700, parents=True)  # nosec B103 -- owner-only is the required mode.
    os.chmod(old_root, 0o700)  # nosec B103 -- nosemgrep -- owner-only is required.
    old_config = '''email {
  smtp_host: "127.0.0.1"
  smtp_port: 24525
  smtp_encryption: SMTP_ENCRYPTION_NONE
  sender_name: "Fasti TrailBase Upgrade Test"
  sender_address: "noreply@fasti.test"
}
server {
  application_name: "Fasti TrailBase Upgrade Test"
  site_url: "http://127.0.0.1:24510"
}
auth {
  auth_token_ttl_sec: 300
  refresh_token_ttl_sec: 3600
  password_minimal_length: 12
  password_must_contain_upper_and_lower_case: true
  password_must_contain_digits: true
  password_must_contain_special_characters: true
}
jobs {}
'''
    (old_depot / "config.textproto").write_text(old_config, encoding="utf-8")
    os.chmod(old_depot / "config.textproto", 0o600)
    (old_depot / "secrets").mkdir(mode=0o700)
    (old_depot / "secrets/secrets.textproto").write_text("", encoding="utf-8")
    os.chmod(old_depot / "secrets/secrets.textproto", 0o600)
    starts: list[dict[str, str]] = []

    initial_password: list[str] = []
    try:
        smtp.messages.get_nowait()
    except queue.Empty:
        pass
    else:
        raise AssertionError("unexpected fixture email remained before the upgrade check")
    process, reader = start_fixture_release(old_executable, old_root, 24510, initial_password)
    try:
        admin_password = "UpgradeAdmin4!Fixture"  # nosec B105 -- local conformance fixture only.
        status, body = request(
            "http://127.0.0.1:24510",
            "POST",
            "/api/auth/v1/login",
            {"email": "admin@localhost", "password": initial_password[0]},
        )
        if status != 200:
            raise AssertionError("prior-release administrator bootstrap login failed")
        admin_token = str(json_object(body)["auth_token"])
        status, _ = request(
            "http://127.0.0.1:24510",
            "POST",
            "/api/auth/v1/change_password",
            {
                "old_password": initial_password[0],
                "new_password": admin_password,
                "new_password_repeat": admin_password,
            },
            admin_token,
        )
        if status != 200:
            raise AssertionError("prior-release administrator credential rotation failed")
        initial_password.clear()
        password = "Upgrade5!Fixture"  # nosec B105 -- local conformance fixture only.
        status, _ = request(
            "http://127.0.0.1:24510",
            "POST",
            "/api/auth/v1/register",
            {"email": "upgrade@fasti.test", "password": password, "password_repeat": password},
        )
        if status != 200:
            raise AssertionError("prior-release sentinel registration failed")
        verification = email_token(smtp.messages)
        status, _ = request(
            "http://127.0.0.1:24510",
            "GET",
            f"/api/auth/v1/verify_email/confirm/{verification}",
        )
        if status != 200:
            raise AssertionError("prior-release sentinel verification failed")
    finally:
        initial_password.clear()
        stop_process(process, reader)
    write_bootstrap_receipt(old_root, old_version)
    runtime.verify_private_root(old_root, old_version)
    backup, backup_sha256 = runtime.backup_depot(
        old_root,
        fixture / "upgrade-backups",
        old_version,
    )

    def verify_login(executable: Path, root: Path, version: str) -> None:
        process, reader = start_fixture_release(executable, root, 24510)
        try:
            status, _ = request(
                "http://127.0.0.1:24510",
                "POST",
                "/api/auth/v1/login",
                {"email": "upgrade@fasti.test", "password": "Upgrade5!Fixture"},  # nosec B105 -- local conformance fixture only.
            )
            if status != 200:
                raise AssertionError(f"TrailBase v{version} did not preserve the sentinel account")
            starts.append(
                {
                    "version": version,
                    "binary_sha256": runtime.sha256_file(executable),
                    "root": str(root.resolve()),
                    "result": "started_stopped_account_verified",
                }
            )
        finally:
            stop_process(process, reader)

    upgrade_root = fixture / "upgrade-target"
    runtime.restore_depot(backup, upgrade_root, old_version)
    verify_login(current_executable, upgrade_root, current_version)
    verify_login(current_executable, upgrade_root, current_version)
    write_bootstrap_receipt(upgrade_root, current_version)
    runtime.verify_private_root(upgrade_root, current_version)
    checks.append({"id": "adjacent_version_upgrade", "status": "pass"})

    rollback_root = fixture / "rollback-target"
    runtime.restore_depot(backup, rollback_root, old_version)
    verify_login(old_executable, rollback_root, old_version)
    verify_login(old_executable, rollback_root, old_version)
    runtime.verify_private_root(rollback_root, old_version)
    if any(
        start["version"] == old_version and start["root"] == str(upgrade_root.resolve())
        for start in starts
    ):
        raise AssertionError("old TrailBase binary was started against the upgraded depot")
    checks.append({"id": "old_binary_full_depot_rollback", "status": "pass"})
    return {
        "status": "verified",
        "from_version": old_version,
        "to_version": current_version,
        "schema_migration_expected": False,
        "scope": "adjacent-version artifact replacement and full-depot rollback",
        "backup_sha256": backup_sha256,
        "starts": starts,
    }


def run_fixture(source_root: Path, receipt_path: Path) -> None:
    started = time.monotonic()
    receipt_path.unlink(missing_ok=True)
    runtime.verify_private_root(source_root)
    executable = runtime.prepare_native(source_root, offline=True)
    checks: list[dict[str, object]] = []
    with tempfile.TemporaryDirectory(prefix="fasti-trailbase-conformance-") as directory:
        fixture = Path(directory)
        os.chmod(fixture, 0o700)  # nosec B103 -- nosemgrep -- owner-only is required.
        backup, backup_digest = runtime.backup_depot(source_root, fixture / "backups")
        test_root = fixture / "root"
        runtime.restore_depot(backup, test_root)
        config = '''email {
  smtp_host: "127.0.0.1"
  smtp_port: 24525
  smtp_encryption: SMTP_ENCRYPTION_NONE
  sender_name: "Fasti TrailBase Test"
  sender_address: "noreply@fasti.test"
}
server {
  application_name: "Fasti TrailBase Test"
  site_url: "http://127.0.0.1:24500"
  logs_retention_sec: 3600
}
auth {
  auth_token_ttl_sec: 300
  refresh_token_ttl_sec: 3600
  password_minimal_length: 12
  password_must_contain_upper_and_lower_case: true
  password_must_contain_digits: true
  password_must_contain_special_characters: true
  oauth_providers: [{
    key: "oidc0"
    value {
      client_id: "fixture-client"
      client_secret: "fixture-secret"
      provider_id: OIDC0
      display_name: "Local OIDC conformance provider"
      auth_url: "http://127.0.0.1:24526/authorize"
      token_url: "http://127.0.0.1:24526/token"
      user_api_url: "http://127.0.0.1:24526/userinfo"
    }
  }]
}
jobs {}
'''
        (test_root / "depot/config.textproto").write_text(config, encoding="utf-8")
        os.chmod(test_root / "depot/config.textproto", 0o600)

        smtp = SmtpServer(("127.0.0.1", 24525))
        smtp_thread = threading.Thread(target=smtp.serve_forever, daemon=True)
        smtp_thread.start()
        oidc = OidcServer(("127.0.0.1", 24526))
        oidc_thread = threading.Thread(target=oidc.serve_forever, daemon=True)
        oidc_thread.start()
        environment = {key: value for key, value in os.environ.items() if not key.startswith("TRAIL_")}
        environment["RUST_LOG"] = "warn"
        command = [
            executable,
            "--depot",
            test_root / "depot",
            "--public-url",
            "http://127.0.0.1:24500",
            "run",
            "--address",
            "127.0.0.1:24500",
            "--admin-address",
            "127.0.0.1:24501",
            "--cors-allowed-origins",
            "http://127.0.0.1:24500",
            "--runtime-threads",
            "1",
        ]

        def spawn() -> subprocess.Popen[bytes]:
            old_umask = os.umask(0o077)
            old_affinity = os.sched_getaffinity(0)
            try:
                os.sched_setaffinity(0, {min(old_affinity)})
                return subprocess.Popen(  # nosec -- digest-verified binary; fixed loopback argv, no shell.
                    command,  # nosemgrep -- exact executable and fixed loopback argument vector.
                    env=environment,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    start_new_session=True,
                )
            finally:
                os.sched_setaffinity(0, old_affinity)
                os.umask(old_umask)

        process = spawn()
        try:
            for _ in range(100):
                if process.poll() is not None:
                    raise AssertionError("TrailBase exited during conformance startup")
                try:
                    status, body = request(
                        "http://127.0.0.1:24500", "GET", "/api/healthcheck"
                    )
                except urllib.error.URLError:
                    time.sleep(0.1)
                    continue
                if status == 200 and body == b"Ok":
                    break
                time.sleep(0.1)
            else:
                raise AssertionError("TrailBase conformance liveness timed out")

            base = "http://127.0.0.1:24500"
            status, body = request(base, "GET", "/api/auth/v1/oauth/providers")
            assert_status(status, 200, "social_provider_inventory", checks)
            if json_object(body).get("providers") != [
                ["oidc0", "Local OIDC conformance provider"]
            ]:
                raise AssertionError("configured social provider inventory drifted")
            for unsafe_redirect in (
                "//attacker.invalid/cb",
                r"/\attacker.invalid/cb",
            ):
                status, location = redirect_response(
                    f"{base}/api/auth/v1/logout?"
                    + urllib.parse.urlencode({"redirect_uri": unsafe_redirect})
                )
                if status != 303 or location != unsafe_redirect:
                    raise AssertionError(
                        "TrailBase redirect behavior changed; review the remote-exposure block"
                    )
            checks.append(
                {
                    "id": "unsafe_protocol_relative_redirect_observed",
                    "status": "pass",
                    "observed_limit": (
                        "TrailBase v0.33.5 accepts protocol-relative redirect values; "
                        "remote account and OAuth route exposure is blocked"
                    ),
                }
            )
            password = "Initial1!Fixture"  # nosec B105 -- local conformance fixture only.
            changed = "Changed2!Fixture"
            reset = "Reset3!Fixture"
            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/register",
                {"email": "weak@fasti.test", "password": "weak", "password_repeat": "weak"},  # nosec B105 -- intentional policy-rejection fixture.
            )
            assert_status(status, 400, "registration_policy_rejects_weak_password", checks)
            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/register",
                {"email": "person@fasti.test", "password": password, "password_repeat": password},
            )
            assert_status(status, 200, "registration", checks)
            verification = email_token(smtp.messages)
            status, _ = request(
                base, "GET", f"/api/auth/v1/verify_email/confirm/{verification}"
            )
            assert_status(status, 200, "email_verification", checks)

            status, body = request(
                base,
                "POST",
                "/api/auth/v1/login",
                {"email": "person@fasti.test", "password": password},
            )
            assert_status(status, 200, "password_login", checks)
            login = json_object(body)
            auth_token = str(login["auth_token"])
            refresh_token = str(login["refresh_token"])
            token_header, token_claims = jwt_parts(auth_token)
            if any(key in token_claims for key in ("iss", "aud", "jti")) or "kid" in token_header:
                raise AssertionError("TrailBase token unexpectedly gained governed identity claims")
            checks.append(
                {
                    "id": "token_claim_limits",
                    "status": "pass",
                    "observed_limit": "access token omits iss, aud, kid, and jti",
                }
            )

            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/change_password",
                {"old_password": password, "new_password": changed, "new_password_repeat": changed},
                auth_token,
            )
            assert_status(status, 200, "password_change", checks)
            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/login",
                {"email": "person@fasti.test", "password": password},
            )
            assert_status(status, 401, "old_password_rejected", checks)

            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/reset_password/request",
                {"email": "person@fasti.test"},
            )
            assert_status(status, 200, "password_reset_request", checks)
            reset_token = email_token(smtp.messages)
            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/reset_password/update",
                {"password": reset, "password_repeat": reset, "password_reset_token": reset_token},
            )
            assert_status(status, 200, "password_reset_update", checks)
            status, body = request(
                base,
                "POST",
                "/api/auth/v1/login",
                {"email": "person@fasti.test", "password": reset},
            )
            assert_status(status, 200, "reset_password_login", checks)
            login = json_object(body)
            auth_token = str(login["auth_token"])
            refresh_token = str(login["refresh_token"])

            status, body = request(base, "GET", "/api/auth/v1/totp/register", token=auth_token)
            assert_status(status, 200, "totp_enrollment_begin", checks)
            totp_url = str(json_object(body)["totp_url"])
            code = totp_code(totp_url)
            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/totp/confirm",
                {"totp_url": totp_url, "totp": code},
                auth_token,
            )
            assert_status(status, 200, "totp_enrollment_confirm", checks)
            status, body = request(
                base,
                "POST",
                "/api/auth/v1/login",
                {"email": "person@fasti.test", "password": reset},
            )
            assert_status(status, 403, "totp_login_challenge", checks)
            mfa_token = str(json_object(body)["mfa_token"])
            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/login_mfa",
                {"mfa_token": mfa_token, "totp": "000000"},
            )
            assert_status(status, 401, "invalid_totp_rejected", checks)
            status, body = request(
                base,
                "POST",
                "/api/auth/v1/login_mfa",
                {"mfa_token": mfa_token, "totp": totp_code(totp_url)},
            )
            assert_status(status, 200, "totp_login", checks)
            mfa_login = json_object(body)
            auth_token = str(mfa_login["auth_token"])
            refresh_token = str(mfa_login["refresh_token"])
            status, _ = request(
                base, "POST", "/api/auth/v1/totp/unregister", {"totp": totp_code(totp_url)}, auth_token
            )
            assert_status(status, 200, "totp_removal", checks)

            status, body = request(
                base, "POST", "/api/auth/v1/refresh", {"refresh_token": refresh_token}
            )
            assert_status(status, 200, "refresh_session", checks)
            json_object(body)
            status, _ = request(
                base, "POST", "/api/auth/v1/refresh", {"refresh_token": refresh_token}
            )
            assert_status(status, 200, "refresh_token_reuse_after_refresh", checks)
            checks[-1]["observed_limit"] = (
                "the same presented refresh token remains valid after a successful refresh"
            )
            status, _ = request(base, "POST", "/api/auth/v1/logout", {"refresh_token": refresh_token})
            assert_status(status, 200, "refresh_revocation", checks)
            status, _ = request(base, "POST", "/api/auth/v1/refresh", {"refresh_token": refresh_token})
            assert_status(status, 401, "revoked_refresh_rejected", checks)

            oidc.profile = {
                "sub": "social-fixture",
                "email": "social@fasti.test",
                "email_verified": True,
                "preferred_username": "social-fixture",
            }
            social_browser = browser()
            status, _ = browser_get(
                social_browser, f"{base}/api/auth/v1/oauth/oidc0/login"
            )
            assert_status(status, 200, "social_signin", checks)
            status, body = browser_get(social_browser, f"{base}/api/auth/v1/status")
            assert_status(status, 200, "social_session_status", checks)
            social_status = json_object(body)
            social_auth_token = str(social_status.get("auth_token") or "")
            if not social_auth_token:
                raise AssertionError("social sign-in did not create a TrailBase session")

            status, body = request(
                base, "GET", "/api/auth/v1/totp/register", token=social_auth_token
            )
            assert_status(status, 200, "social_totp_enrollment_begin", checks)
            social_totp_url = str(json_object(body)["totp_url"])
            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/totp/confirm",
                {"totp_url": social_totp_url, "totp": totp_code(social_totp_url)},
                social_auth_token,
            )
            assert_status(status, 200, "social_totp_enrollment_confirm", checks)

            social_browser = browser()
            status, _ = browser_get(
                social_browser, f"{base}/api/auth/v1/oauth/oidc0/login"
            )
            assert_status(status, 200, "social_callback_bypasses_totp", checks)
            checks[-1]["observed_limit"] = (
                "TrailBase social callbacks do not prove TOTP for the current authentication"
            )
            status, body = browser_get(social_browser, f"{base}/api/auth/v1/status")
            social_auth_token = str(json_object(body).get("auth_token") or "")
            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/totp/unregister",
                {"totp": totp_code(social_totp_url)},
                social_auth_token,
            )
            assert_status(status, 200, "social_totp_removal", checks)

            oidc.profile = {
                "sub": "social-collision",
                "email": "person@fasti.test",
                "email_verified": True,
                "preferred_username": "social-collision",
            }
            collision_browser = browser()
            status, _ = browser_get(
                collision_browser, f"{base}/api/auth/v1/oauth/oidc0/login"
            )
            if status < 400:
                raise AssertionError("social email collision was accepted or auto-linked")
            checks.append(
                {
                    "id": "social_email_collision_rejected",
                    "status": "pass",
                    "http_status": status,
                }
            )

            oidc.profile = {
                "sub": "social-outage",
                "email": "outage@fasti.test",
                "email_verified": True,
                "preferred_username": "social-outage",
            }
            oidc.outage = True
            outage_browser = browser()
            status, _ = browser_get(
                outage_browser, f"{base}/api/auth/v1/oauth/oidc0/login"
            )
            oidc.outage = False
            if status < 400 or not oidc.outage_observed:
                raise AssertionError("social provider outage did not fail closed")
            checks.append(
                {"id": "social_provider_outage", "status": "pass", "http_status": status}
            )

            status, _ = request(base, "DELETE", "/api/auth/v1/delete", token=auth_token)
            assert_status(status, 200, "account_delete", checks)
            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/login",
                {"email": "person@fasti.test", "password": reset},
            )
            assert_status(status, 401, "deleted_account_login_rejected", checks)

            for label, method, url in [
                ("public_admin_absent", "GET", f"{base}/api/_admin/info"),
                ("record_api_unconfigured", "GET", f"{base}/api/records/v1"),
                ("private_health_absent", "GET", "http://127.0.0.1:24501/api/healthcheck"),
                (
                    "private_admin_mfa_absent",
                    "POST",
                    "http://127.0.0.1:24501/api/auth/v1/login_mfa",
                ),
            ]:
                endpoint, path = url.rsplit("/", 1)
                status, _ = request(endpoint, method, f"/{path}", {} if method == "POST" else None)
                assert_status(status, 404, label, checks)

            os.killpg(process.pid, 15)
            process.wait(timeout=10)
            process = spawn()
            for _ in range(100):
                try:
                    status, body = request(base, "GET", "/api/healthcheck")
                except urllib.error.URLError:
                    time.sleep(0.1)
                    continue
                if status == 200 and body == b"Ok":
                    break
                time.sleep(0.1)
            else:
                raise AssertionError("TrailBase restart liveness timed out")
            status, _ = request(
                base,
                "POST",
                "/api/auth/v1/login",
                {"email": "person@fasti.test", "password": reset},
            )
            assert_status(status, 401, "restart_preserves_account_delete", checks)
            upgrade_evidence = run_upgrade_fixture(
                source_root,
                fixture,
                smtp,
                Path(executable),
                checks,
            )
        finally:
            if process.poll() is None:
                os.killpg(process.pid, 15)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, 9)
                    process.wait(timeout=5)
            smtp.shutdown()
            smtp.server_close()
            smtp_thread.join(timeout=2)
            oidc.shutdown()
            oidc.server_close()
            oidc_thread.join(timeout=2)

        runtime.verify_private_root(test_root)
        openapi = subprocess.run(  # nosec -- digest-verified binary; fixed local argv, no shell.
            [executable, "--depot", test_root / "depot", "openapi", "print"],  # nosemgrep -- exact executable and fixed local arguments.
            check=True,
            capture_output=True,
            timeout=30,
        ).stdout
        admin_list = subprocess.run(  # nosec -- digest-verified binary; fixed local argv, no shell.
            [executable, "--depot", test_root / "depot", "admin", "list"],  # nosemgrep -- exact executable and fixed local arguments.
            capture_output=True,
            timeout=30,
        )
        if admin_list.returncode != 0:
            raise AssertionError("identity administration CLI failed")
        checks.append({"id": "identity_administration", "status": "pass"})

        receipt = {
            "schema_version": "fasti.trailbase-conformance.v1",
            "release": runtime.load_release()["version"],
            "backup_sha256": backup_digest,
            "openapi_sha256": hashlib.sha256(openapi).hexdigest(),
            "checks": checks,
            "social": {
                "status": "verified",
                "provider": "oidc0",
                "fixture": "local authorization-code provider with PKCE",
                "assurance_limit": "TrailBase social callbacks do not prove TOTP for the current authentication.",
            },
            "account_disable": {
                "status": "unavailable",
                "reason": "TrailBase v0.33.5 has session invalidation and account deletion but no documented per-account disabled state.",
                "next_action": "Keep disable unavailable until a pinned TrailBase release exposes a public supported account-disable capability.",
            },
            "remote_exposure": {
                "status": "unavailable",
                "reason": "TrailBase v0.33.5 accepts protocol-relative redirect values through its shared redirect validator.",
                "next_action": "Keep TrailBase on loopback and do not expose account or OAuth routes until a pinned release rejects unsafe redirect forms and the negative control passes.",
            },
            "administrator_mfa": {
                "status": "unavailable_on_isolated_admin_listener",
                "reason": "TrailBase v0.33.5 does not mount POST /api/auth/v1/login_mfa on its separate administrator listener.",
                "next_action": "Do not enroll an administrator in TOTP on the isolated listener until a pinned release exposes and verifies the second-factor login route.",
            },
            "upgrade_rollback": upgrade_evidence,
            "architecture_evidence": {
                "executed_native": runtime.host_target(),
                "other_linux_architecture": {
                    "status": "exact_artifact_and_oci_graph_only",
                    "reason": "Cross-architecture emulation is not accepted as native runtime evidence.",
                    "next_action": "Run the same milestone gate on the other native Linux architecture before claiming two-architecture execution.",
                },
            },
            "duration_ms": round((time.monotonic() - started) * 1000),
        }
        receipt_path.parent.mkdir(parents=True, exist_ok=True)
        temporary = receipt_path.with_suffix(receipt_path.suffix + ".tmp")
        temporary.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        temporary.replace(receipt_path)
        print(f"PASS: {len(checks)} TrailBase account and recovery checks; receipt={receipt_path}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        run_fixture(arguments.root, arguments.receipt)
        return 0
    except (AssertionError, OSError, runtime.ReleaseError, subprocess.SubprocessError) as error:
        print(f"FAIL: {error}", file=os.sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
