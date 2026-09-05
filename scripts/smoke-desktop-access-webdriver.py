#!/usr/bin/env python3
"""Reproduce the deferred packaged-WebView C1 authentication boundary."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import platform
import queue
import re
import secrets
import shutil
import smtplib
import socket
import sqlite3
import subprocess  # nosec B404 -- this gate launches exact local test artifacts.
import sys
import tempfile
import threading
import time
import types
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable

import trailbase_runtime as runtime


ROOT = Path(__file__).resolve().parents[1]
SOURCE_TRAILBASE_ROOT = ROOT / ".dev-trailbase"
DESKTOP_MANIFEST = ROOT / "apps/desktop/src-tauri/Cargo.toml"
RELEASE_DESKTOP = ROOT / "apps/desktop/src-tauri/target/release/fasti-desktop"
SMOKE_TRAILBASE = ROOT / "scripts/smoke-trailbase.py"
PRIVATE_DRIVER_ROOT = Path.home() / ".cache/tools/fasti-tauri-webdriver"
PRIVATE_TAURI_DRIVER = PRIVATE_DRIVER_ROOT / "cargo/bin/tauri-driver"
PRIVATE_WEBKIT_DRIVER = PRIVATE_DRIVER_ROOT / "webkit/usr/bin/WebKitWebDriver"
FASTI_ORIGIN = "http://127.0.0.1:8420"
TRAILBASE_ORIGIN = "http://127.0.0.1:4000"
WEBDRIVER_ORIGIN = "http://127.0.0.1:4444"
SESSION_COOKIE = "__Host-fasti_session"
CSRF_COOKIE = "__Host-fasti_csrf"
W3C_ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf"
CREDENTIAL_NAME = re.compile(
    r"(?:trailbase|(?:access|auth|id|refresh)[_:.-]?token|oauth[_:.-]?state)",
    re.I,
)
COMPACT_JWT = re.compile(r"[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+")


class GateError(RuntimeError):
    """A redacted acceptance-gate failure."""


class WebDriverError(GateError):
    """A redacted W3C WebDriver failure."""


def _load_trailbase_smoke() -> types.ModuleType:
    spec = importlib.util.spec_from_file_location("fasti_smoke_trailbase", SMOKE_TRAILBASE)
    if spec is None or spec.loader is None:
        raise GateError("the TrailBase smoke helpers could not be loaded")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _run(
    command: list[str | Path],
    *,
    environment: dict[str, str] | None = None,
    timeout: float = 300,
) -> subprocess.CompletedProcess[bytes]:
    label = Path(str(command[0])).name
    try:
        return subprocess.run(  # nosec B603 -- fixed argv and explicit local paths; no shell.
            [str(value) for value in command],
            check=True,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
        )
    except subprocess.CalledProcessError as error:
        raise GateError(
            f"required local command failed: {label} exited {error.returncode}"
        ) from error
    except subprocess.TimeoutExpired as error:
        raise GateError(f"required local command timed out: {label}") from error
    except OSError as error:
        raise GateError(f"required local command could not start: {label}") from error


def _git_text(*arguments: str) -> str:
    return _run(["git", "-C", ROOT, *arguments], timeout=30).stdout.decode().strip()


def _require_clean_source() -> str:
    status = _git_text("status", "--porcelain=v1", "--untracked-files=all")
    allowed = f"?? {Path(__file__).resolve().relative_to(ROOT)}"
    unexpected = [line for line in status.splitlines() if line != allowed]
    if unexpected:
        raise GateError("the C1 packaged-WebView gate requires a clean Git worktree")
    return status


def _enter_route_less_namespace() -> None:
    if os.environ.get("FASTI_C1_WEBDRIVER_NETNS") == "1":
        _run(["ip", "link", "set", "lo", "up"], timeout=10)
        if _run(["ip", "route", "show"], timeout=10).stdout.strip():
            raise GateError("the packaged-WebView network namespace has an IPv4 route")
        if _run(["ip", "-6", "route", "show"], timeout=10).stdout.strip():
            raise GateError("the packaged-WebView network namespace has an IPv6 route")
        return
    unshare = _command_path(None, "unshare")
    _command_path(None, "ip")
    _run([unshare, "--user", "--map-root-user", "--net", "--", "true"], timeout=10)
    environment = dict(os.environ)
    environment["FASTI_C1_WEBDRIVER_NETNS"] = "1"
    os.execve(
        unshare,
        [
            str(unshare),
            "--user",
            "--map-root-user",
            "--net",
            "--",
            sys.executable,
            "-B",
            str(Path(__file__).resolve()),
            *sys.argv[1:],
        ],
        environment,
    )


def _route_less_evidence() -> dict[str, object]:
    if os.environ.get("FASTI_C1_WEBDRIVER_NETNS") != "1":
        raise GateError("the packaged-WebView gate is outside its network namespace")
    if _run(["ip", "route", "show"], timeout=10).stdout.strip():
        raise GateError("the packaged-WebView network namespace gained an IPv4 route")
    if _run(["ip", "-6", "route", "show"], timeout=10).stdout.strip():
        raise GateError("the packaged-WebView network namespace gained an IPv6 route")
    if Path("/sys/class/net/lo/operstate").read_text(encoding="ascii").strip() not in {
        "unknown",
        "up",
    }:
        raise GateError("the packaged-WebView loopback interface is unavailable")
    return {
        "user_namespace": True,
        "network_namespace": True,
        "loopback_only": True,
        "ipv4_routes": 0,
        "ipv6_routes": 0,
    }


def _command_path(argument: Path | None, name: str) -> Path:
    raw = str(argument) if argument is not None else shutil.which(name)
    if not raw:
        raise GateError(f"missing packaged-WebView prerequisite: {name}")
    path = Path(raw).resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise GateError(f"packaged-WebView prerequisite is not executable: {name}")
    return path


def _require_free_port(port: int, label: str) -> None:
    candidate = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        candidate.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 0)
        candidate.bind(("127.0.0.1", port))
    except OSError as error:
        raise GateError(f"{label} requires unused 127.0.0.1:{port}") from error
    finally:
        candidate.close()


def _private_directory(path: Path) -> None:
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chmod(path, 0o700)  # nosec B103 -- owner-only fixture boundary.


def _copy_exact_release_input(source_root: Path, target_root: Path) -> Path:
    runtime.verify_installation(source_root)
    release = runtime.load_release()
    target = runtime.host_target()
    archive_name = f"trailbase-v{release['version']}-{target}.zip"
    source = source_root / "cache" / archive_name
    runtime.verify_archive(source, release, target)
    _private_directory(target_root)
    cache = target_root / "cache"
    _private_directory(cache)
    copied = cache / archive_name
    shutil.copyfile(source, copied)
    os.chmod(copied, 0o600)
    runtime.verify_archive(copied, release, target)
    if runtime.sha256_file(copied) != runtime.sha256_file(source):
        raise GateError("the disposable TrailBase release input differs from the exact cache")

    auth_ui = release["auth_ui"]
    auth_archive_name = f"trailbase-v{release['version']}-wasm-auth-ui.zip"
    auth_source = source_root / "cache" / auth_archive_name
    if (
        not auth_source.is_file()
        or auth_source.is_symlink()
        or auth_source.stat().st_size != auth_ui["bytes"]
        or runtime.sha256_file(auth_source) != auth_ui["sha256"]
    ):
        raise GateError("the cached TrailBase Auth UI differs from the exact release lock")
    auth_copied = cache / auth_archive_name
    shutil.copyfile(auth_source, auth_copied)
    os.chmod(auth_copied, 0o600)
    if (
        auth_copied.stat().st_size != auth_ui["bytes"]
        or runtime.sha256_file(auth_copied) != auth_ui["sha256"]
        or runtime.sha256_file(auth_copied) != runtime.sha256_file(auth_source)
    ):
        raise GateError("the disposable TrailBase Auth UI differs from the exact cache")
    return runtime.prepare_native(target_root, offline=True)


def _json_object(body: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(body)
    except json.JSONDecodeError as error:
        raise GateError(f"{label} returned invalid JSON") from error
    if not isinstance(value, dict):
        raise GateError(f"{label} returned a non-object response")
    return value


def _new_password() -> str:
    return "Aa1!" + secrets.token_urlsafe(24)


def _bootstrap_disposable_installation(
    smoke: types.ModuleType,
    executable: Path,
    root: Path,
) -> None:
    initial_password: list[str] = []
    replacement_password = _new_password()
    auth_token = ""
    refresh_token = ""
    process: subprocess.Popen[bytes] | None = None
    reader: threading.Thread | None = None
    try:
        process, reader = smoke.start_fixture_release(
            executable,
            root,
            4000,
            initial_password=initial_password,
        )
        if len(initial_password) != 1:
            raise GateError("TrailBase did not produce exactly one disposable administrator secret")
        status, body = smoke.request(
            TRAILBASE_ORIGIN,
            "POST",
            "/api/auth/v1/login",
            {"email": "admin@localhost", "password": initial_password[0]},
        )
        if status != 200:
            raise GateError("the disposable TrailBase administrator could not authenticate")
        login = _json_object(body, "TrailBase administrator login")
        auth_token = login.get("auth_token") if isinstance(login.get("auth_token"), str) else ""
        if not auth_token:
            raise GateError("the disposable TrailBase administrator login omitted its proof")
        status, _ = smoke.request(
            TRAILBASE_ORIGIN,
            "POST",
            "/api/auth/v1/change_password",
            {
                "old_password": initial_password[0],
                "new_password": replacement_password,
                "new_password_repeat": replacement_password,
            },
            auth_token,
        )
        if status != 200:
            raise GateError("the disposable TrailBase administrator rotation failed")
        status, body = smoke.request(
            TRAILBASE_ORIGIN,
            "POST",
            "/api/auth/v1/login",
            {"email": "admin@localhost", "password": replacement_password},
        )
        if status != 200:
            raise GateError("the rotated disposable TrailBase administrator was not verified")
        rotated = _json_object(body, "rotated TrailBase administrator login")
        refresh_token = (
            rotated.get("refresh_token") if isinstance(rotated.get("refresh_token"), str) else ""
        )
        if refresh_token:
            smoke.request(
                TRAILBASE_ORIGIN,
                "POST",
                "/api/auth/v1/logout",
                {"refresh_token": refresh_token},
            )
    finally:
        auth_token = ""
        refresh_token = ""
        replacement_password = ""
        initial_password.clear()
        if process is not None:
            smoke.stop_process(process, reader)

    smoke.write_bootstrap_receipt(root, str(runtime.load_release()["version"]))


def _write_fixture_config(root: Path, smtp_port: int) -> None:
    config = f'''email {{
  smtp_host: "127.0.0.1"
  smtp_port: {smtp_port}
  smtp_encryption: SMTP_ENCRYPTION_NONE
  sender_name: "Fasti C1 Acceptance"
  sender_address: "noreply@fasti.test"
}}
server {{
  application_name: "Fasti C1 Acceptance"
  site_url: "{TRAILBASE_ORIGIN}"
  logs_retention_sec: 3600
}}
auth {{
  auth_token_ttl_sec: 300
  refresh_token_ttl_sec: 3600
  password_minimal_length: 12
  password_must_contain_upper_and_lower_case: true
  password_must_contain_digits: true
  password_must_contain_special_characters: true
}}
jobs {{}}
'''
    path = root / "depot/config.textproto"
    path.write_text(config, encoding="utf-8")
    os.chmod(path, 0o600)


def _register_verified_human(
    smoke: types.ModuleType,
    messages: queue.Queue[bytes],
) -> tuple[str, str]:
    account_email = f"c1-webdriver-{secrets.token_hex(12)}@fasti.test"
    account_password = _new_password()
    verification = ""
    auth_token = ""
    refresh_token = ""
    try:
        status, _ = smoke.request(
            TRAILBASE_ORIGIN,
            "POST",
            "/api/auth/v1/register",
            {
                "email": account_email,
                "password": account_password,
                "password_repeat": account_password,
            },
        )
        if status != 200:
            raise GateError("the generated human TrailBase account could not be registered")
        verification = smoke.email_token(messages)
        status, _ = smoke.request(
            TRAILBASE_ORIGIN,
            "GET",
            f"/api/auth/v1/verify_email/confirm/{verification}",
        )
        if status != 200:
            raise GateError("the generated human TrailBase account could not be verified")
        status, body = smoke.request(
            TRAILBASE_ORIGIN,
            "POST",
            "/api/auth/v1/login",
            {"email": account_email, "password": account_password},
        )
        if status != 200:
            raise GateError("the verified human TrailBase account could not authenticate")
        login = _json_object(body, "TrailBase human login")
        auth_token = login.get("auth_token") if isinstance(login.get("auth_token"), str) else ""
        refresh_token = (
            login.get("refresh_token") if isinstance(login.get("refresh_token"), str) else ""
        )
        if not auth_token or not refresh_token:
            raise GateError("the verified human TrailBase login omitted required proof")
        status, _ = smoke.request(
            TRAILBASE_ORIGIN,
            "POST",
            "/api/auth/v1/logout",
            {"refresh_token": refresh_token},
        )
        if status != 200:
            raise GateError("the human-account preflight session could not be cleared")
        return account_email, account_password
    finally:
        verification = ""
        auth_token = ""
        refresh_token = ""


def _build_and_copy_desktop(source: Path, installation: Path) -> tuple[str, int]:
    _run(["pnpm", "--dir", ROOT, "--filter", "@fasti/web", "build"], timeout=600)
    build_environment = dict(os.environ)
    build_environment["PKG_CONFIG"] = "/usr/bin/pkg-config"
    _run(
        [
            "cargo",
            "build",
            "--manifest-path",
            DESKTOP_MANIFEST,
            "--release",
            "--locked",
            "--offline",
        ],
        environment=build_environment,
        timeout=1800,
    )
    if not source.is_file() or not os.access(source, os.X_OK):
        raise GateError("the locked release desktop binary is missing")
    installation.parent.mkdir(mode=0o700)
    shutil.copyfile(source, installation)
    os.chmod(installation, 0o700)
    source_digest = runtime.sha256_file(source)
    copied_digest = runtime.sha256_file(installation)
    if copied_digest != source_digest:
        raise GateError("the copied desktop bytes differ from the release artifact")
    return copied_digest, installation.stat().st_size


def _read_line_with_timeout(
    stream: Any,
    timeout: float,
    label: str,
) -> str:
    lines: queue.Queue[str] = queue.Queue(maxsize=1)
    reader = threading.Thread(target=lambda: lines.put(stream.readline()), daemon=True)
    reader.start()
    try:
        value = lines.get(timeout=timeout).strip()
    except queue.Empty as error:
        raise GateError(f"{label} did not become ready") from error
    if not value:
        raise GateError(f"{label} did not publish its local address")
    return value


def _start_private_secret_service(
    workspace: Path,
    dbus_daemon: Path,
    keyring_daemon: Path,
    base_environment: dict[str, str],
) -> tuple[subprocess.Popen[str], subprocess.Popen[bytes], dict[str, str]]:
    environment = dict(base_environment)
    bus = subprocess.Popen(  # nosec B603 -- resolved system executable and fixed argv.
        [
            str(dbus_daemon),
            "--session",
            "--nofork",
            "--print-address=1",
            "--nopidfile",
            "--nosyslog",
        ],
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        start_new_session=True,
    )
    keyring: subprocess.Popen[bytes] | None = None
    try:
        if bus.stdout is None:
            raise GateError("the private D-Bus session has no address channel")
        environment["DBUS_SESSION_BUS_ADDRESS"] = _read_line_with_timeout(
            bus.stdout, 5, "the private D-Bus session"
        )
        control = workspace / "keyring-control"
        _private_directory(control)
        keyring = subprocess.Popen(  # nosec B603 -- resolved system executable and fixed argv.
            [
                str(keyring_daemon),
                "--foreground",
                "--unlock",
                "--components=secrets",
                "--control-directory",
                str(control),
            ],
            env=environment,
            stdin=subprocess.PIPE,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        password = bytearray(secrets.token_urlsafe(32).encode("ascii"))
        if keyring.stdin is None:
            raise GateError("the private credential store has no unlock channel")
        try:
            keyring.stdin.write(password + b"\n")
            keyring.stdin.close()
        finally:
            for index in range(len(password)):
                password[index] = 0
        time.sleep(0.5)
        if bus.poll() is not None or keyring.poll() is not None:
            raise GateError("the private credential store exited during startup")
        return bus, keyring, environment
    except BaseException:
        _stop_process(keyring)
        _stop_process(bus)
        raise


def _stop_process(process: subprocess.Popen[Any] | None) -> None:
    if process is not None:
        runtime.stop_managed_process_group(process)


class W3CClient:
    def __init__(self, origin: str = WEBDRIVER_ORIGIN, timeout: float = 10) -> None:
        self.origin = origin
        self.timeout = timeout
        self.session_id: str | None = None
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def _request(
        self,
        method: str,
        path: str,
        payload: dict[str, object] | None = None,
    ) -> Any:
        body = None if payload is None else json.dumps(payload).encode("utf-8")
        headers = {"Content-Type": "application/json"} if body is not None else {}
        request = urllib.request.Request(self.origin + path, body, headers, method=method)
        try:
            with self.opener.open(request, timeout=self.timeout) as response:
                response_body = response.read(1024 * 1024)
        except urllib.error.HTTPError as error:
            error_body = error.read(1024 * 1024)
            error_code = "unknown error"
            try:
                error_envelope = json.loads(error_body)
                error_value = error_envelope.get("value", {})
                if isinstance(error_value, dict) and isinstance(
                    error_value.get("error"), str
                ):
                    error_code = error_value["error"]
            except (AttributeError, json.JSONDecodeError):
                pass
            operation = re.sub(r"/session/[^/]+", "/session/{id}", path)
            if operation.endswith("/element") and isinstance(payload, dict):
                locator = payload.get("value")
                if isinstance(locator, str) and len(locator) <= 512:
                    operation = f"{operation} locator={locator}"
            raise WebDriverError(
                f"the packaged WebDriver rejected {method} {operation}: {error_code}"
            ) from error
        except (OSError, urllib.error.URLError) as error:
            raise WebDriverError("the packaged WebDriver transport failed") from error
        try:
            envelope = json.loads(response_body or b"{}")
        except json.JSONDecodeError as error:
            raise WebDriverError("the packaged WebDriver returned invalid JSON") from error
        if not isinstance(envelope, dict) or "value" not in envelope:
            raise WebDriverError("the packaged WebDriver returned an invalid envelope")
        value = envelope["value"]
        if isinstance(value, dict) and isinstance(value.get("error"), str):
            operation = re.sub(r"/session/[^/]+", "/session/{id}", path)
            if operation.endswith("/element") and isinstance(payload, dict):
                locator = payload.get("value")
                if isinstance(locator, str) and len(locator) <= 512:
                    operation = f"{operation} locator={locator}"
            raise WebDriverError(
                f"the packaged WebDriver failed {method} {operation}: {value['error']}"
            )
        return value

    def wait_ready(self, timeout: float) -> None:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                value = self._request("GET", "/status")
                if isinstance(value, dict) and value.get("ready") is not False:
                    return
            except WebDriverError:
                pass
            time.sleep(0.1)
        raise WebDriverError("tauri-driver did not become ready")

    def create_session(self, application: Path) -> None:
        value = self._request(
            "POST",
            "/session",
            {
                "capabilities": {
                    "alwaysMatch": {
                        "browserName": "wry",
                        "tauri:options": {
                            "application": str(application),
                            "args": [],
                        },
                    }
                }
            },
        )
        if not isinstance(value, dict):
            raise WebDriverError("the packaged WebDriver session response is invalid")
        session_id = value.get("sessionId")
        if not isinstance(session_id, str) or not session_id:
            raise WebDriverError("the packaged WebDriver session has no identifier")
        self.session_id = session_id

    def close(self) -> None:
        if self.session_id is None:
            return
        session_id, self.session_id = self.session_id, None
        try:
            self._request("DELETE", f"/session/{session_id}")
        except WebDriverError:
            pass

    def _session_path(self, suffix: str) -> str:
        if self.session_id is None:
            raise WebDriverError("the packaged WebDriver session is not open")
        return f"/session/{self.session_id}{suffix}"

    def find(self, using: str, value: str) -> str:
        found = self._request(
            "POST",
            self._session_path("/element"),
            {"using": using, "value": value},
        )
        if not isinstance(found, dict):
            raise WebDriverError("the packaged WebDriver returned an invalid element")
        element = found.get(W3C_ELEMENT_KEY, found.get("ELEMENT"))
        if not isinstance(element, str) or not element:
            raise WebDriverError("the packaged WebDriver element has no identifier")
        return element

    def dom_click(self, element: str) -> None:
        clicked = self._request(
            "POST",
            self._session_path("/execute/sync"),
            {
                "script": "arguments[0].click(); return true;",
                "args": [{W3C_ELEMENT_KEY: element}],
            },
        )
        if clicked is not True:
            raise WebDriverError("the packaged WebDriver could not activate the element")

    def send_keys(self, element: str, value: str) -> None:
        self._request(
            "POST",
            self._session_path(f"/element/{element}/value"),
            {"text": value, "value": list(value)},
        )

    def text(self, element: str) -> str:
        value = self._request("GET", self._session_path(f"/element/{element}/text"))
        if not isinstance(value, str):
            raise WebDriverError("the packaged WebDriver returned invalid element text")
        return value

    def attribute(self, element: str, name: str) -> str | None:
        value = self._request(
            "GET",
            self._session_path(f"/element/{element}/attribute/{name}"),
        )
        if value is not None and not isinstance(value, str):
            raise WebDriverError("the packaged WebDriver returned an invalid attribute")
        return value

    def execute(self, script: str) -> Any:
        return self._request(
            "POST",
            self._session_path("/execute/sync"),
            {"script": script, "args": []},
        )

    def current_url(self) -> str:
        value = self._request("GET", self._session_path("/url"))
        if not isinstance(value, str):
            raise WebDriverError("the packaged WebDriver returned an invalid URL")
        return value

    def navigate(self, url: str) -> None:
        self._request("POST", self._session_path("/url"), {"url": url})

    def cookies(self) -> list[dict[str, Any]]:
        value = self._request("GET", self._session_path("/cookie"))
        if not isinstance(value, list) or not all(isinstance(cookie, dict) for cookie in value):
            raise WebDriverError("the packaged WebDriver returned invalid cookies")
        return value


def _wait_for(
    operation: Callable[[], Any],
    predicate: Callable[[Any], bool],
    timeout: float,
    label: str,
) -> Any:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            value = operation()
            if predicate(value):
                return value
        except WebDriverError:
            pass
        time.sleep(0.1)
    raise GateError(f"timed out while waiting for {label}")


def _button(driver: W3CClient, label: str) -> str:
    escaped = label.replace("'", "&apos;")
    return driver.find("xpath", f"//button[normalize-space(.)='{escaped}']")


def _contains_credential_value(value: str, generated_password: str) -> bool:
    return bool(generated_password and generated_password in value) or bool(
        COMPACT_JWT.search(value)
    )


def _validate_fasti_cookies(
    cookies: list[dict[str, Any]], generated_password: str
) -> dict[str, object]:
    if len(cookies) > 128:
        raise GateError("the packaged WebView returned too many cookies")
    by_name = {
        cookie.get("name"): cookie
        for cookie in cookies
        if isinstance(cookie.get("name"), str)
    }
    session = by_name.get(SESSION_COOKIE)
    csrf = by_name.get(CSRF_COOKIE)
    if any(
        CREDENTIAL_NAME.search(name)
        or (
            isinstance(cookie.get("value"), str)
            and _contains_credential_value(cookie["value"], generated_password)
        )
        for name, cookie in by_name.items()
    ):
        raise GateError("the packaged WebView retained a TrailBase credential cookie")
    if session is None or csrf is None:
        raise GateError("the callback did not create both opaque Fasti cookies")
    session_value = session.get("value")
    csrf_value = csrf.get("value")
    if not isinstance(session_value, str) or re.fullmatch(r"[0-9a-f]{64}", session_value) is None:
        raise GateError("the Fasti browser session is not an opaque 256-bit value")
    if not isinstance(csrf_value, str) or re.fullmatch(r"[0-9a-f]{64}", csrf_value) is None:
        raise GateError("the Fasti CSRF proof is not an opaque 256-bit value")
    if session_value == csrf_value or session_value.count(".") == 2:
        raise GateError("the Fasti browser session is not independently opaque")
    for cookie, label in ((session, "session"), (csrf, "CSRF")):
        if cookie.get("path") != "/" or str(cookie.get("domain", "")).lstrip(".") != "127.0.0.1":
            raise GateError(f"the Fasti {label} cookie has the wrong scope")
        if cookie.get("secure") is not True or str(cookie.get("sameSite", "")).lower() != "strict":
            raise GateError(f"the Fasti {label} cookie lacks its required browser policy")
        expiry = cookie.get("expiry")
        if not isinstance(expiry, (int, float)) or expiry <= time.time():
            raise GateError(f"the Fasti {label} cookie lacks a future expiry")
    if session.get("httpOnly") is not True or csrf.get("httpOnly") is True:
        raise GateError("the Fasti session and CSRF HttpOnly policies differ from contract")
    evidence = {
        "session": {
            "name": SESSION_COOKIE,
            "opaque_256_bit": True,
            "path": "/",
            "secure": True,
            "http_only": True,
            "same_site": "Strict",
        },
        "csrf": {
            "name": CSRF_COOKIE,
            "opaque_256_bit": True,
            "path": "/",
            "secure": True,
            "http_only": False,
            "same_site": "Strict",
        },
        "vendor_credential_cookies_absent": True,
    }
    for cookie in cookies:
        if "value" in cookie:
            cookie["value"] = ""
    session_value = ""
    csrf_value = ""
    return evidence


def _validate_browser_storage(
    driver: W3CClient, generated_password: str
) -> dict[str, object]:
    value = driver.execute(
        "return {local: Object.entries(window.localStorage), "
        "session: Object.entries(window.sessionStorage)};"
    )
    if not isinstance(value, dict):
        raise GateError("the packaged WebView returned invalid storage metadata")
    local = value.get("local")
    session = value.get("session")
    if (
        not isinstance(local, list)
        or not isinstance(session, list)
        or len(local) > 128
        or len(session) > 128
        or not all(
            isinstance(entry, list)
            and len(entry) == 2
            and all(isinstance(item, str) and len(item) <= 8192 for item in entry)
            for entry in [*local, *session]
        )
    ):
        raise GateError("the packaged WebView returned invalid storage entries")
    if any(
        CREDENTIAL_NAME.search(key)
        or _contains_credential_value(stored_value, generated_password)
        for key, stored_value in [*local, *session]
    ):
        raise GateError("the packaged WebView retained vendor credential storage")
    return {
        "vendor_credential_keys_absent": True,
        "local_storage_key_count": len(local),
        "session_storage_key_count": len(session),
    }


def _probe_binding_cookie(driver: W3CClient, login_url: str) -> None:
    probe_url = f"{FASTI_ORIGIN}/api/access/v1/trailbase/callback/probe"
    driver.navigate(probe_url)
    _wait_for(
        driver.current_url,
        lambda value: value == probe_url,
        10,
        "the callback-path cookie probe",
    )
    cookies = driver.cookies()
    binding = next(
        (cookie for cookie in cookies if cookie.get("name") == "__Secure-fasti_auth_binding"),
        None,
    )
    if binding is None:
        for cookie in cookies:
            cookie["value"] = ""
        driver.navigate(f"{FASTI_ORIGIN}/first-run")
        _wait_for(
            lambda: driver.find("xpath", "//*[@id='first-run-title']"),
            lambda value: isinstance(value, str),
            10,
            "the first-run cookie diagnostic",
        )
        driver.dom_click(
            driver.find(
                "xpath",
                "//*[@data-testid='first-run-guided-setup']"
                "//button[normalize-space(.)='Sign in to an existing account']",
            )
        )
        _wait_for(
            driver.current_url,
            lambda current: urllib.parse.urlsplit(current).netloc == "127.0.0.1:4000",
            10,
            "the ordinary TrailBase sign-in cookie diagnostic",
        )
        driver.navigate(probe_url)
        _wait_for(
            driver.current_url,
            lambda value: value == probe_url,
            10,
            "the HTTP-set callback cookie probe",
        )
        response_cookies = driver.cookies()
        response_binding_available = any(
            cookie.get("name") == "__Secure-fasti_auth_binding"
            for cookie in response_cookies
        )
        for cookie in response_cookies:
            cookie["value"] = ""
        raise GateError(
            "the packaged WebView did not store the native callback binding cookie; "
            f"http_response_binding_cookie_stored={response_binding_available}"
        )
    value = binding.get("value")
    try:
        if (
            not isinstance(value, str)
            or re.fullmatch(r"[0-9a-f]{64}", value) is None
            or binding.get("path") != "/api/access/v1/trailbase/callback"
            or str(binding.get("domain", "")).lstrip(".") != "127.0.0.1"
            or binding.get("secure") is not True
            or binding.get("httpOnly") is not True
            or binding.get("sameSite") != "Lax"
        ):
            raise GateError("the packaged WebView stored an invalid callback binding cookie")
    finally:
        for cookie in cookies:
            cookie["value"] = ""
    driver.navigate(login_url)
    _wait_for(
        driver.current_url,
        lambda current: current == login_url,
        10,
        "the TrailBase sign-in page after the cookie probe",
    )


def _process_rows() -> dict[int, tuple[int, Path | None]]:
    rows: dict[int, tuple[int, Path | None]] = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            status = (entry / "status").read_text(encoding="utf-8")
            parent_line = next(line for line in status.splitlines() if line.startswith("PPid:"))
            parent = int(parent_line.split()[1])
            executable = Path(os.readlink(entry / "exe"))
            rows[int(entry.name)] = (parent, executable)
        except (OSError, StopIteration, ValueError):
            continue
    return rows


def _descendants(root_pid: int, rows: dict[int, tuple[int, Path | None]]) -> set[int]:
    found = {root_pid}
    changed = True
    while changed:
        changed = False
        for process_id, (parent, _) in rows.items():
            if parent in found and process_id not in found:
                found.add(process_id)
                changed = True
    return found


def _debian_binary_evidence(path: Path) -> dict[str, object]:
    search = _run(["dpkg-query", "--search", path], timeout=30).stdout.decode("utf-8")
    matches = [line for line in search.splitlines() if line.endswith(f": {path}")]
    if len(matches) != 1:
        raise GateError("a running WebKit process has no exact package owner")
    package = matches[0].split(": ", 1)[0]
    fields = _run(
        [
            "dpkg-query",
            "--show",
            "--showformat=${binary:Package}\\n${Version}\\n${Architecture}\\n",
            package,
        ],
        timeout=30,
    ).stdout.decode("utf-8").splitlines()
    if len(fields) != 3 or any(not value for value in fields):
        raise GateError("a running WebKit process has incomplete package metadata")
    return {
        "package": fields[0],
        "version": fields[1],
        "architecture": fields[2],
        "sha256": f"sha256:{runtime.sha256_file(path)}",
        "size_bytes": path.stat().st_size,
    }


def _packaged_process_evidence(
    driver_process: subprocess.Popen[bytes],
    desktop: Path,
    desktop_digest: str,
    tauri_driver: Path,
    webkit_driver: Path,
    timeout: float,
) -> dict[str, object]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        rows = _process_rows()
        process_ids = _descendants(driver_process.pid, rows)
        executables = [rows[pid][1] for pid in process_ids if pid in rows and rows[pid][1]]
        tauri_matches = [
            path
            for pid, (_, path) in rows.items()
            if pid == driver_process.pid and path == tauri_driver
        ]
        desktop_matches = [path for path in executables if path == desktop]
        driver_matches = [path for path in executables if path == webkit_driver]
        network_processes = sorted(
            {path for path in executables if path.name == "WebKitNetworkProcess"}
        )
        web_processes = sorted({path for path in executables if path.name == "WebKitWebProcess"})
        if (
            tauri_matches
            and desktop_matches
            and driver_matches
            and network_processes
            and web_processes
        ):
            if runtime.sha256_file(desktop_matches[0]) != desktop_digest:
                raise GateError("the running desktop differs from the copied release artifact")
            if runtime.sha256_file(driver_matches[0]) != runtime.sha256_file(webkit_driver):
                raise GateError("the running WebKitWebDriver differs from the selected executable")
            return {
                "exact_tauri_driver": True,
                "copied_release_desktop": True,
                "exact_webkit_webdriver": True,
                "webkit_network_process": [
                    _debian_binary_evidence(path)
                    for path in network_processes
                ],
                "webkit_web_process": [
                    _debian_binary_evidence(path)
                    for path in web_processes
                ],
            }
        time.sleep(0.1)
    raise GateError("the packaged WebKit process tree did not become complete")


def _driver_tool_evidence(
    tauri_driver: Path,
    webkit_driver: Path,
) -> dict[str, dict[str, object]]:
    crates_file = tauri_driver.parent.parent / ".crates.toml"
    try:
        crates_text = crates_file.read_text(encoding="utf-8")
    except OSError as error:
        raise GateError("the selected tauri-driver has no exact install metadata") from error
    match = re.search(
        r'^"tauri-driver ([^ ]+) \([^\n]+\)" = \["tauri-driver"\]$',
        crates_text,
        re.M,
    )
    if match is None:
        raise GateError("the selected tauri-driver version is not exact")

    tool_root = webkit_driver.parents[2]
    packages = sorted((tool_root.parent / "packages").glob("webkitgtk-webdriver_*_*.deb"))
    if len(packages) != 1:
        raise GateError("the selected WebKitWebDriver has no unique package metadata")
    package_fields = _run(
        ["dpkg-deb", "--field", packages[0], "Package", "Version", "Architecture"],
        timeout=30,
    ).stdout.decode("utf-8")
    field_values = dict(
        line.split(":", 1) for line in package_fields.splitlines() if ":" in line
    )
    if field_values.get("Package", "").strip() != "webkitgtk-webdriver":
        raise GateError("the selected WebKitWebDriver package identity is not exact")
    webkit_version = field_values.get("Version", "").strip()
    webkit_architecture = field_values.get("Architecture", "").strip()
    if not webkit_version or not webkit_architecture:
        raise GateError("the selected WebKitWebDriver package metadata is incomplete")
    return {
        "tauri_driver": {
            "version": match.group(1),
            "sha256": f"sha256:{runtime.sha256_file(tauri_driver)}",
            "size_bytes": tauri_driver.stat().st_size,
        },
        "webkit_webdriver": {
            "version": webkit_version,
            "architecture": webkit_architecture,
            "sha256": f"sha256:{runtime.sha256_file(webkit_driver)}",
            "size_bytes": webkit_driver.stat().st_size,
            "package_sha256": f"sha256:{runtime.sha256_file(packages[0])}",
        },
    }


def _drive_authenticated_flow(
    driver: W3CClient,
    desktop: Path,
    data_root: Path,
    email_address: str,
    password: str,
    timeout: float,
) -> dict[str, object]:
    driver.create_session(desktop)
    _wait_for(
        lambda: driver.find("xpath", "//button[normalize-space(.)='Create local record']"),
        lambda value: isinstance(value, str),
        timeout,
        "the packaged Setup surface",
    )
    driver.dom_click(
        driver.find("xpath", "//button[normalize-space(.)='Create local record']")
    )
    _wait_for(
        lambda: driver.find("xpath", "//*[@id='first-run-title']"),
        lambda value: isinstance(value, str),
        timeout,
        "the first-run account surface",
    )
    try:
        first_administrator = _wait_for(
            lambda: driver.find(
                "xpath",
                "//*[@data-testid='first-run-guided-setup']"
                "//button[normalize-space(.)='Confirm first Fasti administrator']",
            ),
            lambda value: isinstance(value, str),
            timeout,
            "the first-administrator confirmation control",
        )
    except GateError as error:
        body = re.sub(
            r"\s+",
            " ",
            driver.text(driver.find("tag name", "body")),
        ).strip()[:1024]
        raise GateError(
            f"the first-administrator confirmation control is unavailable; body={body}"
        ) from error
    driver.dom_click(first_administrator)

    login_url = _wait_for(
        driver.current_url,
        lambda value: isinstance(value, str)
        and urllib.parse.urlsplit(value).scheme == "http"
        and urllib.parse.urlsplit(value).netloc == "127.0.0.1:4000"
        and urllib.parse.urlsplit(value).path == "/_/auth/login",
        timeout,
        "the TrailBase sign-in page",
    )
    parsed_login = urllib.parse.urlsplit(login_url)
    ceremony_state = _redacted_access_state(data_root)
    if ceremony_state.get("ceremonies") != [
        ["first_administrator_bootstrap", "first_run", "pending", None]
    ]:
        raise GateError(
            "the first-administrator control started the wrong ceremony; "
            f"access_state={json.dumps(ceremony_state, sort_keys=True)}"
        )
    _probe_binding_cookie(driver, login_url)
    login_query = urllib.parse.parse_qs(parsed_login.query, keep_blank_values=True)
    callback_url = f"{FASTI_ORIGIN}/api/access/v1/trailbase/callback"
    if (
        parsed_login.fragment
        or set(login_query) != {"redirect_uri", "response_type", "pkce_code_challenge"}
        or login_query.get("redirect_uri") != [callback_url]
        or login_query.get("response_type") != ["code"]
    ):
        raise GateError("the TrailBase authorization request differs from the callback contract")
    pkce = login_query.get("pkce_code_challenge", [""])[0]
    if re.fullmatch(r"[A-Za-z0-9_-]{43}", pkce) is None:
        raise GateError("the TrailBase authorization request lacks exact S256 PKCE evidence")

    driver.find("css selector", "form#login-form")
    hidden_redirect = driver.find(
        "css selector", "#login-form input[type='hidden'][name='redirect_uri']"
    )
    hidden_response = driver.find(
        "css selector", "#login-form input[type='hidden'][name='response_type']"
    )
    hidden_pkce = driver.find(
        "css selector", "#login-form input[type='hidden'][name='pkce_code_challenge']"
    )
    hidden_mfa = driver.find(
        "css selector", "#login-form input[type='hidden'][name='mfa_redirect_uri']"
    )
    if (
        driver.attribute(hidden_redirect, "value") != callback_url
        or driver.attribute(hidden_response, "value") != "code"
        or driver.attribute(hidden_pkce, "value") != pkce
        or driver.attribute(hidden_mfa, "value") != "/_/auth/login_mfa"
    ):
        raise GateError("the TrailBase login form did not preserve exact callback evidence")

    email_input = driver.find(
        "css selector",
        "#login-form input[name='email'][type='email'][autocomplete='username']",
    )
    password_input = driver.find(
        "css selector",
        "#login-form input[name='password'][type='password'][autocomplete='current-password']",
    )
    driver.send_keys(email_input, email_address)
    driver.send_keys(password_input, password)
    submit = driver.find(
        "xpath",
        "//form[@id='login-form']//button[@type='submit' and normalize-space(.)='Sign In']",
    )
    if driver.text(submit).strip() != "Sign In":
        raise GateError("the TrailBase login form lacks its exact submit control")
    submitted = driver.execute(
        "const form = document.getElementById('login-form');"
        "if (!(form instanceof HTMLFormElement)) return false;"
        "form.requestSubmit(); return true;"
    )
    if submitted is not True:
        raise GateError("the TrailBase login form could not be submitted")

    try:
        final_callback_url = _wait_for(
            driver.current_url,
            lambda value: isinstance(value, str)
            and urllib.parse.urlsplit(value).scheme == "http"
            and urllib.parse.urlsplit(value).netloc == "127.0.0.1:8420"
            and urllib.parse.urlsplit(value).path == "/first-run"
            and not urllib.parse.urlsplit(value).query
            and not urllib.parse.urlsplit(value).fragment,
            timeout,
            "the Fasti authentication callback",
        )
    except GateError as error:
        current = urllib.parse.urlsplit(driver.current_url())
        body = driver.text(driver.find("tag name", "body"))
        markers = {
            marker: marker.casefold() in body.casefold()
            for marker in (
                "Email not verified",
                "Invalid email or password",
                "Fasti request failed",
                "TrailBase",
            )
        }
        raise GateError(
            "the Fasti authentication callback did not complete; "
            f"location={current.scheme}://{current.netloc}{current.path}; "
            f"query_present={bool(current.query)}; "
            f"fragment_present={bool(current.fragment)}; markers={markers}"
        ) from error
    if final_callback_url != f"{FASTI_ORIGIN}/first-run":
        raise GateError("the first-administrator callback did not return to exact first-run")

    _wait_for(
        lambda: driver.find(
            "xpath",
            "//li[.//strong[normalize-space(.)='Account confirmed']]"
            "//span[contains(@class,'badge') and normalize-space(.)='verified']",
        ),
        lambda value: isinstance(value, str),
        timeout,
        "verified first-administrator binding",
    )
    for step in ("Strong sign-in", "Recovery", "Devices and clients", "External identity"):
        _wait_for(
            lambda step=step: driver.find(
                "xpath",
                f"//li[.//strong[normalize-space(.)='{step}']]"
                "//span[contains(@class,'badge') and normalize-space(.)='unavailable']",
            ),
            lambda value: isinstance(value, str),
            timeout,
            f"unavailable {step} first-run state",
        )

    cookie_evidence = _validate_fasti_cookies(driver.cookies(), password)
    storage_evidence = _validate_browser_storage(driver, password)
    driver.dom_click(
        driver.find("xpath", "//button[normalize-space(.)='Save and finish later']")
    )
    permanent_url = _wait_for(
        driver.current_url,
        lambda value: value == f"{FASTI_ORIGIN}/settings/account",
        timeout,
        "the permanent Account and security task map",
    )
    parsed_permanent = urllib.parse.urlsplit(permanent_url)
    if parsed_permanent.query or parsed_permanent.fragment:
        raise GateError("the permanent account route retained callback parameters")
    driver.find("xpath", "//*[@id='account-security-title' and normalize-space(.)='Account and security']")
    driver.find("css selector", "[aria-label='Account and security tasks']")
    _wait_for(
        lambda: driver.find(
            "xpath",
            "//dt[normalize-space(.)='TrailBase trust']"
            "/following-sibling::dd[1][normalize-space(.)='active']",
        ),
        lambda value: isinstance(value, str),
        timeout,
        "active TrailBase trust evidence",
    )
    _wait_for(
        lambda: driver.find(
            "xpath",
            "//dt[normalize-space(.)='Workspace role']"
            "/following-sibling::dd[1][normalize-space(.)='administrator']",
        ),
        lambda value: isinstance(value, str),
        timeout,
        "administrator workspace-role evidence",
    )
    _wait_for(
        lambda: driver.find(
            "xpath", "//th[@scope='row' and normalize-space(.)='This browser']"
        ),
        lambda value: isinstance(value, str),
        timeout,
        "current packaged-WebView session evidence",
    )
    return {
        "setup": True,
        "first_run": True,
        "first_administrator_binding": True,
        "trailbase_sign_in": True,
        "fasti_callback": True,
        "first_run_account_confirmed_verified": True,
        "later_first_run_steps_unavailable": True,
        "permanent_account_task_map": True,
        "current_session_visible": True,
        "cookies": cookie_evidence,
        "browser_storage": storage_evidence,
        "binding_consumption": {
            "runtime_cookie_listing_not_used": True,
            "set_cookie_contract_tests": [
                "fasti_api::trailbase::tests::"
                "packaged_runtime_bootstrap_start_and_http_callback_share_one_pkce_vault",
                "fasti_api::access::tests::"
                "session_cookie_attributes_keep_credentials_out_of_script_and_cross_site_requests",
            ],
        },
    }


def _redacted_access_state(data_root: Path) -> dict[str, object]:
    database = data_root / "current/fasti.sqlite3"
    if not database.is_file() or database.is_symlink():
        return {"database_available": False}
    try:
        connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
        try:
            ceremonies = connection.execute(
                "SELECT purpose, return_target, state, failure "
                "FROM auth_ceremonies ORDER BY created_at, operation_id LIMIT 8"
            ).fetchall()
            session_count = connection.execute(
                "SELECT COUNT(*) FROM fasti_browser_sessions"
            ).fetchone()[0]
        finally:
            connection.close()
    except sqlite3.Error:
        return {"database_available": True, "query_succeeded": False}
    return {
        "database_available": True,
        "query_succeeded": True,
        "ceremonies": [list(ceremony) for ceremony in ceremonies],
        "browser_session_count": session_count,
    }


def _base_environment(workspace: Path, trailbase_root: Path, data_root: Path) -> dict[str, str]:
    environment = {
        key: value
        for key, value in os.environ.items()
        if key
        not in {
            "FASTI_DATA_ROOT",
            "FASTI_TRAILBASE_ROOT",
            "DBUS_SESSION_BUS_ADDRESS",
            "GNOME_KEYRING_CONTROL",
        }
        and not key.startswith("TRAIL_")
    }
    home = workspace / "home"
    runtime_directory = workspace / "run"
    xdg = workspace / "xdg"
    for path in (
        home,
        runtime_directory,
        xdg / "data",
        xdg / "config",
        xdg / "cache",
        xdg / "state",
    ):
        _private_directory(path)
    wayland_display = environment.get("WAYLAND_DISPLAY")
    source_runtime = environment.get("XDG_RUNTIME_DIR")
    if wayland_display:
        if "/" in wayland_display or not source_runtime:
            raise GateError("the active Wayland display has an unsafe runtime path")
        source_socket = Path(source_runtime) / wayland_display
        if not source_socket.is_socket():
            raise GateError("the active Wayland display socket is unavailable")
        os.symlink(source_socket, runtime_directory / wayland_display)
    environment.update(
        {
            "HOME": str(home),
            "XDG_RUNTIME_DIR": str(runtime_directory),
            "XDG_DATA_HOME": str(xdg / "data"),
            "XDG_CONFIG_HOME": str(xdg / "config"),
            "XDG_CACHE_HOME": str(xdg / "cache"),
            "XDG_STATE_HOME": str(xdg / "state"),
            "FASTI_DATA_ROOT": str(data_root),
            "FASTI_TRAILBASE_ROOT": str(trailbase_root),
            "NO_PROXY": "127.0.0.1,localhost",
            "no_proxy": "127.0.0.1,localhost",
            "RUST_LOG": "warn",
        }
    )
    return environment


def run_gate(arguments: argparse.Namespace) -> dict[str, object]:
    if platform.system() != "Linux":
        raise GateError("tauri-driver packaged-WebView evidence is Linux-only")
    if not os.environ.get("DISPLAY") and not os.environ.get("WAYLAND_DISPLAY"):
        raise GateError("the packaged-WebView gate requires an active display session")
    initial_status = _require_clean_source()
    network_evidence = _route_less_evidence()
    git_commit = _git_text("rev-parse", "HEAD")
    git_tree = _git_text("rev-parse", "HEAD^{tree}")
    tauri_driver = _command_path(arguments.tauri_driver, "tauri-driver")
    webkit_driver = _command_path(arguments.webkit_webdriver, "WebKitWebDriver")
    dbus_daemon = _command_path(None, "dbus-daemon")
    keyring_daemon = _command_path(None, "gnome-keyring-daemon")
    _command_path(None, "dpkg-deb")
    _command_path(None, "dpkg-query")
    driver_evidence = _driver_tool_evidence(tauri_driver, webkit_driver)
    for port, label in (
        (4000, "TrailBase accounts"),
        (4001, "TrailBase administration"),
        (4444, "tauri-driver"),
        (8420, "Fasti Access"),
    ):
        _require_free_port(port, label)

    source_root = arguments.trailbase_root.resolve(strict=True)
    runtime.verify_installation(source_root)
    started = time.monotonic()
    smoke = _load_trailbase_smoke()
    trailbase_process: subprocess.Popen[bytes] | None = None
    smtp: Any = None
    smtp_thread: threading.Thread | None = None
    bus: subprocess.Popen[str] | None = None
    keyring: subprocess.Popen[bytes] | None = None
    tauri_process: subprocess.Popen[bytes] | None = None
    driver = W3CClient(timeout=min(arguments.timeout, 15))
    human_email = ""
    human_password = ""
    desktop_digest = ""
    desktop_size = 0
    receipt: dict[str, Any] = {}
    checks: dict[str, object] = {}
    process_evidence: dict[str, object] = {}
    installed_desktop: Path | None = None
    temporary = tempfile.TemporaryDirectory(
        prefix="fasti-c1-webdriver-",
        dir=Path.home(),
    )
    try:
        workspace = Path(temporary.name)
        os.chmod(workspace, 0o700)
        trailbase_root = workspace / "trailbase"
        data_root = workspace / "fasti-data"
        _private_directory(data_root)
        executable = _copy_exact_release_input(source_root, trailbase_root)
        _bootstrap_disposable_installation(smoke, executable, trailbase_root)

        smtp = smoke.SmtpServer(("127.0.0.1", 0))
        smtp_port = int(smtp.server_address[1])
        _write_fixture_config(trailbase_root, smtp_port)
        runtime.prepare_runtime_lock(trailbase_root)
        receipt = runtime.prepare_installation(trailbase_root, "native")
        verified = runtime.verify_installation(trailbase_root)
        if verified != receipt or receipt.get("declared_restore") is not False:
            raise GateError("the disposable TrailBase installation is not fresh and exact")

        smtp_thread = threading.Thread(target=smtp.serve_forever, daemon=True)
        smtp_thread.start()
        trailbase_process, _ = smoke.start_fixture_release(
            executable,
            trailbase_root,
            4000,
        )
        human_email, human_password = _register_verified_human(smoke, smtp.messages)

        installation = workspace / "installation/fasti-desktop"
        desktop_digest, desktop_size = _build_and_copy_desktop(
            arguments.desktop_binary.resolve(),
            installation,
        )
        installed_desktop = installation
        environment = _base_environment(workspace, trailbase_root, data_root)
        path_entries = [str(webkit_driver.parent), environment.get("PATH", "")]
        environment["PATH"] = os.pathsep.join(entry for entry in path_entries if entry)
        bus, keyring, environment = _start_private_secret_service(
            workspace,
            dbus_daemon,
            keyring_daemon,
            environment,
        )
        tauri_process = runtime.start_managed_process_group(
            [
                str(tauri_driver),
                "--port",
                "4444",
                "--native-driver",
                str(webkit_driver),
            ],
            environment=environment,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        driver.wait_ready(arguments.timeout)
        try:
            checks = _drive_authenticated_flow(
                driver,
                installation,
                data_root,
                human_email,
                human_password,
                arguments.timeout,
            )
        except GateError as error:
            raise GateError(
                f"{error}; access_state={json.dumps(_redacted_access_state(data_root), sort_keys=True)}"
            ) from error
        process_evidence = _packaged_process_evidence(
            tauri_process,
            installation,
            desktop_digest,
            tauri_driver,
            webkit_driver,
            arguments.timeout,
        )
        driver.close()
        if runtime.sha256_file(installation) != desktop_digest:
            raise GateError("the copied desktop artifact changed while it ran")
    finally:
        human_email = ""
        human_password = ""
        driver.close()
        _stop_process(tauri_process)
        _stop_process(keyring)
        _stop_process(bus)
        if trailbase_process is not None:
            smoke.stop_process(trailbase_process)
        if smtp is not None and smtp_thread is not None:
            smtp.shutdown()
        if smtp is not None:
            smtp.server_close()
        if smtp_thread is not None:
            smtp_thread.join(timeout=2)
        temporary.cleanup()

    if installed_desktop is not None and installed_desktop.exists():
        raise GateError("the disposable desktop installation was not removed")
    if _git_text("status", "--porcelain=v1", "--untracked-files=all") != initial_status:
        raise GateError("the packaged-WebView gate changed the Git worktree")
    if _git_text("rev-parse", "HEAD") != git_commit or _git_text("rev-parse", "HEAD^{tree}") != git_tree:
        raise GateError("the source identity changed during the packaged-WebView gate")
    return {
        "schema_version": "fasti.access-desktop-webdriver-smoke.v1",
        "status": "PASS",
        "source": {"git_commit": git_commit, "git_tree": git_tree},
        "desktop": {
            "artifact": "copied-release-binary",
            "sha256": desktop_digest,
            "size_bytes": desktop_size,
            "drivers": driver_evidence,
            "processes": process_evidence,
        },
        "trailbase": {
            "release": runtime.load_release()["version"],
            "runtime": receipt["runtime"],
            "runtime_target": receipt["runtime_target"],
            "artifact_identity": receipt["artifact_identity"],
            "declared_restore": receipt["declared_restore"],
            "disposable_admin_rotated": True,
            "distinct_human_registered": True,
            "local_email_verified": True,
        },
        "checks": checks,
        "network_isolation": network_evidence,
        "credential_store": "isolated-disposable-secret-service",
        "secret_output": False,
        "duration_ms": round((time.monotonic() - started) * 1000),
    }


def self_test() -> None:
    smoke = _load_trailbase_smoke()
    smtp = smoke.SmtpServer(("127.0.0.1", 0))
    thread = threading.Thread(target=smtp.serve_forever, daemon=True)
    thread.start()
    token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJjMS1zZWxmLXRlc3QifQ.signature"
    try:
        with smtplib.SMTP("127.0.0.1", int(smtp.server_address[1]), timeout=5) as client:
            client.sendmail(
                "noreply@fasti.test",
                ["person@fasti.test"],
                f"Subject: verify\r\n\r\nUse {token}\r\n",
            )
        if smoke.email_token(smtp.messages) != token:
            raise GateError("the local SMTP capture self-test lost its verification token")
    finally:
        token = ""
        smtp.shutdown()
        smtp.server_close()
        thread.join(timeout=2)

    cookies = [
        {
            "name": SESSION_COOKIE,
            "value": "a" * 64,
            "domain": "127.0.0.1",
            "path": "/",
            "expiry": int(time.time()) + 60,
            "secure": True,
            "httpOnly": True,
            "sameSite": "Strict",
        },
        {
            "name": CSRF_COOKIE,
            "value": "b" * 64,
            "domain": "127.0.0.1",
            "path": "/",
            "expiry": int(time.time()) + 60,
            "secure": True,
            "httpOnly": False,
            "sameSite": "Strict",
        },
    ]
    evidence = _validate_fasti_cookies(cookies, "disposable-password")
    if any(cookie.get("value") for cookie in cookies):
        raise GateError("the cookie self-test retained secret values")
    encoded = json.dumps(evidence, sort_keys=True)
    if "a" * 64 in encoded or "b" * 64 in encoded:
        raise GateError("the cookie evidence contains a secret value")
    if not _contains_credential_value("prefix-disposable-password-suffix", "disposable-password"):
        raise GateError("the credential redaction self-test missed a generated password")
    if not _contains_credential_value("header.payload.signature", "disposable-password"):
        raise GateError("the credential redaction self-test missed a compact token")
    if not _contains_credential_value(
        '{"token":"header.payload.signature"}', "disposable-password"
    ):
        raise GateError("the credential redaction self-test missed an embedded compact token")
    if any(
        CREDENTIAL_NAME.search(name) is None
        for name in ("access_token", "id-token", "refresh-token")
    ):
        raise GateError("the credential redaction self-test missed a credential name")
    print("PASS: packaged-WebView SMTP, cookie, and redaction self-checks")


def main() -> int:
    runtime.install_termination_cleanup()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--trailbase-root", type=Path, default=SOURCE_TRAILBASE_ROOT)
    parser.add_argument("--desktop-binary", type=Path, default=RELEASE_DESKTOP)
    parser.add_argument(
        "--tauri-driver",
        type=Path,
        default=PRIVATE_TAURI_DRIVER if PRIVATE_TAURI_DRIVER.is_file() else None,
    )
    parser.add_argument(
        "--webkit-webdriver",
        type=Path,
        default=PRIVATE_WEBKIT_DRIVER if PRIVATE_WEBKIT_DRIVER.is_file() else None,
    )
    parser.add_argument("--timeout", type=float, default=45)
    arguments = parser.parse_args()
    try:
        if arguments.self_test:
            self_test()
        else:
            if arguments.timeout < 5 or arguments.timeout > 300:
                raise GateError("--timeout must be between 5 and 300 seconds")
            _enter_route_less_namespace()
            print(json.dumps(run_gate(arguments), sort_keys=True))
        return 0
    except (
        AssertionError,
        GateError,
        OSError,
        runtime.ReleaseError,
        subprocess.SubprocessError,
    ) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
