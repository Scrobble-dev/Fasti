# B1 native and OCI performance evidence

This suite captures the B1 headless baseline without turning a desktop sample into constrained-hardware proof. Capture is Linux-only and requires a clean Git tree, a source-labeled local OCI image, route-less user/network namespaces, a demonstrably local Docker daemon on a Unix socket, Docker network mode `none`, and cgroup v2.

The harness builds the governed image itself from a verifier-owned `git archive HEAD` extraction, never from the live checkout. Ignored files, uncommitted files, and files changed while Docker is building cannot enter the context. The resulting image labels bind exact `HEAD`, `HEAD^{tree}`, `HEAD:contracts`, the Dockerfile digest, and the exact Git-archive digest. It then runs only the immutable `sha256:...` image ID and extracts `/usr/local/bin/fastid` from those same image bytes; an unrelated prebuilt binary cannot enter a qualifying run. Every measured container uses `--network none`. Before sampling, the reported `State.Pid` must exist in a local cgroup-v2 path containing the exact container ID; a Unix-socket proxy to a remote daemon therefore cannot masquerade as local cgroup evidence.

## Canonical budgets

[`budgets.json`](budgets.json) is the single machine-readable owner of the memory budgets:

| Budget           |       Bytes | MiB | B1 disposition                                                                      |
| ---------------- | ----------: | --: | ----------------------------------------------------------------------------------- |
| Idle target      |  67,108,864 |  64 | Evaluated against the largest steady observation within any run, then the worst run |
| Normal target    | 100,663,296 |  96 | `not_applicable`; B1 has no implemented normal-operation workload                   |
| Heavy target     | 167,772,160 | 160 | `not_applicable`; B1 has no implemented heavy-operation workload                    |
| Absolute ceiling | 201,326,592 | 192 | Evaluated against the worst Fasti daemon or guarded-CLI process-tree/cgroup peak    |

Normal and heavy remain visible, but B1 cannot pass them by reusing an idle sample. The validator rejects that claim.

The same document locks the idle CPU, measurement-duration, and distribution-artifact gates:

| Gate                            |            Limit |
| ------------------------------- | ---------------: |
| Idle CPU average                | 0.5% of one core |
| Idle CPU p95                    | 1.0% of one core |
| Idle warm-up                    |      600 seconds |
| Network-denied idle measurement |      900 seconds |
| Raw observation interval        |         1,000 ms |
| Native installed runtime        |           32 MiB |
| Native compressed archive       |           20 MiB |
| Compressed OCI image            |           50 MiB |
| Unpacked OCI image              |          100 MiB |
| Compressed contract pack        |            5 MiB |

Idle CPU is derived independently for native and OCI daemon runs and evaluated against the worst of at least five runs. Artifact results are first-class verdicts; an omitted or hand-entered result cannot close B1.

## Measured scenarios

Every complete receipt contains exactly five scenarios in this order:

| Scenario               | Boundary                                                                       | Readiness or completion proof                                                                      |
| ---------------------- | ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------- |
| `native_empty_process` | Full `/bin/sleep` process tree in a fresh route-less Linux network namespace   | Namespace creation succeeds and `ip route show` is empty                                           |
| `native_fastid_idle`   | Full native launcher/daemon process tree in the same isolation                 | Loopback health succeeds from inside the namespace                                                 |
| `oci_empty_process`    | Whole shell/sleep process tree and cgroup v2 in a fresh network-none container | Container readiness marker                                                                         |
| `oci_fastid_idle`      | Whole daemon process tree and cgroup v2 in a fresh network-none container      | Loopback health succeeds through `docker exec`                                                     |
| `oci_fasti_cli_guard`  | Guarded CLI launch peak plus its retained wrapper process tree and cgroup v2   | `fasti verify` exits nonzero, writes nothing to stdout, and emits its explicit unavailable message |

The retained CLI wrapper keeps the cgroup alive long enough to read `memory.peak`. Its steady value describes the post-command wrapper, not a long-running CLI. The peak includes the CLI launch. Sampling interval and exact commands remain in the receipt.

