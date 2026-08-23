# B2 continuation security and QA review — 2026-08-23

## Review identity

- Repository: `Scrobble-dev/Fasti`
- Pull request: [#14](https://github.com/Scrobble-dev/Fasti/pull/14)
- Base: `release` at `8fc8173d50238dbc97718cf8dfd5b46402efc63e`
- Reviewed runtime head: `eb4f8990a1c010a0bf263467c9c81a16934407e0`
- Documentation commit: the commit that contains this file
- Review scope: B0/B1 foundation, B2 local-kernel implementation, latest contract-policy repair, exact-head CI, security audit, offline and package posture

This is a draft implementation review. It does not activate B2 in the production daemon, close the B1 hardware gate, authorize merge, or claim a supported release.

## Result

No critical or high-severity reachable vulnerability was confirmed in the reviewed production surface or latest repair. The production daemon remains health-only. The B2 kernel is present behind application ports and storage adapters and is exercised by tests, but `fastid` does not construct or expose it.

The continuation found and repaired one contract-policy defect and one source-transfer incident:

| ID | Severity | Finding | Resolution |
| --- | --- | --- | --- |
| FST-B2-001 | Medium | The B2 store correctly returned `bootstrap_closed` and `authentication_failed`, but the capability table permitted only finalized B1 public problems. `FastiProblem` therefore panicked instead of returning the safe failure. | Keep finalized public problems and staged B2 runtime problems as separate sets. Runtime validation accepts both; public iteration exposes only finalized problems. Add regressions for enrollment, ambiguous grants, ungranted profiles, and cross-workspace grants. |
| FST-B2-002 | Process | A formatting-only repository write malformed the macro matcher and two type names. | Restore the valid source in a signed-off recovery commit, keep the intended formatter layout, and rerun all exact-head gates. |

## Security properties reviewed

### Access and bootstrap

- Empty-node initialization is transactional.
- The enrollment proof has a bounded lifetime, is consumed once, and is cleared after success.
- Credential authentication requires an explicit profile and joins the credential, client, grant, profile, and workspace.
- Multiple active grants do not select an arbitrary profile.
- A grant from another workspace cannot be inherited through a matching client identifier.
- Invalid epochs, revoked state, missing grants, and malformed stored IDs fail closed.

### Storage and evidence

- SQLite settings are established and read back: foreign keys, WAL, `synchronous=FULL`, schema version, and bounded busy timeout.
- Existing data paths reject symbolic links and non-regular database files. Owner-only permissions are applied where supported.
- Evidence admission authorizes before temporary-file creation and reserves bounded concurrent/byte capacity.
- Streaming enforces declared and observed size, updates the digest incrementally, flushes and syncs before promotion, and rechecks access before durable use.
- Content-addressed collisions are accepted only after digest and size verification.
- Failed or dropped sessions remove their temporary file and release capacity.

### Receipts and identity

- Operations bind workspace, client, operation identifier, capability, semantic digest, and receipt.
- Same operation and same meaning replay the committed receipt; changed meaning conflicts.
- Receipts and cursors are scoped by workspace, profile, and client.
- Provider identifiers are evidence attached to provider-neutral records. An identifier conflict does not merge records silently.
- Review state and interpretations remain separate from the original observation.

## Attack-path review

The latest defect was reachable through local-kernel tests, not through the production health-only listener. The relevant path was:

```text
expired or consumed proof / ambiguous or foreign grant
    -> SQLite access adapter returns a fail-closed B2 problem
    -> FastiProblem validates the code against the capability policy
    -> policy omitted the staged B2 code
    -> panic instead of typed safe failure
```

The repair changes the final step:

```text
internal B2 failure
    -> staged runtime policy permits the code
    -> typed safe failure is returned
    -> public registry still iterates finalized B1 codes only
```

This does not add authority, weaken profile checks, expose receipt data, or activate a route. It removes a denial-of-service failure in the review kernel while keeping generated public contracts stable.

## Contract disposition

The latest repair changes no public capability meaning.

| Surface | Disposition |
| --- | --- |
| Production OpenAPI | No change; production remains health-only |
| B1 conformance OpenAPI | No semantic change |
| AsyncAPI | No channel or message change |
| JSON Schema | No payload change |
| JSON-LD and OKF | No vocabulary or example change |
| CLI and TypeScript SDK | No command, method, retry, or error-shape change |
| Internal application policy | Staged B2 failures are now permitted without entering public output |

Exact-head Governed Contract Conformance passed after the repair.

## QA evidence

The reviewed runtime head passed:

- [CI run 90](https://github.com/Scrobble-dev/Fasti/actions/runs/32629274676): repository truth, no-publish policy, documentation links, Rust metadata/format/lint/tests/build, retained JavaScript formatting/types/tests, exact source snapshot, OCI build, non-root/health/false-success/memory smoke, and the canonical PR gate.
- [Governed Contract Conformance run 90](https://github.com/Scrobble-dev/Fasti/actions/runs/32629274663): OpenAPI, AsyncAPI, JSON Schema, JSON-LD, OKF, examples, and SDK parity.
- [Security Audit run 91](https://github.com/Scrobble-dev/Fasti/actions/runs/32629274707): locked dependency advisory audit.

A prior B1 receipt applies only to its recorded commit. These runs are the evidence for the reviewed runtime head. The documentation commit must also pass its exact-head checks before the PR is represented as green.

## Offline, package, and performance disposition

The repair adds no network call, service, process, queue, provider lookup, or unbounded collection. Native Rust remains the primary runtime. OCI contains the same daemon and CLI and passed its smoke gate. The B2 store uses local SQLite and the local filesystem and has no required cloud, Redis, broker, provider, or external database.

The change adds only fixed-size policy slices and constant-time membership checks. It does not affect request I/O or storage complexity. Physical Raspberry Pi 5 and J4125 native/OCI measurements remain open and cannot be replaced by hosted-runner results.

## Design and accessibility disposition

This change has no product UI. Product screenshots, browser interaction evidence, and visual acceptance are not applicable and were not fabricated. Contributor documentation uses stable headings, explicit state, persistent next actions, and outcome-first wording. Future UI remains subject to keyboard, screen-reader, touch, remote, target-size, contrast, reduced-motion, focus-return, recovery, and interruption-continuity gates.

## Relationships

- Accepted engineering plan: `winks-HEAD-engineering-plan-20260821-201100(1).md`
- Accepted design: `winks-HEAD-design-20260821-182751(1).md`
- Accepted test plan: `winks-HEAD-eng-review-test-plan-20260821-195019(1).md`
- Prior B1 QA receipt: `winks-vscode-fasti-b0-truth-reset-test-outcome-20260822-063900.md`
- Floppy/Nuvio lineage: [Floppy PR #791](https://github.com/dannyvfilms/Floppy/pull/791), [Floppy issue #532](https://github.com/dannyvfilms/Floppy/issues/532), [Floppy issue #636](https://github.com/dannyvfilms/Floppy/issues/636), and [NuvioTV issue #2935](https://github.com/NuvioMedia/NuvioTV/issues/2935)
- Upstream lineage: [FuzzyGrim/Yamtrack](https://github.com/FuzzyGrim/Yamtrack)
- Review-method references: [Ponytail](https://github.com/DietrichGebert/ponytail) and [gstack](https://github.com/garrytan/gstack)

The Fasti repository has no issue that owns this branch. No issue was created only to populate a relationship field. These cross-project links are context and future compatibility evidence; this PR does not claim to close them.

## Rollback

Revert the staged problem-policy commit and its recovery only as one unit. A partial rollback would restore either the panic or invalid source. Do not add B2-reserved failures to the B1 generated registry as a shortcut. If the B2 implementation is removed, revert the B2 code and its migrations before changing the public B1 contract.

## Postmortem

### What happened

B2 introduced legitimate internal failure modes before their public B2 contract surfaces were activated. The capability table had one problem list and treated it as both public output and runtime validation. This made the safer storage behavior fail a defensive assertion. A later formatting-only repository write then damaged source text while trying to repair layout.

### Why the earlier gates did not prevent it

The original B1 tests did not execute the SQLite B2 paths. Later B2 tests did, but the first exact-head run stopped on the policy mismatch. The malformed formatting commit was created after that run and therefore needed a new exact-head cycle.

### Corrective actions

- Keep public and staged runtime problem sets explicit.
- Test both properties: staged failures are accepted internally and excluded from public output.
- Treat every repository write as source-changing, including formatting-only writes.
- Require a fresh exact-head CI, contract, and security result after each write.
- Keep the PR draft while physical and later-body gates remain open.

## Remaining gates

- Physical Raspberry Pi 5 and J4125 ownership, fingerprint, native-memory, and OCI-memory receipts.
- B2 public DTO, OpenAPI/AsyncAPI/Schema/JSON-LD/SDK/CLI activation in one governed change when authorized.
- Process-crash, restart, supported physical power-cut, and constrained-hardware B2 milestone evidence.
- B3 correction, export, restore, and equality proof.
- Product UI and packaged application work in their later approved bodies.
