#!/usr/bin/env python3
"""Qualify scoped artwork in the real Desktop WebView, without sign-in tests."""

from __future__ import annotations

import argparse
import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "desktop_driver_helpers", ROOT / "scripts/smoke-desktop-access-webdriver.py"
)
assert SPEC and SPEC.loader
native = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(native)
SEED_TEST = "artwork::native_fixture::seed_native_artwork_fixture"


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def network_evidence() -> dict[str, object]:
    parent = os.environ.get("FASTI_ARTWORK_PARENT_NETNS")
    current = os.readlink("/proc/self/ns/net")
    if not parent or current == parent:
        raise native.GateError("artwork fixture requires a separate network namespace")
    for command in (["ip", "route", "show"], ["ip", "-6", "route", "show"]):
        if native._run(command, timeout=10).stdout.strip():
            raise native.GateError("artwork fixture must have no external network routes")
    return {"network_namespace": current, "loopback_only": True, "external_routes": 0}


def enter_namespace() -> None:
    if "FASTI_ARTWORK_PARENT_NETNS" not in os.environ:
        executable = native._command_path(None, "unshare")
        environment = dict(os.environ)
        environment["FASTI_ARTWORK_PARENT_NETNS"] = os.readlink("/proc/self/ns/net")
        os.execve(executable, [str(executable), "--user", "--map-root-user", "--net",
                              "--", sys.executable, "-B", str(Path(__file__).resolve()),
                              *sys.argv[1:]], environment)
    network_evidence()
    native._run(["ip", "link", "set", "lo", "up"], timeout=10)


def image_state(driver: object) -> dict[str, object] | None:
    return driver.execute("""
        const image = document.querySelector('img.main-poster');
        return image ? {src: image.src, complete: image.complete,
            width: image.naturalWidth, height: image.naturalHeight,
            alt: image.alt, error: window.__fastiArtworkRejected === image.src} : null;
    """)


def wait_image(driver: object, sources: tuple[str, ...], dimensions: tuple[int, int],
               timeout: float = 30) -> dict[str, object]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        state = image_state(driver)
        if (state and state["src"] in sources and state["complete"]
                and (state["width"], state["height"]) == dimensions
                and (dimensions != (0, 0) or state["error"])):
            return state
        time.sleep(0.1)
    raise native.GateError("the actual Record poster did not reach the expected decoded state")


