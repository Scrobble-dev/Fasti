# B1 benchmark-only empty Tauri shell

This fixture measures a deliberately empty, hidden Tauri shell inside a dedicated Linux cgroup-v2 scope. It is evidence tooling for a possible B8 packaging route. It is not a Fasti desktop application, a player, a served web surface, or authorization to start product UI work.

The fixture has no commands, navigation, data model, or visible window. Its design-review and served-web applicability are therefore recorded as `N/A` with the reason `not_applicable_non_product_hidden_benchmark_fixture`. The receipt separately exposes the strong 96 MiB low-RAM target and the 192 MiB absolute ceiling. Missing the target requires B8 tuning; breaching the ceiling blocks that packaging route. Neither result weakens the B1 contract implementation.

## Portable checks

Run the schema mutation sentinel and harness tests on macOS or Linux:

```bash
node benchmarks/b1/tauri-shell/validate-evidence.mjs --self-test
python3 -B scripts/benchmark-tauri-b1.py self-test
python3 -B -m unittest benchmarks/b1/tauri-shell/test_benchmark_tauri.py
python3 -B scripts/benchmark-tauri-b1.py policy-check
```

Build the exact locked fixture without changing product workspace membership:

```bash
cargo build \
  --manifest-path benchmarks/b1/tauri-shell/src-tauri/Cargo.toml \
  --release \
  --locked \
  --offline
```

## Capture

Qualifying capture is Linux-only. It requires a real, active, local Wayland or X11 user session; a local display seat; at least one connected physical DRM connector; systemd user scopes; the cgroup-v2 memory controller; and a retained OS installation image digest. The harness rejects macOS attribution, remote or inactive sessions, headless hosts, and detected Xvfb, Xdummy, or headless-Weston processes. Portable macOS checks do not produce milestone evidence.

Capture also refuses a dirty tree, an existing output, fewer than five independent runs, a missing readiness marker, an early process exit, a route in the isolated namespace, or a source/artifact change during measurement. Every run launches the fixture inside a unique transient user scope and reads `memory.current`, `memory.peak`, and `cgroup.procs` from that exact cgroup. This includes WebKit processes that a launcher-only or parent-process boundary can miss.

```bash
python3 -B scripts/benchmark-tauri-b1.py capture \
  --runner-id linux-desktop-lab-01 \
  --os-image-path /retained/path/to/linux-image.iso \
  --repetitions 5 \
  --output benchmarks/b1/tauri-shell/evidence/linux-desktop-lab-01.json
```

The checked-in harness, Cargo lockfile, fixture tree, commit, repository tree, artifact digest, exact commands, OS image, firmware, filesystem, storage, CPU governor, temperature, WebKit runtime, local display-session proof, five raw cgroup samples, derived summaries, and worst observed cgroup peak are bound into the receipt. The exact measured binary is retained at mode `0600` under `evidence/artifacts/sha256-…-fasti-b1-tauri-shell`; its path, digest, and size are part of the receipt and the milestone manifest binds it as a `BuiltArtifact`. The validator refuses symlinks and escape paths and hashes one descriptor-owned byte snapshot.

The ignored `evidence/` directory is a private digest-bound evidence package. Transfer the receipt and its referenced artifact together through protected private storage; the exact-source runner bundle deliberately excludes both. Do not publish hardware receipts or measured binaries merely because the source repository is public. B1 milestone acceptance requires this governed Linux package as well as the physical Raspberry Pi 5 and J4125 packages specified by the controlling test plan.

Validate any receipt independently:

```bash
node benchmarks/b1/tauri-shell/validate-evidence.mjs \
  benchmarks/b1/tauri-shell/evidence/linux-desktop-lab-01.json
```

A missing receipt blocks B1 evidence completion. A result over the 96 MiB target but within the 192 MiB ceiling requires later B8 tuning. A result over the absolute ceiling blocks the Tauri packaging route; it does not relax the headless Fasti contract or turn this fixture into product UI.
