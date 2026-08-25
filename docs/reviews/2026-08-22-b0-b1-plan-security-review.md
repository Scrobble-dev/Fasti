# Fasti B0/B1 Plan and Security Review — 2026-08-22

## Review identity

- Repository: `Scrobble-dev/Fasti`
- Source branch: `vscode/fasti-b0-truth-reset`
- Review branch: `security/b1-evidence-hardening-20260822`
- Reviewed source head: `da66b94b20545df6be66e107d798f898045d017b`
- Integration base: `release`
- Earlier finalization receipt: `8a6c3bf0557938cfcd1248b7839bdbce1771a3b1`
- Pull request: `#14`
- Review type: branch diff, trust-boundary review, plan audit, developer-experience audit, and QA planning

The source branch is a headless engineering baseline. It is not a supported release. B1 software gates have prior evidence. B1 remains open until the named physical Raspberry Pi 5 and J4125 receipts exist. The owner has authorized B2 code development on the draft pull-request branch. That authorization does not close B1, waive physical evidence, authorize merge, or make B2 complete.

## Controlling inputs

The review used these accepted inputs:

- `winks-HEAD-engineering-plan-20260821-201100(1).md`
- `winks-HEAD-design-20260821-182751(1).md`
- `winks-HEAD-eng-review-test-plan-20260821-195019(1).md`
- `tasks-ceo-review-20260821-230057-final(1).jsonl`
- `winks-vscode-fasti-b0-truth-reset-design-audit-20260822-030500(1).md`
- `tasks-autoplan-devex-review-20260821-230057(1).jsonl`
- the current branch and its contract, security, benchmark, workflow, packaging, and contributor surfaces