def run(arguments: argparse.Namespace) -> Path:
    enter_namespace()
    native.runtime.install_termination_cleanup()
    if not os.environ.get("DISPLAY"):
        raise native.GateError("run with xvfb-run for a disposable native display")
    os.umask(0o077)
    for port in (8444, 8445, 8420):
        native._require_free_port(port, "isolated artwork fixture")
    desktop = arguments.desktop_binary.resolve(strict=True)
    seeder = arguments.seed_binary.resolve(strict=True)
    tools = (
        native.PRIVATE_TAURI_DRIVER,
        native.PRIVATE_WEBKIT_DRIVER,
        Path("/usr/bin/dbus-daemon"),
        Path("/usr/bin/gnome-keyring-daemon"),
    )
    for path in (desktop, seeder, *tools):
        native._command_path(path, path.name)
    output_root = ROOT / "target/native-artwork"
    native._private_directory(output_root)
    output = Path(tempfile.mkdtemp(prefix="run-", dir=output_root))
    receipt = {
        "kind": "fasti.desktop.artwork-native-fixture",
        "status": "fail",
        "scope": {"synthetic_offline_cache": True, "real_desktop_webview": True,
                  "provider_acquisition": False, "packaged_authentication": False},
        "source": {"commit": native._git_text("rev-parse", "HEAD"),
                   "tree": native._git_text("rev-parse", "HEAD^{tree}"),
                   "dirty": bool(native._git_text("status", "--porcelain=v1"))},
        "desktop_sha256": digest(desktop), "seed_binary_sha256": digest(seeder),
        "harness_sha256": digest(Path(__file__)),
        "network": network_evidence(), "checks": {}, "stage": "prepare",
    }
    driver = native.W3CClient("http://127.0.0.1:8444", timeout=10)
    bus = keyring = driver_process = None
    try:
        with tempfile.TemporaryDirectory(prefix="fasti-native-artwork-") as temporary:
            workspace = Path(temporary).resolve(strict=True)
            for child in ("data", "cache", "config", "user-data", "state", "run"):
                native._private_directory(workspace / child)
            (workspace / ".fasti-native-artwork-fixture").write_text(
                "disposable native artwork fixture\n", encoding="ascii"
            )
            # No provider secrets, ambient bus, proxies, or TrailBase configuration.
            # Preserve HOME's value; isolate app state with the native XDG paths.
            environment = {name: os.environ[name] for name in
                           ("PATH", "HOME", "LANG", "LC_ALL", "DISPLAY", "XAUTHORITY", "TMPDIR")
                           if name in os.environ}
            environment.update({
                "FASTI_NATIVE_ARTWORK_FIXTURE_ROOT": str(workspace),
                "FASTI_DATA_ROOT": str(workspace / "data"),
                "XDG_CACHE_HOME": str(workspace / "cache"),
                "XDG_CONFIG_HOME": str(workspace / "config"),
                "XDG_DATA_HOME": str(workspace / "user-data"),
                "XDG_STATE_HOME": str(workspace / "state"),
                "XDG_RUNTIME_DIR": str(workspace / "run"),
                "GDK_BACKEND": "x11",
                "RUST_LOG": "warn",
            })
            try:
                receipt["stage"] = "private-keyring"
                bus, keyring, environment = native._start_private_secret_service(
                    workspace, tools[2], tools[3], environment
                )
                receipt["stage"] = "seed"
                seed = subprocess.run(
                    [str(seeder), "--exact", SEED_TEST, "--ignored", "--test-threads=1"],
                    env=environment, stdin=subprocess.DEVNULL, capture_output=True, timeout=60,
                )
                if seed.returncode:
                    (output / "seed-stderr.txt").write_bytes(seed.stderr[-65536:])
                    (output / "seed-stdout.txt").write_bytes(seed.stdout[-65536:])
                    raise native.GateError("disposable artwork seeding failed; private diagnostics retained")
                fixture = json.loads((workspace / "artwork-fixture.json").read_text())
                receipt["fixture"] = fixture
                installation = workspace / "fasti-desktop"
                shutil.copyfile(desktop, installation)
                installation.chmod(0o700)
                if digest(installation) != receipt["desktop_sha256"]:
                    raise native.GateError("copied Desktop artifact differs")
                receipt["stage"] = "native-driver"
                driver_process = native.runtime.start_managed_process_group(
                    [str(tools[0]), "--port", "8444", "--native-port", "8445",
                     "--native-host", "127.0.0.1", "--native-driver", str(tools[1])],
                    environment=environment, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                )
                driver.wait_ready(15)
                driver.create_session(installation)
                receipt["stage"] = "record-route"
                route = f"http://127.0.0.1:8420/records/film/{fixture['record_id']}/synthetic-native-artwork-fixture"
                driver.navigate(route)
                expected_sources = tuple(prefix + fixture["locator"] for prefix in
                                         ("asset://localhost/", "http://asset.localhost/"))
                dimensions = (fixture["width"], fixture["height"])
                image = wait_image(driver, expected_sources, dimensions)
                receipt["checks"]["canonical_record_image"] = image
                receipt["stage"] = "reload"
                driver.navigate(route)
                receipt["checks"]["reload_image"] = wait_image(driver, expected_sources, dimensions)
                receipt["stage"] = "reject-query"
                rejected_source = image["src"] + "?denied=1"
                driver.execute("""
                    const image = document.querySelector('img.main-poster');
                    window.__fastiArtworkRejected = null;
                    image.addEventListener('error', () => {
                        window.__fastiArtworkRejected = image.src;
                    }, {once: true});
                    image.src = """ + json.dumps(rejected_source) + "; return true;")
                receipt["checks"]["query_rejected"] = wait_image(driver, (rejected_source,), (0, 0))
                receipt["stage"] = "restore-route"
                driver.navigate(route)
                receipt["checks"]["restored_image"] = wait_image(driver, expected_sources, dimensions)
                screenshot = driver._request("GET", driver._session_path("/screenshot"))
                if not isinstance(screenshot, str) or len(screenshot) > 8_000_000:
                    raise native.GateError("native screenshot exceeds its bound")
                (output / "record.png").write_bytes(base64.b64decode(screenshot, validate=True))
                receipt["network_after"] = network_evidence()
                receipt["status"] = "pass"
                receipt["stage"] = "complete"
            except Exception:
                if driver.session_id:
                    try:
                        receipt["diagnostic"] = driver.execute("""
                            return {url: location.href, image: !!document.querySelector('img.main-poster'),
                                headings: [...document.querySelectorAll('h1,h2')].slice(0,8).map(x=>x.textContent.slice(0,160))};
                        """)
                    except native.GateError:
                        pass
                raise
            finally:
                driver.close()
                cleanup_failed = False
                for process in (driver_process, keyring, bus):
                    try:
                        native._stop_process(process)
                    except Exception:
                        cleanup_failed = True
                if cleanup_failed:
                    receipt["status"] = "fail"
                    raise native.GateError("a managed fixture process did not stop cleanly")
    except BaseException as error:
        receipt["status"] = "fail"
        receipt["error"] = str(error) if isinstance(error, native.GateError) else type(error).__name__
        if not isinstance(error, Exception):
            raise
    finally:
        (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(output / "receipt.json", flush=True)
    if receipt["status"] != "pass":
        raise native.GateError(f"native artwork fixture failed at {receipt['stage']}")
    return output


def self_test() -> None:
    from unittest.mock import patch

    source = "asset://localhost/fasti-artwork.fixture"
    baseline = {"src": source, "complete": True, "width": 512, "height": 512, "error": False}

    class Driver:
        def __init__(self, state):
            self.state = state

        def execute(self, _script):
            return self.state

    cases = [
        ({}, (512, 512), True),
        ({"src": source + ".old"}, (512, 512), False),
        ({"height": 1}, (512, 512), False),
        ({"complete": False}, (512, 512), False),
        ({"width": 0, "height": 0}, (0, 0), False),
        ({"width": 0, "height": 0, "error": True}, (0, 0), True),
        ({"width": 0, "height": 0, "error": True, "src": source + ".old"}, (0, 0), False),
    ]
    for changes, dimensions, expected in cases:
        with patch.object(time, "monotonic", side_effect=[0, 0, 2]), patch.object(time, "sleep"):
            try:
                wait_image(Driver(baseline | changes), (source,), dimensions, timeout=1)
            except native.GateError:
                accepted = False
            else:
                accepted = True
        assert accepted == expected, f"image predicate differs: {changes}"

    # Stop before paths, processes, namespaces, or fixture writes.
    with patch.dict(os.environ, {"DISPLAY": ""}), \
            patch(__name__ + ".enter_namespace") as namespace, \
            patch.object(native.runtime, "install_termination_cleanup") as cleanup:
        try:
            run(argparse.Namespace())
        except native.GateError as error:
            assert "disposable native display" in str(error)
        else:
            raise AssertionError("missing display must stop the harness")
        namespace.assert_called_once_with()
        cleanup.assert_called_once_with()
    print("PASS: native artwork predicates and cleanup registration")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--desktop-binary", type=Path)
    parser.add_argument("--seed-binary", type=Path)
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    try:
        if arguments.self_test:
            self_test()
        elif arguments.desktop_binary is None or arguments.seed_binary is None:
            parser.error("--desktop-binary and --seed-binary are required for the native fixture")
        else:
            run(arguments)
    except native.GateError as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
