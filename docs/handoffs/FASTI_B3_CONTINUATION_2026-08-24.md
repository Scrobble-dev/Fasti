# Fasti B3 continuation handoff — 2026-08-24

**Status:** Dated snapshot. Records body status and the state of the B3 export slice.
**Audience:** A harness or engineer picking up B3 with no access to prior sessions.
**Branch at time of writing:** `dev` @ `4c59ead0`. Export work-in-progress on `b3/export` @ `c299cd87`.

> Read [`README.md`](README.md) in this directory for precedence. Live source and exact-head evidence outrank this file. Where this file and the repository disagree, the repository wins.

---

## 1. Where B0-B2 actually stand

Every claim below was checked against source on `dev`, not copied from a status table. A body is complete only when its declared evidence passes at exact head.

### B0 — complete

The truth reset holds. `scripts/check-repository-truth.sh` passes and still rejects documentation that names any crate from the deleted pre-reset set. The active crate set is `fasti-domain`, `fasti-application`, `fasti-contracts`, `fasti-api`, `fasti-store`, `fasti-cli`.

The deleted crate names are deliberately not repeated here. That guard treats them as forbidden product claims in active documentation, and it is correct to: those surfaces do not exist on `dev`. The forbidden list lives in the script itself.

Preserve the boundary and the regression checks. Nothing further is owed.

### B1 — software scope complete, milestone NOT closed

**What is true.** The contract spine, fixture path, generated artifacts and SDK checks pass. `cargo xtask contract verify --locked` is green and emits a receipt. Headless QA and developer-experience gates pass.

**What is still required.** Named physical **Raspberry Pi 5** and **J4125** RAM evidence. Hosted-runner results do not substitute for it.

**Consequence.** `docs/capability-ledger.md` states that until this evidence exists, B1 remains in progress and **B2 is not authorized**. Do not describe B1 as done because CI is green.

### B2 — implemented behind ports, not released, not activated

**What is true.** Local identity, observations, credentials, receipts, SQLite persistence, content-addressed evidence storage and review workflows exist in `crates/fasti-store` and are unit-tested.

**What is NOT true.** None of it is reachable from a shipping binary.

Three independent gates hold it there. All three were verified on `dev`:

| #   | Gate                      | Evidence                                                                                                                                                                                     |
| --- | ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Link gate** — strongest | `fasti-store` has **zero dependents**. `apps/fastid` depends on `fasti-api`, `axum`, `tokio`, `tracing`, `tracing-subscriber`, `anyhow` only. `crates/fasti-cli` does not link it either.    |
| 2   | **Type gate**             | Exactly **one** entry in `define_capabilities!` carries `Implemented`: `SystemHealth`. `is_production_executable()` keys off that, so every other capability is unreachable by construction. |
| 3   | **CLI gate**              | `export`, `restore`, `verify` are explicit nonzero stubs, asserted by `crates/fasti-cli/tests/unavailable_commands.rs`.                                                                      |

**Do not cross gate 1 casually.** Adding `fasti-store` to `apps/fastid` or `crates/fasti-cli` activates every staged capability at once. That is a governed decision, separate from adding any single capability.

**Still open for B2:** process crash and restart proof; commit-before-reply replay on the final public path; supported power-loss evidence; Pi 5 and J4125 ownership, fingerprint, native-memory and OCI-memory receipts; and public DTO plus transport activation as **one** coordinated change — not one surface at a time.

### B3 — partial

| Slice                            | State                                                         |
| -------------------------------- | ------------------------------------------------------------- |
| Correction persistence           | Landed behind ports (`crates/fasti-store/src/correction.rs`)  |
| Workspace integrity verification | Landed behind ports (`crates/fasti-store/src/portability.rs`) |
| **Export**                       | **In progress on `b3/export`, not merge-ready**               |
| Restore                          | Not started                                                   |
| Clean-equality proof             | Not started                                                   |

`cargo xtask test milestone --body B3` deliberately bails. That is correct and should stay until B3 is authorized.

---

## 2. B3 export — current state

Commit `c299cd87` on `b3/export`, branched from `dev` @ `4c59ead0`. Builds clean; 28 application tests pass.

### What exists

**Application boundary** — `crates/fasti-application/src/portability.rs`:

- `ExportWorkspaceQuery`, mirroring `VerifyWorkspaceQuery`;
- `WorkspaceExportEntity`, a 15-variant enum whose `ALL` order **is** the archive section order;
- `WorkspaceExportOutcome`, carrying per-entity counts, bytes written and the archive digest;
- `WorkspaceExportPort`, taking `&mut dyn std::io::Write`.

The sink type is deliberate. `std::io::Write` is a standard-library boundary, not an adapter type, so the adapter can stream bounded pages while the domain-inward dependency rule holds.

**Store adapter** — `crates/fasti-store/src/portability.rs`:

- JSONL v1 archive: header line, per-section marker, one line per row, counts trailer;
- every section ordered by its **full primary key**;
- per-page re-authorization, so revocation part-way through stops further disclosure;
- snapshot-by-recount fence matching `verify_workspace` — pages release the connection lock so acceptance is not blocked, and a closing recount that differs fails with `StorageUnavailable`.

