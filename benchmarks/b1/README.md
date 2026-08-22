# B1 native and OCI performance evidence

This suite captures the B1 headless baseline without turning a desktop sample into constrained-hardware proof. Capture is Linux-only and requires a clean Git tree, a source-labeled local OCI image, route-less user/network namespaces, a demonstrably local Docker daemon on a Unix socket, Docker network mode `none`, and cgroup v2.

The harness never pulls or builds an image. It resolves the operator's image reference once, verifies labels for exact `HEAD`, `HEAD^{tree}`, and `HEAD:contracts`, and runs only that immutable `sha256:...` image ID. It extracts `/usr/local/bin/fastid` from that same image ID and uses those exact bytes for the native scenarios; an unrelated prebuilt binary cannot enter a qualifying run. Every measured container uses `--network none`. Before sampling, the reported `State.Pid` must exist in a local cgroup-v2 path containing the exact container ID; a Unix-socket proxy to a remote daemon therefore cannot masquerade as local cgroup evidence. An ordinary unlabeled image remains useful for non-benchmark development, but it cannot produce qualifying evidence.

## Canonical budgets

[`budgets.json`](budgets.json) is the single machine-readable owner of the memory budgets:

| Budget           |       Bytes | MiB | B1 disposition                                                                      |
| ---------------- | ----------: | --: | ----------------------------------------------------------------------------------- |
| Idle target      |  67,108,864 |  64 | Evaluated against the largest steady observation within any run, then the worst run |
| Normal target    | 100,663,296 |  96 | `not_applicable`; B1 has no implemented normal-operation workload                   |
| Heavy target     | 167,772,160 | 160 | `not_applicable`; B1 has no implemented heavy-operation workload                    |
| Absolute ceiling | 201,326,592 | 192 | Evaluated against the worst Fasti daemon or guarded-CLI process-tree/cgroup peak    |

Normal and heavy remain visible, but B1 cannot pass them by reusing an idle sample. The validator rejects that claim.

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

For native subjects the harness records startup, steady and peak process-tree RSS, process-tree CPU, process count, binary size, and exact commands. OCI subjects add cgroup `memory.current`, `memory.peak`, and `cpu.stat` usage, plus image and in-image binary sizes. Each steady sample retains minimum, median, and maximum observations; the memory gate uses the maximum, never the median. Empty-process results are never subtracted.

## Run

Install the locked workspace dependencies before validation, and prepare the local OCI image without using this harness:

```bash
pnpm install --frozen-lockfile

SOURCE_COMMIT="$(git rev-parse HEAD)"
SOURCE_TREE="$(git rev-parse HEAD^{tree})"
CONTRACT_REF="$(git rev-parse HEAD:contracts)"
docker build \
  --build-arg FASTI_SOURCE_COMMIT="$SOURCE_COMMIT" \
  --build-arg FASTI_SOURCE_TREE="$SOURCE_TREE" \
  --build-arg FASTI_CONTRACT_REF="$CONTRACT_REF" \
  --tag fasti:b1 \
  .
```

Run those commands from a clean checkout. On the named Linux device, capture at least three repetitions. The hardware profile is derived from observed CPU and device-tree fields; there is no operator-supplied profile flag. Raspberry Pi 5 requires a device model containing `Raspberry Pi 5` and no hypervisor CPU flag. J4125 requires `J4125` in the CPU model, `systemd-detect-virt --vm` to report no VM, no hypervisor CPU flag, and non-virtual DMI vendor/product evidence. An unrecognized or virtualized fingerprint is refused rather than re-labeled.

```bash
python3 scripts/benchmark-b1.py capture \
  --image fasti:b1 \
  --runner-id pi5-lab-01 \
  --custodian "assigned person or team" \
  --repetitions 5 \
  --steady-window-seconds 5 \
  --sample-interval-ms 10 \
  --output benchmarks/b1/evidence/pi5-lab-01.json
```

The harness writes through a validated temporary file and refuses to overwrite an existing receipt. A budget failure still writes the valid measurement and exits nonzero, because a failing result is evidence rather than a capture error.

Validate a receipt independently:

```bash
node benchmarks/b1/validate-evidence.mjs benchmarks/b1/evidence/pi5-lab-01.json
```

Run the portable static and negative-sentinel tests on any development host:

```bash
node benchmarks/b1/validate-evidence.mjs --static
node benchmarks/b1/validate-evidence.mjs --self-test
python3 scripts/benchmark-b1.py self-test
python3 -m unittest benchmarks/b1/test_benchmark_b1.py
```

## Refusal conditions

No receipt is written when any required fact is unsupported or missing. Capture refuses:

- a non-Linux host, unavailable `/proc`, unavailable route-less user/network namespace, or unavailable cgroup v2;
- a dirty Git tree, absent local image, remote/non-Unix Docker endpoint, non-Docker cgroup-v2 engine, or existing output path;
- missing runner ID, physical custodian, fingerprint-derived hardware profile, exact `HEAD:contracts`, commit, tree, native digest, immutable image ID, or matching image source labels;
- a subject that exits early, fails readiness, gains an IP route or Docker network, produces an invalid guarded CLI result, or lacks a process-tree/cgroup sample;
- a source tree, `HEAD:contracts`, native binary, Docker context/socket, mutable image reference, immutable image ID, or image source label that changes before measurement finishes;
- a receipt whose summaries, budgets, digests, scenarios, network proof, or verdicts do not validate.

The script does not fabricate a partial JSON document for any of those cases.

## Device qualification ledger

[`device-hypotheses.json`](device-hypotheses.json) tracks the Raspberry Pi 5 champion, calibrated J4125 secondary, Ugoos AM6B+, Xiaomi Box M3, Nvidia Shield, and a representative TV profile. Each currently remains honestly `blocking_unassigned`: custodian, exact runner fingerprint, artifact and contract references, evidence reference, and results are `null` until a real custodian performs a validated run.

The v2 ledger can represent `assigned_pending_evidence`, `evidence_validated`, `qualified`, and `calibrated` without weakening that checked-in state. For any evidence-backed state, the validator loads the referenced receipt, verifies its SHA-256, validates the receipt, and correlates custodian, physical fingerprint, source artifacts, contract object, and derived budget results. `qualified` requires both applicable B1 budgets to pass. `calibrated` additionally requires the exact evidence reference owned by a qualified Raspberry Pi 5 entry, matching git tree, contracts, harness, budgets, workload/scenario settings, and measured J4125-to-champion idle and absolute-memory ratios.

J4125 evidence does not stand in for the champion result. Its calibration relationship to the Raspberry Pi 5 must be defined and measured before it becomes a calibrated secondary signal.

## Files

- [`evidence.schema.json`](evidence.schema.json): JSON Schema 2020-12 receipt shape.
- [`budgets.json`](budgets.json): canonical budget values.
- [`budgets.schema.json`](budgets.schema.json): locked budget contract.
- [`device-hypotheses.json`](device-hypotheses.json): explicitly unassigned hardware ledger.
- [`device-hypotheses.schema.json`](device-hypotheses.schema.json): assignable ledger and calibration-state contract.
- [`validate-evidence.mjs`](validate-evidence.mjs): JSON Schema and semantic validation, including negative sentinels.
- [`test_benchmark_b1.py`](test_benchmark_b1.py): portable capture trust-boundary tests.
- [`../../scripts/benchmark-b1.py`](../../scripts/benchmark-b1.py): Linux capture harness.