The review also used restraint and review patterns from [Ponytail](https://github.com/DietrichGebert/ponytail) and the plan, developer-experience, and QA workflows from [gstack](https://github.com/garrytan/gstack). Those sources informed review method. They do not replace Fasti's accepted plan.

## Result

No critical or high-severity reachable vulnerability was found in the reviewed production or conformance HTTP path.

The review confirmed four defects and one repository-guidance gap:

| ID | Severity | Finding | Resolution |
| --- | --- | --- | --- |
| FST-REV-001 | Medium | Third-party workflow actions used mutable tags. A moved tag could change the code that evaluates build or security status. | Pin every external action to a 40-character commit and enforce that rule in repository truth checks. |
| FST-REV-002 | Low | The private runner snapshot checked file type before copy but did not prove the source stayed unchanged during copy. | Compare source identity and metadata before and after copy, verify copied byte count, delete the snapshot on mismatch, and add a race regression test. |
| FST-REV-003 | Low | The normal OCI build used floating base tags while the evidence build used immutable digests. | Remove the remote Dockerfile frontend declaration and pin both normal base images to the same multi-architecture digests used by the evidence image. |
| FST-REV-004 | Medium | Contributor and constitutional copy named abstract architecture labels that the accepted boundary requires as behavior, not slogans. | Replace the labels with concrete ownership and dependency rules across README, constitution, completion criteria, contribution guide, and pull-request template. |
| FST-REV-005 | Medium | `AGENTS.md` routed skills but did not encode the current phase, product boundary, contract duties, offline rules, evidence limits, security rules, accessibility duties, or relationship requirements. | Replace it with repository-specific execution rules. |

## Threat model

### Assets

- stable local media identity and history;
- original observations and retained evidence;
- profile, client, credential, grant, and receipt integrity;
- generated contracts and capability meaning;
- benchmark and hardware qualification evidence;
- private runner bundles;
- CI, package, and release trust.

### Threat actors

- an unauthenticated network client;
- a malicious local process or user;
- hostile provider or fixture data;
- a malicious or compromised contribution;
- a compromised action, dependency, base image, or build input;
- an operator mistake or concurrent file mutation.

### Trust boundaries

- the production loopback listener;
- the feature-gated loopback conformance server;
- HTTP, SSE, SDK, and CLI input into application policy;
- future application-to-store and application-to-filesystem ports;
- source checkout and CI into generated artifacts;
- runner input files into detached exact-commit bundles;
- native, OCI, and later packaged distribution.

### Required invariants

- Missing durability cannot return success.
- Production routes cannot appear before their authorization, persistence, replay, failure, and recovery behavior exists.
- Each rule and public meaning has one authoritative owner.
- Provider data is evidence and cannot become canonical identity.
- Network-denied local operation works or returns a local typed recovery path.
- Secrets do not enter URLs, arguments, logs, screenshots, fixtures, or proof bundles.
- Hardware and release claims remain blocked until their exact receipts pass.

## Reviewed security paths

### Production daemon

`fastid` exposes the health route only. The event-submission route remains absent. The default listener is loopback. The OCI wrapper uses an explicit container listener and requires deliberate host publication.

### B1 conformance fixture

The fixture is compile-time gated, IPv4-loopback only, bounded, in-memory, and nondurable. Credential comparison is constant-time. Bootstrap state is consumed only on success. Authorization is checked while the fixture state lock remains held. Request bodies, operation count, replay, and SSE state are bounded.

No reachable authentication bypass was found in this path.

### TypeScript transport

The SDK restricts base URLs, bounds retries and response sizes, does not retry non-idempotent enrollment or administration calls, and bounds SSE parsing and cursor state. No reportable vulnerability was confirmed.

### Runner handoff

The handoff is exact-commit bound, rejects extra references and prerequisite-dependent bundles, verifies in an empty object database, refuses unsafe links and overwrite destinations, and makes no unsupported authenticity claim. The source-mutation fix closes the remaining local race in the snapshot step.

## Contract disposition

The B1 remediation changes no runtime capability, public payload, route, event, problem, permission, or SDK behavior.

| Surface | Disposition |
| --- | --- |
| Production OpenAPI | N/A — no production handler change |
| Conformance OpenAPI | N/A — no fixture handler or DTO change |
| AsyncAPI | N/A — no stream contract change |
| JSON Schema | N/A — no public payload change |
| JSON-LD and OKF | N/A — no vocabulary or operational meaning change |
| CLI and SDK | N/A — no command or transport behavior change |
| Documentation and agent guidance | Updated |
| OCI and CI trust inputs | Updated |
| Runner evidence tooling | Updated with regression coverage |

Contract verification still must run because repository truth and generated-surface drift are release gates.

## Offline, packaging, and performance

The remediation adds no network requirement and no hosted service. It preserves native daemon and CLI primacy. OCI remains a wrapper around the same binaries. Later package formats remain outside the B1 remediation.

The OCI digest change improves reproducibility and does not alter runtime behavior. The runner check uses constant-size metadata and adds no material memory cost. Existing memory targets remain unchanged:

- 64 MiB idle;
- 96 MiB normal operation;
- 160 MiB heavy operation;
- 192 MiB absolute process-tree ceiling.

These are not new hardware results. Physical Raspberry Pi 5 and J4125 evidence is still missing.

## Design, accessibility, and attention continuity

This change has no product UI. Product screenshots, browser interaction evidence, and rendered interface review are not applicable. They are not fabricated.

The contributor surfaces use stable headings, short sentences, concrete actions, explicit status, and persistent next steps. Future rendered changes remain subject to grouping, visible system state, predictable navigation, keyboard and screen-reader use, target size, contrast, reduced motion, focus return, error recovery, and ADHD/AuDHD state-continuity gates.

## QA plan

Run from a clean checkout after applying the remediation:

```bash
python3 -m unittest benchmarks/b1/test_runner_bundle.py
bash scripts/check-repository-truth.sh
bash scripts/check-no-publish.sh
bash scripts/test-no-publish-policy.sh
node scripts/check-doc-links.mjs

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask contract verify --locked
cargo xtask test pr

pnpm install --frozen-lockfile
pnpm format:check
pnpm typecheck
pnpm test

docker build --tag fasti:b0 .
bash scripts/smoke-oci.sh fasti:b0
```

The prior `8a6c3bf` receipts do not prove this remediation. Attach new outputs to the final commit.

## Relationships

- Existing Fasti issues at review time: none.
- Existing pull request for the source branch at review time: none found.
- Active review pull request: `#14`.
- Related accepted planning artifacts: listed above.
- Relevant method references:
  - [Ponytail](https://github.com/DietrichGebert/ponytail)
  - [gstack](https://github.com/garrytan/gstack)
- No new issue is required only to populate a relationship field.

## Postmortem

### What happened

The branch correctly concentrated on truthful behavior and strong evidence. Later evidence and CI work expanded faster than the repository-wide contributor and supply-chain controls. This left mutable action tags, a small snapshot race, inconsistent OCI pinning, and generic agent guidance.

### Why prior QA did not catch it

The earlier mandatory QA at `8a6c3bf` preceded four later commits. Those receipts could not validate later files. Existing workflow policy checked publishing authority, but it did not check immutable action revisions. Runner tests covered links, extra references, prerequisites, ownership, and caller mutation after snapshot creation, but they did not mutate the open source file during the copy.

### Corrective action

- Make immutable workflow revisions an executable repository truth rule.
- Make snapshot stability an executable runner invariant.
- Use one pinned base-image set for normal and evidence OCI paths.
- Put phase, boundary, contract, offline, performance, security, accessibility, and traceability rules in `AGENTS.md`.
- Describe architecture through enforceable ownership and dependency statements.

### Prevention

Every later commit after a milestone receipt requires a new diff-aware review. A prior receipt can remain historical evidence, but it cannot be treated as proof for a changed head.