**Capability** — `ExportWorkspace` already existed as `Reserved` / `Guarded` with the `workspace_export` scope. The only change is its **staged** problem list, `[]` to `[IntegrityFailed, StorageUnavailable]`. `FastiProblem::new` asserts the code is allowed, so the adapter would panic without it. The **public** problem list and `contracts/registry/v1/capabilities.yaml` are untouched: no contract drift, nothing to regenerate.

### Export policy as implemented

Included: `workspaces`, `profiles`, `clients`, `records`, `external_identifiers`, `evidence`, `observations`, `observation_clues`, `occurrences`, `interpretations`, `review_items`, `review_candidates`, `corrections`, `receipts`, `operations`.

Excluded, enforced by a unit test rather than convention: `credentials`, `profile_grants`, `grant_scopes`, `node_state`, `listener_configuration`.

### Three things a reviewer must decide

1. **Adapter tests are not written.** Including the cross-process determinism test. A same-process test would pass while two separate CLI runs diverge, because Rust's `HashMap` seed is per-process. This one has to be deliberate.
2. **Evidence blob content is not embedded in v1.** The header declares `evidence_content: excluded_v1`; only digest and size manifest rows are written. The handoff wording permits "a clearly versioned inclusion policy", but the consequence is that **restore cannot yet prove evidence equality**.
3. **`clients` is exported without `current_credential_epoch`.** Referential integrity needs the client rows, because observations and receipts carry client foreign keys. The epoch is the live fence, and exporting it would let a stale credential re-validate after a restore. That split is a judgement call.

---

## 3. Determinism rules for the archive

A byte-identical archive from identical durable state is a requirement, not a nicety, because the restore equality proof depends on it. Four traps, and how each is currently handled:

| Trap                                                      | Why a test would miss it                                                                              | Status                                                                            |
| --------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Multiple `SELECT`s without one read transaction           | Tests run single-threaded with no concurrent writer, so dangling references only appear in production | Handled by the snapshot-by-recount fence; fails closed                            |
| `HashMap` iteration order (per-process SipHash seed)      | Two exports **inside one test** share the seed and match; two CLI runs do not                         | Closed structurally: `serde_json` has no `preserve_order`, so `Map` is `BTreeMap` |
| `ORDER BY` without a unique tiebreak                      | Fixtures insert sequentially so rowid order accidentally matches; fragmented pages flip it            | Every section orders by its full primary key                                      |
| Archive container metadata (`mtime`, `uid`, `gid`, umask) | Single machine with consistent umask hides host divergence                                            | Not applicable — JSONL, no tar                                                    |

Two further properties worth preserving:

- The schema declares **zero REAL and BLOB columns**. `encode_row` fails closed on both, so the no-float-formatting assumption stays enforced rather than assumed.
- The header contains **no wall-clock time and no host identity**. Either would break byte equality between two exports of the same state.

---

## 4. Next exact actions

1. Write the store adapter tests: happy path; populated workspace; **cross-process** determinism; revocation mid-export; cross-workspace denial; fence fires on a concurrent insert.
2. Add a `grant_export_scope` helper mirroring `grant_verify_scope`.
3. Run the full gate: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets`, `cargo xtask contract verify --locked`.
4. Open a PR into `dev`. CI runs on PRs into `dev`; it does **not** run on PRs into any other long-lived branch.
5. Decide the three review questions in section 2.
6. Then restore, then the clean-equality proof.

Do not activate B3 public contracts, and do not flip the CLI guards, as part of this work. Activation is one coordinated change covering DTOs, OpenAPI, AsyncAPI, JSON Schema, JSON-LD, SDK, CLI, examples, scopes, errors and conformance together.

---

## 5. Open decisions held by the maintainer

- **glib advisory** (`GHSA-wrw7-89jp-8q8g`). Removal was attempted and is not achievable: `tauri 2.11.5` is the latest release, `glib` is structural through `gtk 0.18` across the whole Tauri Linux stack, there is no `0.18.x` fix, and RustSec classifies it as **unsoundness, not a vulnerability**. `cargo audit` already scans that lockfile in `security.yml` and passes. Full evidence and four review triggers: [`../reviews/2026-08-24-dependency-advisory-disposition.md`](../reviews/2026-08-24-dependency-advisory-disposition.md). Dismissal is a maintainer call.
- **Identity UAT phase mapping.** The 126-case identity matrix uses `M0`-`M6`; `uat-ownership.v1.json` uses `B0`-`B8`. The mapping is unmade, and the matrix currently gates shape and completeness rather than a body. Recorded in `tests/conformance/README.md`.

---

## 6. What has not changed

- The product boundary. **Fasti records. Players play.**
- The evidence rule. A body is complete only when its declared evidence passes at exact head.
- Milestone status. No body was advanced by this session.
- `release` still holds the pre-truth-reset scaffold, and heals when #20 lands.