For native subjects the harness records startup, steady and peak process-tree RSS, process-tree CPU, process count, binary size, and exact commands. Native CPU comes from `/proc/<pid>/schedstat` runtime nanoseconds, so the 0.5% and 1% gates are measurable instead of being rounded to scheduler jiffies. OCI subjects add raw cgroup `memory.current`, `memory.peak`, and `cpu.stat usage_usec` counters, plus image and in-image binary sizes. Every named observation is retained in the receipt with a monotonic elapsed nanosecond, steady-window marker, memory, CPU-runtime counter, and process count. The validator independently recomputes sample summaries, scenario summaries, idle CPU, and verdicts and rejects duplicated observations. Empty-process results are never subtracted.

## Run

Install the locked workspace dependencies from a clean checkout:

```bash
pnpm install --frozen-lockfile
```

Run the non-mutating JSON preflight on the named physical device before reserving the measurement window:

```bash
python3 -B scripts/benchmark-b1.py preflight \
  --image fasti:b1 \
  --runner-id pi5-lab-01 \
  --custodian "named physical custodian" \
  --os-image /retained/path/to/os-image.img
```

The harness opens the retained OS image as a regular file with `O_NOFOLLOW`, hashes the actual bytes, and records its basename, size, and digest. An operator-supplied digest is not accepted. The preflight emits either a machine-readable pass or a refusal with the unmet requirement and next action.

On the named Linux device, capture at least five independent repetitions. The harness builds the image itself from the pinned multi-architecture [`Dockerfile`](Dockerfile), binds the build to exact source and contract objects, resolves the result to an immutable image ID, and extracts the native binary from those same image bytes. It does not accept an independently built image as evidence. The locked idle windows alone require at least 4 hours 10 minutes across five native and five OCI idle runs, before build and non-idle scenarios.

The hardware profile is derived from [`physical-profiles.json`](physical-profiles.json); there is no operator-supplied profile flag and Python/JavaScript do not maintain separate profile literals. Raspberry Pi 5 requires the exact `Raspberry Pi 5 Model B Rev x.y` device-tree grammar, both `raspberrypi,5-model-b` and `brcm,bcm2712` compatible values, `systemd-detect-virt` to prove `none`, no hypervisor flag, 4 GB class memory, active cooling, canonical stock-clock rules, and `/` on a positively identified USB SSD at 5,000 Mb/s or faster. J4125 requires the canonical CPU grammar, four logical cores, `systemd-detect-virt` to prove `none`, non-virtual DMI evidence, and `/` on a positively identified SSD using an accepted transport. Non-rotational eMMC or flash is not relabeled as SSD.

The checked-in Raspberry Pi image digest allowlist is intentionally empty until an authoritative official image digest is pinned. Therefore Pi preflight currently stops with an explicit image-policy blocker. `/etc/os-release` can prove runtime release fields; it cannot prove that an arbitrary retained image is Raspberry Pi OS Lite. J4125 records its retained image digest without making an edition claim.

```bash
python3 -B scripts/benchmark-b1.py capture \
  --image fasti:b1 \
  --runner-id pi5-lab-01 \
  --custodian "named physical custodian" \
  --os-image /retained/path/to/os-image.img \
  --repetitions 5 \
  --output benchmarks/b1/evidence/pi5-lab-01.json
```

The harness writes through a validated temporary file and refuses to overwrite an existing receipt. Before validation it atomically publishes the exact compressed OCI image and contract-pack bytes at receipt-relative `artifacts/sha256/<digest>.tar.gz` paths. Each receipt records path, digest, and size under `retained_artifacts`; the validator opens each path once with `O_NOFOLLOW`, rejects escapes and symlinks, and verifies the same byte buffer. A budget failure still writes the valid measurement and exits nonzero, because a failing result is evidence rather than a capture error.

Validate a receipt independently:

```bash
node benchmarks/b1/validate-evidence.mjs benchmarks/b1/evidence/pi5-lab-01.json
```

Run the portable static and negative-sentinel tests on any development host:

```bash
node benchmarks/b1/validate-evidence.mjs --static
node benchmarks/b1/validate-evidence.mjs --self-test
python3 -B scripts/benchmark-b1.py self-test
python3 -B -m unittest benchmarks/b1/test_benchmark_b1.py
```

## Refusal conditions

No receipt is written when any required fact is unsupported or missing. Capture refuses:

- a non-Linux host, unavailable `/proc`, unavailable route-less user/network namespace, or unavailable cgroup v2;
- a dirty Git tree, missing governed Dockerfile or failed exact-source image build, remote/non-Unix Docker endpoint, non-Docker cgroup-v2 engine, or existing output path;
- a live-checkout Docker context, changed exact-HEAD Git archive, missing `O_NOFOLLOW`, symlinked retained input, or unapproved Raspberry Pi image digest;
- missing runner ID, physical custodian, fingerprint-derived hardware profile, exact `HEAD:contracts`, commit, tree, native digest, immutable image ID, or matching image source labels;
- a subject that exits early, fails readiness, gains an IP route or Docker network, produces an invalid guarded CLI result, or lacks a process-tree/cgroup sample;
- a source tree, `HEAD:contracts`, native binary, exact Git-archive context, Docker socket, mutable image reference, immutable image ID, or image source label that changes before measurement finishes;
- a receipt whose raw observations, summaries, budgets, retained artifact bytes, digests, scenarios, network proof, or verdicts do not validate.

The script does not fabricate a partial JSON document for any of those cases.

## Device qualification ledger

[`device-hypotheses.json`](device-hypotheses.json) names Ryan Winkler as the performance-gate owner and separately tracks the Raspberry Pi 5 champion, calibrated J4125 secondary, and four packaging hypotheses. Performance-gate ownership does not assign a physical runner: the Pi and J4125 custodians, exact runner fingerprints, artifact and contract references, evidence references, and results remain `null`. Every profile therefore remains honestly `blocking_unassigned` until a real custodian performs a validated run.

The four structured B4/B8 spikes are research inputs, not compatibility claims or qualification evidence:

| Profile                            | Exact target and documented constraints                                                                | Authoritative inputs                                                                                                                                                                                                |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| UGOOS AM6B Plus                    | Android 9.0; 4 GB LPDDR4; 32 GB eMMC                                                                   | [UGOOS product PDF](https://ugoos.com/files/uploads/63a38e470077cb39a0f8ca6933db3cbb.pdf)                                                                                                                           |
| Mi Box 3 MDZ-16-AB                 | Factory Android TV target; 2 GB DDR3; 8 GB eMMC; actual updated build must be recorded                 | [Xiaomi manual](https://ams-go.buy.mi.com/es/servicecenter/file/Mi_Box_es/?binaryId=11780&namespaceId=2&publicationId=18032), [Xiaomi declarations](https://www.mi.com/global/support/terms/declaration/)           |
| NVIDIA SHIELD TV Pro (2019, 16 GB) | Android 11; 3 GB RAM; 16 GB internal and USB storage; installed SHIELD build must be recorded          | [NVIDIA product page](https://www.nvidia.com/en-gb/shield/shield-tv-pro/), [NVIDIA support](https://www.nvidia.com/en-eu/shield/support/shield-tv-pro/)                                                             |
| Sony BRAVIA 3 K-43S30 (2024)       | Google TV / Android TV; 16 GB; firmware `6120800301`; exact installed Android version must be recorded | [Sony specifications](https://www.sony.com/electronics/support/televisions-projectors-lcd-tvs-android-/k-43s30/specifications), [Sony firmware](https://www.sony.com/electronics/support/product/k-43s30/downloads) |

Each packaging spike hypothesizes a locally signed APK installed through ADB over USB or a device-local package installer, followed by network-denied operation. Execution must record free and attached storage, the actual installed runtime and firmware, and the enabled background environment. Measurements explicitly include the vendor launcher, Google Play Services, Google Cast, Google voice services, and device-specific services such as Plex on SHIELD; their absence must be observed, never assumed. These entries remain `documented_unverified` and do not imply compatibility, physical evidence, or qualification.

The v3 ledger can represent `assigned_pending_evidence`, `evidence_validated`, `qualified`, and `calibrated` without weakening that checked-in state. For any evidence-backed state, the validator loads the referenced receipt, verifies its SHA-256, validates the receipt, and correlates custodian, physical fingerprint, source artifacts, contract object, and derived budget results. `qualified` requires both applicable B1 budgets to pass. `calibrated` additionally requires the exact evidence reference owned by a qualified Raspberry Pi 5 entry, matching git tree, contracts, harness, budgets, workload/scenario settings, and measured J4125-to-champion idle and absolute-memory ratios.

J4125 evidence does not stand in for the champion result. Its calibration relationship to the Raspberry Pi 5 must be defined and measured before it becomes a calibrated secondary signal.

## Governed empty Tauri packaging receipt

B1 also requires exactly one valid receipt for the benchmark-only empty Tauri shell under [`tauri-shell/`](tauri-shell/). This is packaging evidence for a possible B8 route, not a product desktop application, player, or authorization to start UI work.

A qualifying receipt must be captured on a governed real Linux desktop with Wayland or X11, a transient systemd user scope, cgroup v2 memory accounting, a route-less user network namespace, and at least five independent runs. macOS remains useful for the portable tests and locked build, but cannot qualify: WebKit XPC helpers may be reparented and therefore escape a parent-process RSS boundary. Simulated Xvfb or Weston sessions are also refused.

The receipt keeps the 96 MiB low-RAM target strong and applies the 192 MiB absolute ceiling. Missing evidence blocks B1 evidence completion. A measured target miss requires B8 tuning; an absolute breach blocks the Tauri packaging route rather than weakening the headless Fasti contracts. See the [Tauri fixture instructions](tauri-shell/README.md) for the exact capture command.

## Exact private runner handoff

Runner source can be transferred without publishing a branch or relying on the maintainer's object database. From a clean committed tree, create a self-contained bundle outside the checkout:

```bash
python3 -B scripts/package-b1-runner.py self-test
python3 -B scripts/package-b1-runner.py create \
  --output /private/handoff/fasti-b1-runner.bundle
```

The adjacent manifest binds the bundle to exact `HEAD`, `HEAD^{tree}`, and `HEAD:contracts`. Before using it, verify it in an empty bare repository and unpack the exact detached commit:

```bash
python3 -B scripts/package-b1-runner.py verify \
  --bundle /private/handoff/fasti-b1-runner.bundle \
  --manifest /private/handoff/fasti-b1-runner.manifest.json

python3 -B scripts/package-b1-runner.py unpack \
  --bundle /private/handoff/fasti-b1-runner.bundle \
  --manifest /private/handoff/fasti-b1-runner.manifest.json \
  --destination /private/runner/fasti-b1
```

Verification rejects prerequisite-dependent bundles, extra refs, digest or object mismatches, symlink inputs, traversal, and an existing destination. The handoff does not require or authorize a public remote.

## Files

- [`evidence.schema.json`](evidence.schema.json): JSON Schema 2020-12 receipt shape.
- [`budgets.json`](budgets.json): canonical budget values.
- [`budgets.schema.json`](budgets.schema.json): locked budget contract.
- [`physical-profiles.json`](physical-profiles.json): canonical Raspberry Pi 5 and J4125 physical-profile policy.
- [`physical-profiles.schema.json`](physical-profiles.schema.json): policy shape and locked semantics.
- [`device-hypotheses.json`](device-hypotheses.json): explicitly unassigned hardware ledger.
- [`device-hypotheses.schema.json`](device-hypotheses.schema.json): assignable ledger and calibration-state contract.
- [`validate-evidence.mjs`](validate-evidence.mjs): JSON Schema and semantic validation, including negative sentinels.
- [`test_benchmark_b1.py`](test_benchmark_b1.py): portable capture trust-boundary tests.
- [`Dockerfile`](Dockerfile): pinned multi-architecture governed OCI build recipe.
- [`runner-bundle.schema.json`](runner-bundle.schema.json): exact private handoff manifest contract.
- [`validate-runner-bundle.mjs`](validate-runner-bundle.mjs): canonical handoff schema validator.
- [`test_runner_bundle.py`](test_runner_bundle.py): private bundle integration and negative-sentinel tests.
- [`tauri-shell/`](tauri-shell/): non-product hidden packaging fixture and governed Linux receipt contract.
- [`../../scripts/benchmark-b1.py`](../../scripts/benchmark-b1.py): Linux capture harness.
- [`../../scripts/benchmark-tauri-b1.py`](../../scripts/benchmark-tauri-b1.py): governed empty-shell capture harness.
- [`../../scripts/package-b1-runner.py`](../../scripts/package-b1-runner.py): exact bundle create, verify, and unpack flow.
