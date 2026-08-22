# B1 native and OCI performance evidence

This suite captures the B1 headless baseline without turning a desktop sample into constrained-hardware proof. Capture is Linux-only and requires a clean Git tree, a prebuilt native `fastid`, a prebuilt local OCI image, route-less user/network namespaces, Docker network mode `none`, and cgroup v2.

The harness never pulls or builds an image. Every Docker invocation runs or inspects the local image named by the operator, and every measured container uses `--network none`.

## Canonical budgets

[`budgets.json`](budgets.json) is the single machine-readable owner of the memory budgets:

| Budget           |       Bytes | MiB | B1 disposition                                                                                 |
| ---------------- | ----------: | --: | ---------------------------------------------------------------------------------------------- |
| Idle target      |  67,108,864 |  64 | Evaluated against the worst native process-tree or OCI process-tree/cgroup steady idle maximum |
| Normal target    | 100,663,296 |  96 | `not_applicable`; B1 has no implemented normal-operation workload                              |
| Heavy target     | 167,772,160 | 160 | `not_applicable`; B1 has no implemented heavy-operation workload                               |
| Absolute ceiling | 201,326,592 | 192 | Evaluated against the worst Fasti daemon or guarded-CLI process-tree/cgroup peak               |

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

For native subjects the harness records startup, steady and peak process-tree RSS, process-tree CPU, process count, binary size, and exact commands. OCI subjects add cgroup `memory.current`, `memory.peak`, and `cpu.stat` usage, plus image and in-image binary sizes. Empty-process results are never subtracted.

## Run

Install the locked workspace dependencies before validation, and prepare the native binary and local OCI image without using this harness:

```bash
pnpm install --frozen-lockfile
cargo build --locked --release --bin fastid --bin fasti
```

On the named Linux device, bind the run to the current tracked contracts and capture at least three repetitions:

```bash
CONTRACT_REF="$(git rev-parse HEAD:contracts)"

python3 scripts/benchmark-b1.py capture \
  --native-binary target/release/fastid \
  --image fasti:b1 \
  --hardware-profile raspberry_pi_5_champion \
  --runner-id pi5-lab-01 \
  --custodian "assigned person or team" \
  --contract-ref "$CONTRACT_REF" \
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
```

## Refusal conditions

No receipt is written when any required fact is unsupported or missing. Capture refuses:

- a non-Linux host, unavailable `/proc`, unavailable route-less user/network namespace, or unavailable cgroup v2;
- a dirty Git tree, missing executable, absent local image, non-Docker cgroup-v2 engine, or existing output path;
- missing runner ID, physical custodian, hardware profile, immutable contract reference, commit, tree, artifact digest, or runner fingerprint;
- a subject that exits early, fails readiness, gains an IP route or Docker network, produces an invalid guarded CLI result, or lacks a process-tree/cgroup sample;
- a receipt whose summaries, budgets, digests, scenarios, network proof, or verdicts do not validate.

The script does not fabricate a partial JSON document for any of those cases.

## Device qualification ledger

[`device-hypotheses.json`](device-hypotheses.json) tracks the Raspberry Pi 5 champion, calibrated J4125 secondary, Ugoos AM6B+, Xiaomi Box M3, Nvidia Shield, and a representative TV profile. Each currently remains `blocking_unassigned`: custodian, exact runner fingerprint, artifact and contract references, evidence reference, and results are `null` until a real custodian performs a validated run.

J4125 evidence does not stand in for the champion result. Its calibration relationship to the Raspberry Pi 5 must be defined and measured before it becomes a calibrated secondary signal.

## Files

- [`evidence.schema.json`](evidence.schema.json): JSON Schema 2020-12 receipt shape.
- [`budgets.json`](budgets.json): canonical budget values.
- [`budgets.schema.json`](budgets.schema.json): locked budget contract.
- [`device-hypotheses.json`](device-hypotheses.json): explicitly unassigned hardware ledger.
- [`device-hypotheses.schema.json`](device-hypotheses.schema.json): ledger shape and blocking-state contract.
- [`validate-evidence.mjs`](validate-evidence.mjs): JSON Schema and semantic validation, including negative sentinels.
- [`../../scripts/benchmark-b1.py`](../../scripts/benchmark-b1.py): Linux capture harness.
