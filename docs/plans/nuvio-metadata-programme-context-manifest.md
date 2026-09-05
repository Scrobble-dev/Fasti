# Fasti Metadata and Nuvio Programme Context Manifest

**Recorded:** 2026-08-30

**Programme:** Metadata, Search, Discover, Library, Collections, and Nuvio compatibility

**Repository:** `Scrobble-dev/Fasti`

**Target:** `dev`

**M0 branch:** `codex/nuvio-metadata-programme-m0`

**Exact base:** `adbdef3038786b0efb2ec615bce080e3eaa9361f`

**Base tree:** `a7a1f661ae1b0ef4470ba736d65942f54793d1b0`
**Disposition:** M0 approved; no production behavior changes in this slice

## Controlling artifacts

| Artifact                                                       | SHA-256                                                            | Purpose                                                                                 |
| -------------------------------------------------------------- | ------------------------------------------------------------------ | --------------------------------------------------------------------------------------- |
| `docs/plans/nuvio-metadata-discovery-library-collections.md`   | `3d4702a003e7f098d21c821b70628dc56b6a3d1edb53543949cd99478ad55820` | Canonical scope, architecture, security, delivery, reviews, and Definition of Done.     |
| `docs/designs/nuvio-metadata-discovery-library-collections.md` | `3c3bcafc9bcec8c97c388530852cecc7665d036841007f158364d93a9954a994` | Approved product and ownership design.                                                  |
| `docs/designs/nuvio-metadata-flow-wireframe.html`              | `26b1454b291e844589a2c5bf2e3222d105deba1b486c1c6cbcbcb777e1c3ff68` | Search-to-record, Nuvio, and local-workspace state reference.                           |
| `docs/designs/nuvio-metadata-route-reference.html`             | `022f202a3cbbe9d780b142d872e1a612d2791a3488a7a1e18dee16ad8bf26992` | Tabler Workbench route and state reference.                                             |
| `contracts/registry/v1/nuvio-metadata-programme-preview.yaml`  | `cabd4dfd4a3772f50e647d76871d583d3bcf5b2b21d1ba6575e40ac234e4c71e` | Planning-only reservation of 51 capability owners, eight events, and 35 typed problems. |
| `tests/conformance/uat-matrix.csv`                             | `a8921831a3254c2246d8526538e308f9e7f4f805a086c06cc42e78535bdd017b` | Existing 80 identity cases plus 60 programme acceptance cases.                          |

Hashes bind this M0 review only. Any content change requires regenerating this table and rerunning the M0 checks.

## Source ledger

| Source                     | Revision or evidence                                                                               | Use                                                                                                                       | Constraint                                                                                  |
| -------------------------- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Fasti                      | `adbdef3038786b0efb2ec615bce080e3eaa9361f`                                                         | Live domain, application, store, API, Tauri, web, contracts, tests, and security truth.                                   | Exact Git objects outrank historical checkpoints.                                           |
| Authentication programme   | PR #93 merged at the planning base; canonical `docs/plans/trailbase-authentication-remediation.md` | Sessions, recent authentication, CSRF/Origin/Host policy, OAuth/device clients, vault, backup crypto, restore generation. | This programme consumes the boundary and does not edit auth-owned files.                    |
| Nuvio Desktop              | `ab498c9378aebf1a81cff104b3069eb6ac7701dc`                                                         | TMDB enrichment, MDBList separation, anime projection, Collections, native HTTP and synchronization behavior.             | GPLv3 source is evidence; Fasti does not copy implementation.                               |
| NuvioTV                    | `7c3baa16e491aeec5ee017dd867a271568ecfba3`                                                         | Typed Collection storage, catalog/meta URL consumption, upstream client target.                                           | GPLv3 source is evidence; upstream delivery remains a separate repository.                  |
| Scrob                      | `1c4d775b70f489ca0531376b2c3de6a8c3de2a2b`                                                         | Real Nuvio Cloud endpoint, sign-in, profile, refresh-token, snapshot, delta, and write benchmark.                         | Benchmark behavior does not create a stability promise for Nuvio RPC.                       |
| Kaptain Collection         | `fdb7a91e545f18f8a67aab49d4742b217fc02e2c` plus issue #9                                           | Lossless format compatibility and Trakt-dependency regression.                                                            | User-supplied or synthetic fixtures only; no unlicensed content enters Git or CI artifacts. |
| Stremio add-on protocol    | `2728da3ee853207cd5ee200aabe15a08cc1d01d1`                                                         | Public manifest, catalog, and metadata transport.                                                                         | Read-only public projection; no sensitive write synchronization or URL secret.              |
| TMDB and MDBList           | Primary documentation listed in canonical plan section 31                                          | Current authentication, search, enrichment, rating, catalog, and account capability semantics.                            | Terms, redistribution, origin, rate, and credential policy gate activation.                 |
| Historical Fasti artifacts | `ryan-winkler/gstack-artifacts-winks`                                                              | Decision provenance and prior evidence locations.                                                                         | Historical context is never current implementation proof.                                   |

## Current capability truth

| Area                             | Exact-base state                                                                                                                                                     | First owning slice                                              |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| Provider runtime                 | Real TMDB and Google Books behavior exists only in the trusted Tauri host; browser paths include incomplete surfaces.                                                | M1 extracts one concrete shared runtime for Tauri and `fastid`. |
| Credential status                | Protected writes and platform credential storage exist; capability-specific registry and full browser-safe composition do not.                                       | M1.                                                             |
| Metadata                         | Domain/application/store foundations exist; field lifecycle, profile projection policy, provenance UI, and archive v3 do not.                                        | M2.                                                             |
| Identity routing                 | Stable Records and typed evidence exist; operation-purpose routing and anime export policy are incomplete.                                                           | M3.                                                             |
| Search                           | Existing provider/local fragments exist; source-neutral local-first orchestration, durable candidate receipts, and canonical candidate routes do not.                | M4.                                                             |
| Library                          | Existing browser view materializes at most 500 Records; independent profile state and bounded keyset queries are incomplete.                                         | M5.                                                             |
| Discover                         | Existing UI and provider fragments exist; governed rails, persistence, cache policy, and stale-state truth are incomplete.                                           | M6.                                                             |
| Collections                      | Profile Nuvio JSON import/export/replace/clear exists; source-neutral entities, packs, previews, receipts, rollback, and safe URL activation do not.                 | M7.                                                             |
| TMDB                             | Search and selected enrichment exist in Tauri; required field groups, locale/region, episode/company/network routes, shared runtime, and attribution are incomplete. | M8.                                                             |
| MDBList                          | No complete capability-separated production adapter exists.                                                                                                          | M9a public reads, M9b private account state.                    |
| Public Nuvio/Stremio             | No approved publication descriptor or public field allowlist exists.                                                                                                 | M10.                                                            |
| Nuvio Cloud                      | No production connection exists; current generic synchronization models are process-local/conformance evidence.                                                      | M11a-c.                                                         |
| Fasti provider for Nuvio clients | No versioned production server or native client integration exists.                                                                                                  | M11d-g.                                                         |
| Local Shared Media Workspace     | No active sharing surface exists.                                                                                                                                    | M13a-e after Access and crypto gates.                           |

## Ownership map

| Bounded context | Owns                                                                                                              | Must not own                                        |
| --------------- | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| Identity        | Stable Record ID, typed external evidence, purpose route assertions, ambiguity review.                            | Metadata precedence, Library membership, playback.  |
| Metadata        | Field claims, provenance, freshness, projection policy, user overrides.                                           | Record identity or account synchronization.         |
| Search          | Query validation, source orchestration, candidates, candidate receipts.                                           | Implicit Record creation or Library mutation.       |
| Discover        | Governed catalog rails, source status, cache and stale presentation.                                              | Search query semantics or user-owned Library state. |
| Library         | Independent profile saved/progress/watched/rating/note state and bounded queries.                                 | Provider identity or Collection organization.       |
| Collections     | Documents, folders, memberships, source bindings, packs, previews, receipts, rollback.                            | Tracking semantics or provider account grants.      |
| Connections     | Credential references, capability/direction grants, health, reauthorization.                                      | Domain merge policy.                                |
| Synchronization | Journals, attempts, receipts, acknowledgements, cursors, tombstones, leases, reconciliation.                      | Playback or provider-specific domain decisions.     |
| Access          | Sessions, recent authentication, clients, scopes, grants, CSRF/Origin/Host, backup crypto and restore generation. | Metadata or synchronization business rules.         |

## Contract disposition

- `contracts/registry/v1/nuvio-metadata-programme-preview.yaml` is planning data and is not imported by the runtime registry or generators.
- Each owning milestone promotes only implemented capabilities into `contracts/registry/v1/capabilities.yaml` with application ownership, OpenAPI, applicable AsyncAPI, JSON Schema, JSON-LD or reasoned `N/A`, generated SDK, CLI discovery, scopes, typed problems, examples, knowledge, and UAT evidence.
- The focused `cargo xtask integration check` path is implemented in M1 using existing xtask, contract types, loopback fixtures, and `--output human|json` conventions.
- Nuvio Desktop and NuvioTV use their installed native HTTP/serialization stacks against the dual-licensed plain contract and fixtures. No Kotlin SDK or copied Fasti implementation is introduced.
- Public Stremio transport covers only explicitly allowlisted catalog and metadata reads. Nuvio Cloud and authenticated Fasti-provider lanes own writes.

## Dependency and delivery graph

```text
M0
 -> M1 -> M2 -> M3 -> M4 -> M5 -> M6 -> M7
                      |      |      |      |
                      |      |      |      +-> M10
                      |      |      +--------> M8
                      |      +---------------> M9a
                      +-----------------------> M11a

M9a + M11a -> M9b
M11a -> M11b -> M11c
M11a -> M11d -> M11e + M11f -> M11g
Access + C3-CRYPTO + M11d -> M13a -> M13b -> M13c -> M13d -> M13e
M8 + M9 + M10 + M11 + M13 -> M12 integrated gate
```

Every Fasti slice targets `dev` from its accepted predecessor. One writer owns each migration number, authored registry set, generated contract set, SDK surface, and Workbench composition slice. Nuvio client work uses pinned upstream repository branches and cannot claim release until accepted in a published upstream build.

## Threat and evidence map

The controlling threat model is canonical plan sections 23, 39, and 54. Mandatory negative evidence includes credential non-disclosure, governed egress, cross-profile denial, candidate re-fetch, inert imported URLs, bounded parsing, private/public cache separation, token-rotation fault injection, complete idempotency authority scope, storage admission quotas, stream invalidation, and restore-generation quarantine.

The programme acceptance map is `tests/conformance/uat-matrix.csv` rows `MDN-001` through `MDN-060`. Deterministic fixtures prove semantics. Redacted live acceptance separately proves TMDB, MDBList, Nuvio Cloud, Nuvio Desktop, and NuvioTV transport on exact revisions. Neither class substitutes for the other.

## M0 verification

```text
git rev-parse HEAD HEAD^{tree}
git diff --check
bun YAML parse + uniqueness/count check: 51 capabilities, 35 problems, 8 events
bun CSV uniqueness/count check: 140 total UAT rows, 60 MDN programme rows
cargo xtask contract verify --locked
```

M0 changes planning, design, preview-contract, and acceptance-evidence files only. It does not activate a capability, change a schema, alter generated contracts, or modify production behavior.

## M3 merged checkpoint — 2026-09-01

| Evidence | Exact value |
| --- | --- |
| Pull request | `Scrobble-dev/Fasti#120` |
| Reviewed PR head | `1c8fb5db15036d6d13b703adda116f01b08baf83` |
| Reviewed PR tree | `5552947a30b82497c7fa279a6932fe7877ed612b` |
| Merged `origin/dev` | `df09101028a988a92f4546313c5eed6dd20d238a` |
| Merged tree | `5552947a30b82497c7fa279a6932fe7877ed612b` |
| Merged at | `2026-09-01T02:29:37Z` |
| Delivery evidence | 22 exact-head checks passed; zero unresolved review threads; the merged tree is byte-identical to the reviewed PR tree. |

M3 publishes schema v15 and archive v5. Archive v4 remains frozen at its
29-stream prefix. Archive v5 retains that prefix and appends five M3 streams,
for 34 streams total. Historical restore must continue to accept archive v5
with migration version 15. M4 owns the single append-only schema v16 migration.
Archive v6 is not allocated; M4 may advance the archive only if frozen durable
Search state proves that portability requires it.

The merged M3 invariants are:

- stable Record identity and purpose-specific identity routes;
- profile defaults plus per-client anime grouping overrides without Record or Chronicle re-keying;
- workspace-wide operation identity with replay bound to actor client, profile, action scope, and semantic digest;
- bounded lifecycle loading that splits valid multi-Record batches and fails closed when one Record exceeds the per-Record limit;
- literal same-origin browser-smoke endpoints with the existing CSRF, method, body, and credential behavior;
- unchanged C1 TrailBase exchange, opaque Fasti browser-session, and zeroizing process-memory PKCE boundary;
- unchanged M2 provider-operation serialization and bounded outbound policy.

M4 is now the sole writer for schema v16, Search/provider candidate types,
identity-routing integration, atomic Record actions, capability registry and
generators, generated contracts, API, SDK, hosts, Workbench composition,
portability extensions, and the related governed documentation and tests.
M8 may prepare or edit only its isolated metadata-refresh leaf after explicit
file allocation; it must not touch shared provider, metadata, contract, host,
or UI composition files until M4 merges. M5 remains read-only preparation until
M4 and the minimum Collections membership predecessor merge.

Codex Security is intentionally disabled for this programme. Exact-diff review,
canonical CI, CodeQL, Codacy, CodeRabbit, advisory checks, browser QA, OCI,
coverage, documentation, contract parity, and both low-hardware envelopes are
the retained M3 evidence.

### M4 provider paging checkpoint — 2026-09-04

The shared provider runtime now accepts an explicit upstream page and TMDB
locale. It preserves each normalized page and response digest, validates the
returned TMDB page, and returns a bounded continuation. The existing desktop
entry point uses the same implementation. TMDB retains all 20 upstream results;
Google Books requests 10. Empty raw pages stop traversal; pages containing only
filtered candidates can continue. Search query debug output is redacted.

Primary sources inspected on 2026-09-04:
[TMDB multi-search](https://developer.themoviedb.org/reference/search-multi),
[TMDB page limits](https://developer.themoviedb.org/docs/errors), and
[Google Books volume search](https://developers.google.com/books/docs/v1/reference/volumes/list).
Provider runtime tests pass (27), strict clippy passes, and an independent
read-only review found no actionable defect (15 focused provider tests pass).
This evidence covers parsing, URL construction, bounds, and credential ordering;
it does not prove live upstream paging or completed M4 Search.

The next implementation work is durable, authorized candidate receipts and
atomic Record actions; selected-Record metadata loading; the local search index;
and API/SDK/host/UI integration. Stable public cursors must refer to persisted
ordering, not directly to changing upstream pages. Cross-page duplicate handling,
offline cache, partial results, real browser QA, and the 10,000-Record performance
gate remain required before M4 delivery. Schema remains v15 at this checkpoint;
M4 retains allocation v16 and the archive decision is still unallocated.

Parallel preparation confirmed reuse of the M2 projection resolver and provider
locks, M3 workspace-wide operation replay semantics, and the existing C1 browser
authentication boundary. SQLite already compiles FTS5; no dependency is needed.
C2 owns its isolated Access credential modules and additive Access exports in
domain/application lib.rs. M4 owns Search exports and all previously allocated
shared surfaces. Codex Security remains disabled.

### M4 query and selected metadata checkpoint — 2026-09-04

Provider paging is committed at `81b7cbea`; shared domain query admission is
committed at `b8249540`. `SearchQuery` owns the 256-byte UTF-8 bound, whitespace
and control-character checks, and redacted debug representation. Provider URL
construction consumes the validated value. No query rewrite discards upstream
search syntax.

Record metadata loading now accepts the caller's bounded, typed Record IDs,
instead of selecting the workspace's first 500 Records again. The current
Record-list caller uses this shared loader. It retains the M2 projection and
lifecycle resolver. The regression selects Record 501, tests duplicate input
IDs, excludes a populated foreign-workspace Record, preserves profile override
isolation, and checks empty and oversized batches. This prepares local Search
to resolve its actual matches without a second metadata implementation.

Verification: domain 88 tests pass; provider runtime 27 tests pass; store 292
tests pass with three subprocess-worker tests ignored by design. Strict clippy
passes for all targets in these three crates. Store snapshot tests require
`TMPDIR=/mnt/secondary-ssd/cache/home/tmp` on this host: the default temporary
path traverses a symlink and correctly fails SQLite's no-follow check. The
physical-path rerun passes without changing protection or test assertions.
Independent review of the selected-ID loader found no actionable issue.

No M4 runtime Search endpoint, durable candidate cache, action receipt, local
index, migration, archive format, or browser flow is activated by these commits.
Those implementation and verification requirements remain in the active M4
workfront above. The full programme goal remains active.

### M4 evidence admission and shared hardening checkpoint — 2026-09-04

Codex Security remains intentionally skipped. Native independent review and
negative tests remain active; no external AI key was used. The commander keeps
sole write ownership of M4 shared surfaces. Parallel agents reviewed evidence
admission, optimized secret erasure, typed-ID parsing, and the v15 archive
compatibility prerequisite. The next bounded read-only review maps Search
partition inputs to existing authorization and provider-configuration owners.

Three isolated shared-owner commits were handed to C2 for cherry-pick:

- `f46e5de8c94d6e31d43cc0e0a78f61ea7a501d08`: frozen executable PAT resource
  `pat_` and immutable consent revision `cnr_` identifiers. Neither is a bearer
  secret. No Search candidate identifier is included in that handoff.
- `fee9059d0a59d8b26bacbfbd8568636f4ec92ff3`: reuse existing `zeroize` for
  `SecretMaterial::drop` and enable the installed SHA-256 zeroizing feature.
  A compile-time regression requires `Sha256: ZeroizeOnDrop`. Independent
  optimized compilation of the actual owner retained 32 volatile zero stores.
  This does not prove erasure of prior copies or exposed hexadecimal strings.
- `8657a9124609ffe767f2131399714f6e944aa357`: fix the pre-existing concrete-ID
  JSON deserialization bypass by routing through the existing strict `FromStr`.
  The reproduced bug accepted a Record or consent ID as a PAT ID. The generated
  test matrix now checks every concrete type against every registered prefix.
  This proves a validation defect and fix, not an authorization exploit.

The M4 application evidence owner now validates normalized candidate fields,
canonical provider coordinates, bounded JSON and image origins. Provider
normalization reuses the text/image predicates. Invalid optional artwork is
discarded without losing the candidate; persisted unsafe artwork is rejected.
Candidate identity uses a reserved `scr_` typed ID, not an invented Record ID.

Receipt partitions bind workspace, profile, stable actor, grant, query digest,
grant digest, provider-configuration digest and terms. Browser-session rotation
alone does not change the stable actor. Current authorization and configuration
must be recomputed by the service before comparing a persisted partition.
The caller's complete digest inputs are not yet implemented or verified.
Lifetime tests enforce 120-second freshness, 600-second stale-on-error without
stale-while-revalidate, and the independent 24-hour candidate-details expiry.
Clock rollback before creation and timestamp overflow fail closed.

Verification: 537 tests passed across domain (90), application (127), provider
runtime (28) and store (292), with three intentional store subprocess-worker
tests ignored. Strict all-target clippy passed for these four crates. The
additional published-v15 fingerprint regression passed independently and pins
`sha256:36720ca62ef606e52f960e71cb40452323269f14e4a4af984e2fe875279a155e`.
It explicitly runs the historical v1–v15 migration chain, not the current-version
dispatcher. All published migration functions remain unchanged.

Schema is still v15; M4 retains v16. No new archive version or shared-file
release has occurred. Before changing schema version, explicitly preserve
archive-v5/schema-v15 acceptance with that fingerprint and prove genuine old
archive restore plus forged-fingerprint rejection. Durable action receipts
must determine the archive disposition; disposable candidate cache is not a
reason to export node-local authority. Receipt persistence, atomic actions,
local index, API/SDK/host/Workbench integration and real browser QA remain
required M4 work, not completed capabilities. No programme scope is removed.

The completed partition-owner review fixes the next integration sequence:
derive query/page digests from exact validated text, provider, effective
locale/region, sorted grain filters and upstream page using version-tagged
deterministic serialization. Derive authorization digests inside the store
from current scopes and credential/subject epochs, not API-supplied digests.
Include provider capability version and current outbound policy: the existing
transport configuration digest covers only provider, capability and origin.
Acquire the existing shared provider lock; authorize before cache/network work;
reauthorize and compare current authority/provider state in the final commit
transaction. Locks do not prevent authorization changes during network I/O.
Preserve browser rotation without binding cache identity to its session ID.

TMDB Search currently sends locale but no region; Google Books Search sends
neither. Record actual upstream coordinates without claiming detail-specific
region semantics. A candidate detail fetch retains Fasti `SearchMetadata` /
`metadata_search` authority and separately checks the provider `metadata.read`
capability state; this is not a new human permission. Record actions retain
their existing mutation scopes. No new grant
revision or restore-generation API is allocated by this preparation. Existing
revocation is terminal; future in-place regrant needs its owner's explicit
generation contract. Also verified: all 29 contract unit tests passed after
the shared identifier parsing fix. M4 remains unmerged.

### M4 immutable Search storage checkpoint — 2026-09-04

The commander now owns an internal, unmerged v16 implementation with immutable
Search pages and candidate receipts. Published migration functions v1–v15 remain
byte-identical to merged M3. No v16 freeze, v17 allocation, shared-file release,
or public Search activation has occurred. `metadata.search` is registered with
its explicit scope and guarded/reserved disposition while integration proceeds.

Storage derives current authorization and provider partitions, rechecks them in
the final transaction, retains browser activity on successful reads and misses,
and requires mutation proof for browser snapshot writes. Pages survive restart,
retain upstream order, and create no Records. Admission is bounded to 100
candidates per page, 1,024 pages and 64 MiB of normalized candidate payload.
Expired-only cleanup progresses even when admission remains over quota.

The existing provider-state owner now distinguishes Search authority generation
from ordinary health updates. Credential/configuration changes and disabled
transitions invalidate receipts; a routine available/degraded health transition
does not invalidate the cache needed during an outage. No second provider
registry, credential owner, dependency or service was introduced.

Archive v5 remains 34 streams. A new regression builds all streams from an actual
published-v15 database, restores populated Records into v16, verifies absent
Search and node-local authority, rejects a forged fingerprint, and proves staging
cleanup. This does not merely relabel a current-schema archive. The existing M3
identity/policy receipt round-trip remains required alongside it. Durable Search
actions, not disposable caches, must determine any later archive-v6 disposition.

Focused Search/schema/browser tests and the genuine historical-archive test pass.
The 114 JavaScript contract/SDK tests pass; strict workspace clippy passes.
Exact capability/profile/problem inventory assertions are retained at 53/18/383.
Run `TMPDIR=/mnt/secondary-ssd/cache/home/tmp cargo xtask contract verify --locked`
on the committed tree for the authoritative receipt. The host's default temporary
directory traverses a symlink; do not weaken SQLite NOFOLLOW to accommodate it.

Parallel read-only reviews confirmed the next required sequence: persist bounded
locale/region/filter route context; implement authorized 24-hour candidate lookup
without raw query text; re-fetch through the existing provider `metadata.read`
state and shared lock; reauthorize after I/O; implement atomic Record actions,
local indexing and the 10,000-Record query-plan gate; then complete API/SDK/host/
Workbench and actual browser QA. These remain active M4 work, not deferrals.

M8 preparation confirms existing M2/M3 policy, cache, override and alias owners
must be reused. Proven execution gaps remain in field coverage, Find execution,
fallback acquisition and related-entity routes. Source conflict: TMDB movie
details documents language and append_to_response, not region; current code's
region query parameter does not prove region-specific claims. Resolve this using
documented release-date/watch-provider response semantics. No M8 production file
ownership is allocated by this checkpoint. Codex Security remains skipped.

### M4 candidate-details persistence checkpoint — 2026-09-05

The candidate owner now persists canonical provider/page/locale/region/grain
coordinates and the original query digest in a bounded 2 KiB context. The
authorization partition binds that complete context digest. It stores no raw
query text and requires no browser-supplied metadata to reopen a candidate.
The context changes remain inside the unfrozen M4 v16; published v1–v15 are
unchanged, and no migration or shared-file release has occurred.

The new store read authorizes first, binds workspace/profile/client/subject/grant
in the receipt lookup, reconstructs current provider/grant/policy/terms authority,
and checks the exact route source and grain. Details remain readable after the
Search cache window until their exclusive 24-hour deadline. Clock rollback,
unknown IDs, changed context or authority, and cross-actor access cannot return
the receipt. The shared bounded snapshot validator also catches missing siblings.
Wrong-grain candidates are rejected before snapshot admission.

Regression evidence covers restart without query text, canonical context bounds,
unknown and corrupt receipts, atomic filter rejection, independent expiry,
real session-owner rotation/profile-return/revocation, expired sessions, and a
different authorized subject sharing the same client/profile/grant. Successful
misses retain throttled browser activity. The 15-second session fixture respects
the existing 10-second write interval; no production authentication rule changed.
Independent read-only reviews found no P0/P1/P2 issue in this persistence slice.
Actual browser rendering and live provider re-fetch are not claimed by these tests.

Next active M4 integration remains governed provider fan-out/re-fetch, atomic
Record Create/Attach with minimal portable replay evidence, local indexing,
stable pagination, and API/SDK/host/Workbench plus browser/performance gates.
Reuse `insert_record`, `attach_identifier_tx`, exact identifier matching and the
existing metadata claim writer in one transaction; do not compose separately
committing public Record methods. Search permission alone cannot write Records.
Keep one Create/Attach replay family and distinguish its workspace-scoped
operation uniqueness from the older observation-specific operation table.
Freeze durable action evidence before deciding archive disposition.

C2 checked the proposed disjoint store allocation and found no honest existing-
schema implementation: required tables and capability/port contracts are not yet
allocated. Shared ownership remains with M4; C2 continues approved independent
work without speculative SQL. Read-only M5 preparation is Library, not Discover
(M6). Neither is promoted before its canonical dependencies are satisfied.
M5 preparation originally identified identifier/activity batch loaders that
reselected the first 500 Records. M4 has since corrected both to use the actual
selected IDs, alongside the metadata batch loader. The UI's inferred saved/watching
defaults are not independent Library state and must not become Search side effects.
Native Collection membership is still a predecessor, not the Nuvio catalog document.

### M4 provider cancellation and failure-state checkpoint — 2026-09-05

Candidate-details head `75331d30184e1d823ac96075cfb70196ccc0806e`, tree
`11c901ccaed216ef969a7fa3054c61b73ddccc81`, passed all 27 native contract gates
with a clean exact-head receipt. That receipt precedes the following hardening.

Provider integration review found that cancelled API/Desktop callers could drop
their gate while blocking vault work continued. Desktop now moves the existing
gate's owned guard into its blocking closure. The four API state-mutating paths
acquire their existing per-provider gate before spawning one admitted worker,
reauthorize after waiting, and retain it through complete state reconciliation.
Cancelled waiters never start later. This is request-cancellation protection,
not process-crash recovery, and must not become detached Search network work.
No new gate map, service, dependency or schema surface was introduced.

Real router tests cover cancelled vault writes, removal finalization, rollback,
credential/health result persistence, waiting cancellation and independent
provider progress. A discovered missing-credential check returned an integrity
error because its status contradicted its absent reference. One application
failure-status helper now preserves reference invariants and health/credential
separation in both hosts. The Desktop regression also covers an externally
removed credential while a reference remains. Native read-only review reports
no P0/P1/P2 issue; post-wait revocation is an additional negative-test target.

Focused application/provider API tests and all 45 Desktop library tests pass.
Strict API and Desktop all-target Clippy pass. Desktop's locked graph needed
only the two existing SHA-256 zeroize feature edges from prior M4 hardening;
no package version changed. Its unused production `scoped_account` function
had only test callers and now lives in that test module; account identity
behavior is unchanged. Browser QA is not claimed by these host tests.

Parallel local-Search preparation confirms identifier/activity enrichment must
use selected IDs. It also found that the current activity loader omits profile
scope. Fix this before reuse. Existing SQLite/keyset/metadata owners remain the
path to local Search; no external index service is allocated. Provider page
results must retain page-level freshness, including empty pages. M4 v16 remains
unfrozen, archive v5 unchanged, and all shared surfaces remain M4-owned.

### M4 selected-Record enrichment checkpoint — 2026-09-05

Provider cancellation hardening is committed at
`342b1313b7a049a443aa64fd3bba14e0dc235daf`, tree
`2535640b93e30c176303544df913947454491ba0`. The subsequent local-Search prerequisite
keeps all three existing enrichment owners on the actual selected Record IDs.
Identifier/activity loaders no longer reconstruct an unrelated first-500 page.
The activity loader now filters the authorized profile before ranking occurrences;
workspace-shared identity does not expose another profile's consumption activity.
The metadata loader reuses the same existing 500-input serialization boundary.

Ten focused identity tests and strict all-target store Clippy pass. New tests use
real grants, evidence uploads and accepted observations to prove profile isolation,
then prove sparse selection beyond Record 500, duplicate/missing IDs, empty input,
foreign workspace rejection and the unchanged bound. Native read-only review found
no P0/P1/P2 issue. This is not evidence of a local text index, public pagination,
10,000-Record latency, or bounded history scanning; those remain active M4 gates.
No schema, archive, registry or external contract changed in this prerequisite.

### M4 governed Search service checkpoint — 2026-09-05

Selected-Record head `094e2d9995f687597c2c4f08a5495d8cb112ef05`, tree
`9ef3c220f8d4adee79aa2c52f43361e04710ce86`, passed all 27 native contract gates
with a clean exact-head receipt. That receipt precedes this service slice.

The provider runtime now composes the existing Search persistence port and
governed provider fetch. Fresh cache hits, including empty pages, perform no
upstream work. Offline and transient outage paths consult only the authorized
stale-on-error window. Stored pages retain their evaluated freshness, lifetime
and response digest. Candidate receipt retention remains a separate 24-hour
details window; it does not extend Search page freshness. Network work is
directly awaited and is not detached on cancellation. Post-fetch authorization
precedes disclosure of both failures and results; final persistence rechecks
authority atomically. No Record or Library action is implied by Search.

Native review found a real transport classification bug: policy/address denials
were flattened into network errors, making them eligible for stale fallback.
The existing runtime error type now preserves configuration denials through
credential authorization. Real transport-denial regressions prove no stale read
or commit. The focused rereview reports no remaining P0/P1/P2 finding.
Codex Security remains skipped as directed; no external AI key is used.

The same review traced M2 blocking persistence after caller cancellation. A
lifetime-only lease now carries the existing host provider guard through every
blocking metadata/Search operation. It introduces no lock map or authorization
owner. API and Desktop supply their existing guards. A paused blocking-worker
regression proves cancellation retains the gate until persistence finishes;
Search sequencing fixtures also prove network cancellation releases it. Those
fixtures are not live HTTP or browser QA evidence.

Focused gates pass: 36 provider runtime tests, 14 real-store Search tests,
4 metadata API tests, all 45 Desktop tests, and strict all-target Clippy for
provider runtime, store, API and Desktop. Empty-page clock fixtures verify
fresh, stale-on-error, expired and future-dated behavior with the production
immutable trigger restored before reads. The filtered-page fixture uses the
same query context as its prepared partition.

Shared ownership remains M4-only. Read-only agents prepare exact host integration
and effective locale/region handling while the commander verifies this slice.
Actual Search coordinates still need normalization before host activation:
TMDB Search sends language but not region; Google Books sends neither. Public
fanout/API/SDK/Workbench, details refetch, atomic Record actions and action
receipts, local text index/keyset/10,000-Record evidence, browser QA and exact
merged-head delivery remain active M4 gates. Schema v16 remains unfrozen;
archive v5 is unchanged. No v17 allocation or shared-file release has occurred.

### M4 effective Search coordinates and landing preparation — 2026-09-05

Governed Search service commit `c766da295dff29630dd7df27c24027fbd8e2c298`, tree
`238e58dae1c289a247144e9248c44f30c8f40831`, passed all 27 exact-head native
contract gates with a clean receipt. C2 received that material checkpoint with
no migration allocation or ownership release.

Search now reuses the existing metadata locale-normalization owner before
preparation, cache reads, fetching or commit. TMDB retains requested language
or canonical `en-us`; its unsupported Search region is absent. Google Books
Search uses neither coordinate. Query text, provider, page and grain filters
are unchanged. This matches the current [TMDB multi-search reference](https://developer.themoviedb.org/reference/search-multi)
and [Google Books volume-list reference](https://developers.google.com/books/docs/v1/reference/volumes/list),
checked 2026-09-05. Google `langRestrict` is a book-language filter, not a
replacement for response localization. Thirty-eight provider runtime tests and
strict Clippy pass; native read-only review finds no P0/P1/P2 introduced. Tests
cover default equivalence, distinct TMDB locales, ignored regional preferences,
Google normalization, idempotence and public empty-cache/offline paths.

This does not change M2 historical detail-region provenance. The documented
movie/TV top-level routes do not prove regional responses. Correcting new
acquisition and deciding historical claim/receipt disposition remain explicit
work; do not rewrite immutable evidence or silently remove M8 regional scope.

Parallel host preparation identifies the existing daemon runtime/kernel and
provider gate map as the composition roots. Search handlers must use the
application browser-or-bearer authentication owner, not the older bearer-only
metadata handler helper. Snapshot-writing browser requests need the existing
mutation proof; candidate reads use the read boundary. Never mount Search on
integration/webhook listeners. Desktop retains its existing shared provider gate;
local results must not wait on it. Preserve the legacy vector-returning Search
path until its receipt-bearing replacement is verified. No implemented multi-
source fanout owner exists yet. Trusted terms revision must come from pinned
provider policy evidence, never browser input; the existing descriptor's posture
label must not be presented as an independently versioned legal-terms receipt.

Parallel local-Search preparation found no existing interactive keyset owner.
Reuse the archive owner's total-order SQL pattern, existing Record indexes and
selected-ID enrichment, not its archive cursor framework. Bundled SQLite already
supports FTS5. The global shortcut currently uses substring matching; retain that
behavior until an explicit, tested replacement exists. Do not index only
`metadata_projections`: reads resolve current profile policy/overrides/time
independently, and that table is not comprehensively maintained. The derived
candidate index must feed the existing resolver without duplicating precedence
or leaking profile-private matches through counts/cursors. One-/two-character,
punctuation, literal FTS syntax and Unicode queries need coverage. Stable Record-
ID pagination is not a frozen snapshot under concurrent title/membership changes.
The 10,000-Record gate must measure the full authorized selection and enrichment,
including extensive-history fixtures; returned-row bounds and SELECT counts do
not prove bounded scanned work. Existing benchmark script ownership is retained.

### M4 local Record Search implementation checkpoint — 2026-09-05

Prior head `25f220c339963e60dd620ca0f97d641c1bf01613`, tree
`891b9d655b0be7ce4536b633e07bae569a63e136`, passed all 27 exact-head contract
gates with a clean receipt. The following local Search slice builds on it.

The existing Search persistence port now implements authorized local Record
Search without a provider, credential vault or remote call. Literal Unicode
default-lowercase substring matching covers title and original title, including
one-/two-character text and punctuation. A single SQLite 1/2/3-character posting
index avoids a second FTS/short-query mechanism or new dependency. It is a
candidate accelerator only: all matches pass through the existing profile
metadata resolver. The Record-summary composition is shared with List Records,
including exact identifiers and profile-scoped activity. No durable tag owner
is invented; the existing projected tags are empty. Host/shortcut integration
must use the explicit default-case contract rather than implicit locale casing.

Public postings cover immutable title claims; private override postings are
partitioned by profile. Claim writes share the existing savepoint. Override
replacement rebuilds only its profile/Record postings within a savepoint;
clearing one field retains its sibling's supported postings. Migration v16
backfills published metadata rows. Restore rebuilds after legacy conversion and
before final verification/commit. These disposable rows add no archive stream;
published migrations v1–v15 and archive-v5's 34 streams remain unchanged.

Each page reads at most 101 postings per public/current-profile partition,
merges and deduplicates by canonical Record ID, inspects at most 100 IDs, and
enriches only selected IDs. Grain filtering occurs after this bound. Cursor
context binds query, grains, workspace, profile and current grant; every page
reauthorizes. Rejected candidate batches can be empty and still carry a
continuation. The cursor is a position, not an authorization grant or immutable
result snapshot. Never label an empty page with continuation as exhausted.

Native review found a measured planner defect: SQLite chose a workspace/grain
covering index and scanned that workspace per candidate (51,323 VM steps at
100 Records; 506,839 at 1,001). The query now uses the existing
`records_workspace_record_idx`; unchanged zero-fullscan and <5,000-VM-step
assertions pass, including absent/sparse grain filters. No new Record index
was added. Read-only rereview found no P0/P1/P2 finding in the resulting diff.

The initial optimized 10,000-Record fixture measured 100 samples:
p50 1.371186 ms, p95 1.429666 ms, max 1.500475 ms. It exercises the full store
call over short, Unicode, punctuation, common and missing queries, not merely
posting lookup. This is dirty-slice store evidence, not exact merged-head,
extensive-history, daemon, concurrent-workload or browser performance proof.
Run it explicitly with `cargo test --release --locked -p fasti-store
local_search_10000_records_release_latency -- --ignored --nocapture` and the
physical TMPDIR. The benchmark refuses debug execution; its p95 <250 ms
assertion is mandatory when explicitly run.

All 324 then-current store tests passed (four ignored, including the explicit
release benchmark). Additional populated-v15 backfill and archive-v3 private-
partition rebuild tests pass. Direct-owner fault injection proves failed gram
insertion rolls back source and index rows without relying on an outer
transaction. Strict store Clippy passes. Full exact-head gates follow commit.

Commander retains every shared production surface. One native agent owned only
the new local Search test file and has released it; independent reviewers remained
read-only. Codex Security is not used, per the user's repeated instruction.
Next M4 work includes provider/local host composition, trusted provider-policy
revision, details refetch, atomic Record actions/action receipts, generated
contracts/SDK/UI and browser QA. The selected metadata loader still retains up
to 256 claims per field across a page; extensive-history memory and scanned-work
gates must be resolved within M4 before runtime completion is claimed. Streaming
resolution must reuse the existing domain resolver, not truncate history or
copy its precedence rules. No v17 allocation or shared-file release has occurred.

### M4 metadata-history memory and query-work checkpoint — 2026-09-05

The preceding local Search commit `5d3ae4f9c97242794a1654c00b2dda6ad4e3bd2c`,
tree `55972c175226ee64f8ff43933a9ed0fe6aa465b3`, passed all 27 contract gates
with a clean exact-source receipt. Its optimized 10,000-Record Search fixture
measured p50 1.421807 ms, p95 1.477787 ms and max 2.025606 ms over 100 samples.
That fixture has sparse metadata history; it does not qualify dense history.

The shared selected-Record metadata loader now resolves one field's bounded
history at a time and retains only resolved results. It reuses the domain
resolver, profile overrides and one batch timestamp. Every selected claim is
still decoded and validated, including claims hidden by an override. The
256-claim limit, source/time ordering, provenance, lifecycle status, policy,
empty fields and public Record-summary interface are unchanged.

One compound provenance index in unfrozen v16 selects the first 256 joinable
claim keys per requested Record/field. The existing claim index supplies the
covering existence check; payload values are read only after narrow-key sorting.
The explicit outer order and bundled-SQLite query-plan assertions prevent a
silent return to a page-wide payload sort. No new table, capability, dependency,
resolver or cache owner was introduced by this hardening slice. Published
migrations v1–v15, archive v5/34 streams, and C1 remain unchanged. Migration fault
injection checks that the new index rolls back with the other v16 objects.

Native differential checks cover 255/256/257 claims, exact selected IDs at a
same-time/source tie, missing provenance before the cap, duplicate requested
fields, sparse selected Records, mixed provider/locale/lifecycle history,
override-only and empty fields, and corruption despite a winning override.
Query-plan checks distinguish narrow-key sorting, covering existence reads and
primary-key payload reads. Increasing a selected field from 256 to 4,096 claims
does not increase the measured VM work or the eight constant-field scan steps.
With the final covering read, the bundled-library regression records 40,787
VM steps at both depths. The final sparse 10,000-Record Search run remains green:
p50 1.427837 ms, p95 1.448247 ms, max 1.482007 ms over 100 samples.
One test agent owned only `metadata_batch_tests.rs`; independent reviews and
SQLite diagnostics were read-only. Commander retains all shared production
files. Codex Security is not used.

After the covering-index refinement, the full store suite passed: 333 unit
tests and three integration tests, with five explicit ignored unit fixtures.
Strict all-target store Clippy passed. These are working-tree results; clean
exact-source contract verification follows the slice commit.

The explicit dense release fixture uses 100 or 500 selected Records, five
canonical fields, 256 claims per field and 4,096-byte values. It seeds a real
disk database without retaining the fixture in a Rust vector and checks Linux
process peak memory against 192 MiB. Run `cargo test --release --locked -p
fasti-store metadata::batch_tests::metadata_batch_dense_release_memory_and_latency
-- --ignored --exact --nocapture --test-threads=1`, with physical TMPDIR; set
`FASTI_METADATA_BATCH_RECORDS=500` for the legacy List Records maximum.
Before the final covering-index refinement, the bounded-query runs measured
100 Records: median 4.798923821 s, max 4.988622487 s, peak 21,233,664 bytes;
500 Records: median 29.990127061 s, max 76.583983004 s, peak 36,474,880 bytes.
Memory passed; these timings do not meet interactive Search latency. Final
covering-index measurement for 100 Records: median 2.932160553 s, max
2.954091063 s, peak 20,926,464 bytes. This improves the measured batch without
changing its inputs or skipping validation; latency is still not qualified.
The final 500-Record run measured median 17.614984496 s, max 21.618950448 s,
peak 37,019,648 bytes. Both final runs preserve the 192 MiB memory ceiling;
neither is an interactive latency pass. Exact-source gates follow commit.

Do not use persisted `metadata_projections` as an unproved shortcut: generic
claim writes, other profiles, override/policy changes, lifecycle changes and
expiry do not maintain their coherence. Restored projections are empty. Current
reads must retain authoritative resolution and validation. Dense latency remains
an active M4 gate alongside host composition, policy revision, details refetch,
atomic actions, contracts and browser QA; it is not deferred to another stage.

The parallel native GitHub advisory audit found current `fast-uri` 3.1.5 and
`qs` 6.15.3 lockfile matches requiring bounded updates (patched versions reported
as 3.1.6 and 6.16.0). Existing image-size patches and the documented Tauri/glib
exception must not be discarded. Open grouped dependency PRs #123/#124 overlap
these owners. Read-only inspection found neither fixes these two versions:
#124 at `a06c9a0ec9a198ed568de5f4ddd2bcbd7eb0d622` retains both; #123 at
`4e454d5360e164457b9dca88d47e00a19b61e071` changes only Rust dependencies.
Ajv's existing range permits the fast-uri patch. Express/body-parser's qs tilde
ranges do not permit 6.16.0; use the existing bounded override owner, preserve
the parents, and verify their loopback docs-dev-server caller when updating.
No M4 PR or hosted exact-M4-head review exists yet. No v17 allocation or shared-file release
has occurred.

### M4 dependency and documentation QA checkpoint — 2026-09-05

The metadata-history slice at `12af3a2acbe12b0491bf9ce5008acf8a28dfa5f9`, tree
`5029dabded8ce68de068498ff459d91ae01d6bbe`, passed all 27 canonical contract
gates with clean exact-source evidence. Its dense-history latency limitation
above remains open; the pass does not remove that M4 gate.

Commit `06c5b698c2011051b72781dc3ee5e2b75b39b1e0` updates only fast-uri
3.1.5 → 3.1.7 and qs 6.15.3 → 6.16.0, using the existing central override for
the parents' qs tilde range. The newer fast-uri patch also fixes port injection
and bracket-authority parsing. Native consumer-chain tests exercise the actual
Ajv resolver and both Express/body-parser dependencies, plus valid controls.
The existing image-size patch and Tauri/glib exception remain intact. Local
audit has no unignored advisory entries; it still counts two tracked high
image-size advisories. This is not a zero-vulnerability claim. No external AI
credential or Codex Security service was used.

Docs packaging exposed a separate physical-path mismatch in this worktree's
symlinked target directory. A clean rebuild reproduced all 54 failed MDX renders;
generated metadata was present, but webpack's physical resource paths missed
the docs loader's lexical include. Commit `9f8a1182797e4908a4a539c9033d8b839f782f01`,
tree `2ef5c57df49c18405be187d09d16cce2af721a4e`, resolves the existing docs path
with the standard library. The exact-head canonical docs package passes all
59 routes and builds its search index. Validation and webpack symlink handling
remain enabled. No new dependency, API or infrastructure owner was introduced.

The local JS suite passed 144 tests with two browser tests initially skipped;
all four docs browser checks subsequently passed against the built site.
Native browser QA covered six pages, docs search results, navigation, back/forward,
invalid deployment input/reset, mobile menu Escape/focus and overflow. No browser
defect was observed within that quick scope. This does not qualify provider media
Search, full accessibility, dense latency or deployment. The local report is
`.gstack/qa-reports/qa-report-local-docs-2026-09-05.md`, also retained in the
existing project-scoped QA artifact directory. No public deployment occurred.

Parallel allocation remains one shared-surface commander. Native agents supplied
an independent dependency review, exact consumer regressions and the next trusted
cache-policy map. The next minimal cache-policy change reuses provider descriptors
and existing cache partition keys; legal posture must not masquerade as a cache
revision. Preserve old immutable refresh receipts and exact command replay.
Workbench bundle preparation remains read-only: shared generated catalogues are
retained, and any route splitting must prove offline navigation, focus and chunk
failure recovery before implementation. No speculative projections were added.

C2 proposed a foundation-only delivery from source
`5eb3def007f712f62038f4f1bd8c64f76c47098e`, tree
`2c15dd499281dc91fcbc3bb8325eb8878f1c04fe`. Read-only reconciliation against
observed dev `df09101028a988a92f4546313c5eed6dd20d238a` is clean; against M4
`9f8a1182` it finds only the typed-ID registry-count assertion (27 versus 26).
Preserve M4's 27 and reserved `scr_`. The three shared ID/secret patches are
equivalent; module merging must preserve both access credentials and Search.
C2 may prepare that bounded delivery, excluding later C3 work, subject to exact
tests/review/merged-tree proof and corrected stale M3 ownership text. This is not
approval to activate C2 or modify M4 schema, store, registry, API, SDK, host or
Workbench. No v17 allocation or shared-file release has occurred.

### M4 trusted cache-policy checkpoint — 2026-09-05

The preceding dependency/docs checkpoint at
`25f20b2a4c7a834d8530bc64b9158ae24d624387`, tree
`dfab3cc1f1cb5ecbcd8126ab32417ce85d1b3540`, passed all 27 canonical contract
gates with a clean exact-source receipt. Its full JS suite passed 145 tests;
the two opt-in browser cases were separately exercised in the four-test docs
browser run. That evidence does not complete M4's media Search integration.

Active TMDB and Google Books descriptors now declare the implementation revision
`fasti.public-metadata-cache.v1` in their existing `cache_policy` field. M2's
enrichment and offline cache keys consume that descriptor, not `licence_and_terms`.
Search replaces caller-selected revisions before any prepare/cache/network work;
the same trusted request reaches stale reads, post-I/O authorization and commit.
The existing `terms_revision` storage slot is retained. This is a Fasti policy
revision, not evidence of an upstream legal-terms revision or permission to
redistribute metadata or share profile-bound Search receipts. Provider legal
posture and attribution remain separate and unchanged.

Existing immutable operation receipts still replay their original result and
semantic digest. New cache lookups use a distinct partition; old entries are not
rewritten, alias-read or deleted. A real SQLite regression changes only the
revision, proves the new partition misses, and proves the old entry remains.
Runtime tests assert trusted policy at every persistence boundary, including
empty/caller-selected/legal-label input online and offline. An unknown-provider
negative test caught an invalid typed-error mapping: `provider_route_unavailable`
is not allowed for SearchMetadata. Descriptor misses now return its existing
`validation_failed` problem before persistence or fetch. No registry expansion
or error-validation bypass was added.

Focused evidence: all 39 provider-runtime tests pass; strict provider/store
all-target Clippy passes. Store tests pass for revision isolation, exact refresh
receipt replay, cache-hit receipt replay, Search partition separation, candidate
authority rechecks, historical archive-v4 immutable receipts and archive-v5/v15
restore into v16. Native independent review remains read-only. Clean exact-source
contract verification follows the commit; Codex Security remains off.

Next details preparation must atomically snapshot the existing Search receipt
authority and separate provider `metadata.read` state inside the Search store
transaction. Reuse `authorize_application_transaction` for browser or credential
access; do not fabricate credential context from a browser session. Existing M2
refresh is Record-bound and is not a pre-Record detail service. Recheck both
authority and provider state before exposing post-fetch success or failure.
Future details requests derive the same descriptor revision, never the stored
receipt's old revision. No production details host constructor exists yet.

Explicit Create/Attach actions must reuse the existing identity and metadata
transaction helpers, preserve unrelated Library/progress/rating state, and commit
durable retry outcomes atomically. Freeze those retry/archive semantics before
adding durable action storage; published archive v5 must not silently acquire
another stream. No action receipt, migration v17, archive-version change, API,
shared-file release or M4 merge was introduced by this cache-policy slice.

### M4 candidate-detail runtime checkpoint — 2026-09-05

Trusted-policy commit `46431578a4818e307780261725735abcbff4ced0`, tree
`49c34d401323d6f04038a80a1c558958e015b384`, passed all 27 canonical gates
with a clean exact-source receipt. The next bounded slice implements read-only
candidate details through the existing Search persistence and provider runtime
owners. No public route, generated contract or Workbench activation is claimed.

Online preparation authorizes the existing receipt and snapshots the separate
`metadata.read` capability in one store transaction. Refetch coordinates come
only from that validated receipt; the existing provider locale owner determines
the effective locale. Current top-level movie, series and volume detail routes
do not promote the original requested region into response provenance. Both
success and provider failure require another atomic preparation; changed receipt
or provider authority fails closed. Health-only state changes retain authority.
Returned normalized identity and bounded public fields must match the original
receipt. Fresh details do not overwrite the immutable receipt, extend its expiry,
write a Record, or authorize an eventual Create/Attach action.

Offline details read only the original authorized snapshot; they do not require
`metadata.read`, resolve DNS, load credentials or access the network. Both paths
derive the current descriptor-owned cache revision. Successful missing-receipt
reads commit browser activity; authorization errors still roll back. Nested
provider outcome codes are not transport-level SearchMetadata problems: future
API wiring must preserve that distinction rather than bypass typed-error rules.

Real SQLite checks cover missing/disabled/unavailable read state, independent
Search partitions, configuration away-and-back authority changes, receipt
coordinates and expiry, cross-profile and revoked-scope denial, and no Record,
identifier or claim writes. Existing browser fixtures additionally prove session
rotation, profile return, revocation, expiry, stable subject isolation and activity
on an authorized miss. Full store tests pass: 338 unit tests and 3 integration
tests, with 5 unit subprocess/performance cases and 1 documentation case ignored
as declared by the suite. All 47 provider-runtime tests pass, including 8 detail
tests for invalid upstream identities and fields, both post-fetch success/error
rechecks and cancellation. Network cancellation releases the gate; cancellation
during a blocking post-fetch read retains it until that worker finishes. Strict
provider/store all-target Clippy and formatting pass. Native review
found no concrete production defect; exact-source contract verification follows
the commit. Codex Security remains prohibited.

Parallel allocation remains compact: commander owns the active shared Search
surfaces; one agent verifies the exact detail slice and negative cases; two
read-only agents trace the existing atomic Record/metadata transaction owners
and durable retry/archive disposition. Test leaf ownership is explicit and
released before integration. No second roadmap or speculative API is added.
Durable actions, host/SDK/Workbench integration and dense-history latency remain
active M4 work, not completed or removed scope. Existing workspace-revision
triggers cover metadata resolver inputs, but expiry, rollback revision reuse and
restore identity require explicit proof before any validated-result cache.

C2 separately owns only its bounded inherited licence correction and the two
qualified rustdoc links in the Nuvio application module, subject to exact final
reconciliation. M4 has not released v16, allocated v17, changed archive v5 or
activated C2 runtime capabilities.

### M4 shared candidate metadata and action disposition — 2026-09-05

Candidate-detail commit `e6ce760df5bca95f3b3785e597be5b132911cb0c`, tree
`88e786c46fd51c756190aeb59e5ca839a9d3cbb1`, passed all 27 canonical gates
with a clean exact-source receipt. This follow-on reuses one application-owned
five-field conversion for online provider details and original cached Search
evidence. The runtime wrapper retains its existing observation time, 24-hour
metadata freshness and callers; it no longer owns a duplicate field mapping.

Cached conversion supplies the receipt's accepted response digest, original
timestamp and 120-second freshness deadline, with effective stored locale and no
unsupported region. It validates context against the original partition digest.
The receipt's separate 24-hour readability lifetime is not metadata freshness.
Zero-freshness historical receipts project an initially Stale claim with no
invented deadline; this cannot become indefinitely fresh. Conversion is pure
evidence projection, not authorization: the forthcoming action transaction must
still recheck receipt expiry and current scope. Authors remain in candidate
evidence; no speculative metadata field was added.

Seven application regressions cover complete/minimal field sets, stable immutable
semantics, context substitutions, provider coordinates, response digests and
freshness boundaries. Four real SQLite regressions preserve existing persisted
claim IDs on identical writes, original stale evidence, and original rows after
a timestamp/digest collision. Later-field namespace and provenance failures roll
back the existing provider Record transaction, including local Search postings.
These tests prove existing metadata-write idempotence and rollback, **not** the
unfinished durable Search operation replay. The runtime regression verifies one
received time across all fields, existing freshness and invalid-response mapping.
Full checks pass: 137 application tests, 48 provider-runtime tests, 342 store unit
tests and 3 store integration tests; the store's 5 declared ignored unit cases
and 1 documentation case remain reported, not silently counted as passes.
Independent native exact-diff review found no concrete P0/P1/P2 finding.

The canonical plan now records the action disposition without restarting prior
planning gates. Both Create and Attach reuse existing AttachIdentifier authority;
new operations separately require Search scope. Completed exact retries retain
historical outcomes after candidate expiry/configuration change, but never bypass
current IdentityWrite or browser mutation proof. Online exact refetch stays the
normal path. An explicit cached action preserves original evidence; it is not an
automatic fallback after invalid data or failed authorization. Stable actor,
client, profile, action, target, route and evidence mode bind durable operation
identity. Existing namespace/identifier/metadata transaction bodies remain owners.

M4 has explicitly allocated archive **v6** for the durable Search action stream.
No schema or archive implementation is changed in this slice: runtime remains
v5 with 34 streams, historical v1–v5 remain frozen, and v16 stays with M4. Import
retains historical subject IDs only as audit evidence, with no excluded-auth FK
or recreated authorization. Fresh recovery clients and browser subjects cannot
inherit prior actors' retry rights. No v17 or shared-file release has occurred.

Next implementation is the operation-bearing action command and durable receipt,
one immediate Record/claim/result transaction, followed by archive-v6 roundtrip
and adversarial replay checks. Agents prepared exact command/digest, store and
archive maps read-only; the commander retains all shared integration writes.
M5's selected-ID batching gap is now correctly marked resolved. Its independent
Library state/filter/pagination work and M7 native-membership dependency remain
visible; Nuvio configuration JSON is not native Collection membership.

C2's isolated foundation at `23ffff55b5701c91f3c1ef9297bffa1eba5b37d5`, tree
`fc74c898acd6c562d4e8b6f85b94cf5fc1978ab1`, received a native read-only
exact-diff review against `df09101028a988a92f4546313c5eed6dd20d238a`: no concrete
P0/P1/P2 findings. Review verified the 16 original source/test/manifest blobs,
exact licence exception/checksums and two qualified rustdoc links. C2 reported
canonical and strict rustdoc passes; those were not rerun by this review. Its
bounded factual C1 merge-status documentation pass remains separately owned and
must receive final exact-head reconciliation. No runtime activation or M4 scope
release follows from that review. Codex Security remains prohibited.

### M4 atomic Search actions and archive v6 — 2026-09-05

Predecessor `3b8dd98f5c73c7de783e6e952717814c42005611`, tree
`e649e476e49d684e840b7a54fd55ad6cae21df5d`, passed all 27 canonical gates
with a clean exact-source receipt. This slice implements the previously frozen
Create/Attach action disposition through existing application, provider-runtime,
identity and metadata owners. No new dependency, capability or parallel writer
was introduced. AttachIdentifier now permits its required IdempotencyConflict;
generated problem/SDK projections are regenerated from that policy.

First save reauthorizes IdentityWrite, real browser mutation proof when applicable,
and the original scoped Search receipt. Cached evidence is explicit and never a
fallback after failed refetch. Online evidence uses exact stored coordinates and
effective provider locale, then rechecks authority after I/O. One immediate
transaction commits namespace registration, exact identity reuse/attachment,
metadata claims, local Search postings and the durable result. Later-field or
receipt failure rolls everything back. No Library, progress, rating, override or
Collection state changes implicitly. Shared provider-field and namespace writers
remain the sole owners.

Completed replay requires current write authorization and stable actor/profile/
client/operation intent, but not an expired candidate or current provider state.
Real browser tests prove rotation, revocation, read-proof denial and different
subject isolation. Their fixture grants write authority before Search: changing
the grant afterward correctly invalidates the original Search partition. Runtime
checks cover zero fetch on cached/replay paths, exact response provenance,
post-fetch denial, concurrent replay and lease retention during canceled blocking
work. A separate real-SQLite contention regression prepares both commands before
two threads commit: identical operations return one receipt, while distinct
operations reuse one Record without duplicate identifiers or claims. Receipt
decoding reuses canonical provider mappings and bounded typed data;
legacy cached TMDB locale-None remains valid, while actual refetch locales are
enforced for both TMDB and Google Books.

Archive v6 appends only durable action receipts as stream 35. Historical v1–v5
schemas/examples remain byte-identical. The published schema-v15/archive-v5
fingerprint still restores; schema v16 pairs only with v6. The import owner checks
canonical nested receipt bytes, every redundant column, recomputed intent digest,
portable scope and Record grain. Historical subject IDs are audit values without
auth foreign keys. Four archive tests prove populated roundtrip and exact stream
re-export, hostile receipt rejection, missing/wrong portable relationships, and
real recovery bootstrap: a freshly authenticated replacement client cannot replay
either prior credential or historical-browser operations. Temporary Search state,
credentials, grants and sessions stay excluded. Recovery does not confer old
actors' replay rights.

Validation before commit: 141 application tests, 55 provider-runtime tests,
361 store unit tests and 3 store integration tests pass, followed by the added
real-SQLite contention test passing independently; 5 declared store unit
cases and 1 documentation case remain ignored. The 13 portability contract tests
and 18 generator tests pass. Strict all-target Clippy for application, contracts,
provider runtime, store and xtask passes, as do formatting and diff checks. Native
independent review found no concrete remaining defect. Clean exact-head canonical
verification follows the commit; the predecessor receipt is not evidence for
this new diff. Visual accessibility is not claimed by this headless slice.

Parallel allocation: commander retains v16, archive v6 and all shared integration
surfaces. Separate agents owned only explicit application/runtime/store test
leaves, releasing them before integration. Independent readers reviewed current
code, prepared host/API/SDK/Workbench wiring, and traced dense-history performance.
Next measured optimization is the domain resolver's quadratic duplicate-ID scan;
do not introduce a cache before measuring the standard-library improvement.
Dense cold latency remains an open M4 gate. Search host/wire/Workbench activation
and canonical direct-link details remain unfinished M4 work, not removed scope.
No v17 allocation, M4 merge or shared-file release has occurred.

C2 PR #125 separately reports pushed head
`0d085fcb57dbe70d7640815da199f75370452e62`, tree
`90b9ab4eb555f88367a1d2008ec2d02dc720d54a`. Native reconciliation verified
the exact released dependency fix and physical-docs-path blobs plus a bounded
Playwright route-availability race correction. C2 reported its clean local
canonical, docs and built-site gates green; remote exact-head CI is pending,
with no merge or M4 ownership release. Refresh merge analysis only on a material
handoff. Codex Security remains prohibited; reviews and gates use native tools.

### M4 archive verification follow-up — 2026-09-05

Atomic actions are preserved in commit
`bf69caf1517b36871d13038b8fd8fe6687af56e5`, tree
`744a67f6dd5433ff075772ad7216b55607faaba8`. Its first exact-head canonical
run stopped at JavaScript formatting: the existing generator-owned exclusions
covered archive v2–v5 but omitted the newly generated v6 schema. No success
receipt was emitted. The follow-up extends that exact generated-file ownership
rule to v6; reproducible generation and checked-in byte comparison remain gates.

Inspection also found that the existing offline JSON Schema validator compiled
only archive v1/v2. It now checks v3–v6 through the same strict Ajv owner, including
format/stream count, local-only references, actual example validation, canonical
manifest checksum and frozen preceding stream bytes. Regression tests mutate the
versioned inputs, so missing files or generic failures cannot masquerade as schema
proof. No archive bytes, application/runtime code, capability or migration changes
are part of this verification correction. The next clean exact-head run must pass
before this follow-up is reported verified.

Focused authored-contract checks pass: 45 tests, including 23 new archive
regressions for v3–v6 versions, missing/reordered streams, stale checksums,
correctly rechecksummed changed prefixes, external references and malformed
schema compilation. Scoped formatting and the full formatting gate pass.

The next exact run passed those checks and typechecking, then exposed one stale
generated-contract inventory expectation: AttachIdentifier's authored
IdempotencyConflict makes 384 canonical problems, not 383. The fixed-count
sentinel is updated to that verified single policy addition; it is not removed
or replaced with a permissive bound. Other 138 mutation/SDK checks passed in that
run; no complete verification receipt was emitted. Public Search action wiring
must also review the shared conflict next-action wording, which still mentions
an observation despite its existing metadata/Search uses.

### M4 profile-switch regression and parallel preparation — 2026-09-05

Commit `580998f40ba09ec1d41230bab0841093ab5be841`, tree
`cafa63cec20ec67372b92d074337c2a8e192cdb7`, passed all 27 canonical
contract gates with a clean exact-source receipt. That verifies the committed
internal action/archive slice, not public Search activation or an M4 merge.

Independent review identified a test gap, not a demonstrated defect: prepare an
uncommitted action, switch the real browser session's profile, then attempt to
commit. The new regression covers Cached and Refetch. The old proof returns
BrowserSessionRevoked; the new profile returns ValidationFailed for the original
candidate. Fifteen existing persistence owners, including browser sessions and
workspace revisions, remain unchanged after each rejected action. The focused
test passes with physical SSD TMPDIR. No production behavior changes here.

Parallel readers refreshed the exact public Search wiring map and bounded M9a
and M11a preparation. Search must reuse scoped-or-browser application
authentication, not the bearer-only metadata HTTP helper. MDBList aggregate-score
origin and account/profile/purpose retention require an explicit disposition in
existing rating owners before activation. M11's in-memory conformance outbox and
process-local provider lock are not durable journal or fencing implementations;
completed Search replay rules also must not be copied as synchronization authority.
These are read-only dependency maps, not new APIs, migration allocations or
production branches. Commander retains v16, archive v6 and shared integration
surfaces. M4 remains unmerged; no v17 or shared-file release has occurred.

The clean predecessor's 100-Record dense release baseline passed its memory
ceiling (median 3.141628070 s, max 3.440542170 s, peak 21,676,032 bytes).
The 500-Record run returned StorageUnavailable before timings. Investigation is
in progress: this host's /tmp is now tmpfs, unlike the prior explicit SSD
benchmark environment. Do not classify the cause or change the resolver until
the underlying failure and a controlled physical-SSD baseline are verified.
Codex Security remains prohibited; all review and verification here are native.

The storage investigation subsequently confirmed **EDQUOT**, not a resolver
regression: the unchanged release test's SQLite database and temporary-file
`pwrite64` calls returned `Disk quota exceeded` on the user-quota-enabled /tmp
tmpfs. Remaining filesystem-wide capacity did not establish available user quota.
The scoped diagnostic is `target/dense500-storage-failures.strace`; its traced
723-second runtime is not performance evidence. No quota, global temporary
directory or production error mapping was changed.

The same unchanged release binary passes all five 500-Record samples with
`TMPDIR=/mnt/secondary-ssd/cache/home/tmp`: median 18.174888723 s, max
68.175424069 s, peak 35,889,152 bytes. This controlled reproduction resolves the
storage failure, not the outstanding dense-latency gate. Benchmark commands on
this host must use that explicit physical SSD location: resolving /tmp with
readlink is insufficient now that /tmp is a real, quota-limited mount.

The test-only profile-switch commit is
`010478224061153508cbad9a31367bb73715dfc3`, tree
`f89947b4b1905d814640de7a7a84959e345738ce`; its focused regression and strict
all-target store Clippy pass. Independent review is clear. The session comparison
does not force an activity-write rollback because the fresh session remains
inside its activity throttle; do not claim that additional coverage.

Next bounded resolver ownership: one agent may change only the domain metadata
validator and its tests, using the standard-library HashSet while preserving
per-claim error precedence. No speculative cache or new abstraction is allocated.
Commander measures the same dense fixture and retains shared integration files.
The next transport pass can reuse ListRecords with an optional exact RecordId
selector and the existing selected-ID enrichment loader. Default list behavior
and profile authorization must remain unchanged; direct details must not scan
the first 500 Records or implicitly gain tracking-state authority.

The matching unchanged-binary 100-Record SSD baseline also passes: median
2.932622939 s, max 2.949880879 s, peak 21,225,472 bytes. Compare the resolver
change against these SSD baselines, not the tmpfs run or traced timings.

The bounded validator change replaces only duplicate-ID and unknown-lifecycle
membership rescans with a temporary standard-library HashSet. Each ID is inserted
at the original per-claim validation point; target, lifecycle, override, expiry
and winner-selection rules remain unchanged. The set is dropped before ranking.
All 23 domain metadata tests and strict all-target domain Clippy pass. Independent
native review is clear. This does not change the separate lifecycle traversal
algorithm or claim to make the entire resolver linear for arbitrary event lists.

The changed 100-Record SSD measurement passes: median 2.917748803 s, max
2.929780586 s, peak 20,946,944 bytes. The approximately 0.5% median difference
does not establish a meaningful end-to-end speedup. No cache was added, no dense
latency gate was waived, and the 500-Record comparison remains to be completed.

The changed 500-Record SSD run also passes: median 16.353478279 s, max
16.638940383 s, peak 37,134,336 bytes. Its median is lower than the 18.174888723 s
baseline, but this single sequential comparison does not isolate filesystem/cache
variation or establish interactive latency. Both sizes remain within the
192 MiB peak-memory ceiling; dense latency still fails the programme's intended
interactive outcome. The separate sparse 10,000-Record query gate remains distinct.

Post-change sparse Search evidence: 10,000 Records, 100 samples, p50 1.476397 ms,
p95 1.592627 ms, max 2.708454 ms. Full store verification passes 363 unit tests
and three integration tests, with five declared ignored unit fixtures and one
ignored documentation example. Strict all-target domain/store Clippy, formatting
and diff checks pass. Clean exact-source canonical verification follows commit;
the preceding head's receipt is not evidence for this changed source.

### M4 direct Tabler imports and material C2 handoff — 2026-09-05

The resolver commit `3462f1fab9e5a22e43bebf77047092dd39ae1963`, tree
`6d3780e892f8f3a60ffb321d4d32119602e98191`, passes all 27 canonical contract
gates with a clean exact-source receipt. All earlier M4 results remain committed.

The next mechanical UI change uses the already-installed Tabler package's
supported direct icon subpaths, following status-panel's existing pattern.
Across 24 files, 199 imports retain the same local identifiers and the same 96
upstream icon implementations. Independent review confirms every non-import byte
is unchanged: no component, prop, behavior, style, dependency or config change.
The web build transforms 279 modules instead of 6,378 and takes 1.33 s rather
than 3.84 s in this comparison. CSS retains identical bytes/hash; JS remains
740.90 kB, while gzip grows from 146.57 to 148.36 kB. This is a build-work
improvement, not a download-size saving. No CSS pruning or speculative lazy
component framework was introduced.

UI typecheck, Tabler boundary/policy, formatting and diff checks pass. Both
existing navigation-semantics Playwright tests pass on the isolated health-stub
fixture: eight routes retain one current navigation item, and empty Media Detail
retains its named recovery surface with no axe violations. This is bounded UI
regression evidence, not live provider or complete accessibility qualification.

C2's material handoff now reports PR #125 merged into dev at
`62e10d2e9bd738ed5da425c008eb839f89cdbea5`, tree
`d6fcea1563b673f83cb4cabe1ef50d1c6dc5c087`, matching reviewed PR head
`90374622ec5bad52beabf9405835bba51b56dda5`. All reported PR checks passed,
including the hardware windows. Commander must independently fetch/verify before
rebasing the clean committed M4 branch. Preserve scr_, the 27-ID inventory, both
Search/access_credentials exports, MetadataSearch default-deny, and dev's actual
physical-docs-path regression. This is not a new dev-push qualification claim.
Merged v15/archive v5 remain frozen; M4 retains v16/archive v6 and every named
shared surface. No v17 allocation or M4 shared-file release is authorized by
this C2 foundation merge. Codex Security remains prohibited.

### M4 integration onto the verified C2 foundation — 2026-09-05

Fetched origin/dev exactly matches C2's handoff: commit
`62e10d2e9bd738ed5da425c008eb839f89cdbea5`, tree
`d6fcea1563b673f83cb4cabe1ef50d1c6dc5c087`. The pre-rebase M4 head
`ffcb61b596e63a2f4833e206513c785245d0a481`, tree
`5abe7b9eaa970acec9bd5f10f6ad632748b11bca`, is retained on the local recovery
reference `codex/m4-before-c2-foundation`. No remote history was rewritten.

The clean rebase produced `eb6d32be786dfa44fa98f42e2d071b45a709cee3`, tree
`ca82c5e90d2ae16123d87033e175ef0550931c30`, with the exact handoff as its
merge base. Range comparison preserves all M4 implementation patches. Four
already-upstream ID/secret/dependency prerequisites were automatically omitted.
The only conflict was the older docs-path source assertion; the production
configuration was already byte-identical, and the newer executable symlink/ENOENT
regression supersedes that assertion. Its resolved blob exactly matches dev:
`e036664d7904fa6d9e6426697764c9b997135917`. No other M4 implementation was dropped.

Independent post-rebase comparison confirms unchanged M4 IDs, scopes,
capabilities, access/store, v16 schema, archive v6 and entire UI sources. Domain
and application module lists only gain the expected access_credentials exports;
Search remains exported, Reserved/Guarded and absent from FULL_ADMIN_SCOPES.
The incoming C2 source, dependencies and behavioral docs regression remain
byte-identical to the merged dependency. This source-preservation review has no
concrete findings. It does not establish runtime PAT support, full M4 completion,
or post-rebase verification. The review checklist is applied to this integration;
completed planning gates and unrelated global artifact-sync queues are not rerun.

Run the unchanged canonical PR gate on the next clean checkpoint commit with
`TMPDIR=/mnt/secondary-ssd/cache/home/tmp PKG_CONFIG=/usr/bin/pkg-config cargo xtask test pr`.
It includes contract verification, docs verification and the portable B1 checks.
M4 still owns v16/archive v6 and every shared integration surface; C2 activation
waits for M4's actual merged handoff. No v17 or shared-file release has occurred.

### M4 exact C2-foundation gate and provider outage correction — 2026-09-05

The full canonical PR gate passed on clean commit
`ebbb15bf0c8bbfe3374b9399d99472d8c63dee96`, tree
`973d90f35f745141989bb10793debe95e95ce604`. Both local receipts identify that
exact clean source: 27 contract gates and 11 portable gates passed; documentation
verification also passed. This is local software evidence, not new hardware,
deployment or M4 completion evidence.

The subsequent read-only offline audit found that real HTTP 500/502/503/504
responses were mapped to invalid-response errors, bypassing eligible stale Search
pages. The shared status mapper now reports provider-unavailable for exactly those
statuses. Search, provider checks and detail fetches reuse this owner; credential,
policy, vault and malformed-response boundaries are unchanged. Actual mapper
results enter the stale-fallback regression for each active provider, including
negative 401/403/404/501/505 cases. All 56 provider-runtime tests and strict
all-target Clippy pass after the change. The preceding full-gate receipt does not
cover this later diff.

The audit also confirmed an open section-18 implementation requirement: response
cache directives currently do not shorten the fixed Search lifetimes. Address it
through existing response and receipt-lifetime owners, not another cache/schema.
Fresh-page reuse, stale-page reuse and 24-hour candidate evidence retention remain
separate. In particular, zero freshness is not equivalent to no storage. Resolve
no-store/private admission and historical-save semantics explicitly before claiming
response-policy coverage. Public provider Search composition remains unfinished;
do not confuse backend receipts with verified browser offline/partial behavior.

### M4 direct Record selector integration — 2026-09-05

The commander owns the application/API/registry/generated/SDK integration. The
store agent's explicitly allocated identity.rs leaf is integrated and released;
all other agent work remains read-only. Existing ListRecords now supports an
exact typed selector, using the primary key before the unchanged profile-aware
enrichment owner. Its ordinary 500-row list behavior is unchanged. The existing
GET records route takes optional record_id; SDK CallOptions remains first, with
the optional query second. Generated parsers, OpenAPI and problem catalogs come
from the canonical generator. No migration, archive or new capability was added.

Independent review found two issues during implementation: malformed selectors
could precede browser/scope authorization, and nullable generated input disagreed
with SDK omission behavior. Both are corrected. The application carries a typed
InvalidRecordSelector to the existing authorized store transaction; null SDK
input normalizes to omission. Unknown/duplicate/bad selectors remain rejected,
and exact responses must match identity, contain at most one row and not truncate.
Independent final review has no remaining concrete findings.

Four focused selector tests pass, including Record 501 enrichment, profile
override/activity isolation, unknown/foreign/inactive/local-only cases, missing
IdentityRead, malformed input after real browser revocation, and constant SELECT
count plus indexed/no-full-scan/no-sort evidence at 10,000 Records. The HTTP
create/list/attach/namespace regression passes with selector/invalid-query
extensions. All 63 SDK client tests pass, including the loopback Rust fixture.
Strict all-target application/store/API Clippy and formatting/diff checks pass.
An initial browser fixture used its historical timestamp and correctly expired;
the regression now creates its session at the current test clock. No runtime
expiry behavior was changed.

Run the complete canonical PR gate on the next clean commit before extending the
shared diff. Workbench/host canonical-detail wiring and provider/local Search
composition are still required. Existing parallel maps cover that next step and
response-header policy; they are preparation, not activated contracts. M4 retains
v16/archive v6 and all shared surfaces. No M4 PR/merge, v17 allocation or C2
activation release has occurred. Codex Security remains prohibited.

### M4 exact selector verification and next workfront — 2026-09-05

The complete canonical PR gate passed on clean commit
`25c5bde0db0d3c16417bba73044ca6d4c259ac2e`, tree
`9979d8c9cfe42d4608fbd03a8d82492fb6e74a4c`. Both generated local receipts
identify that exact source: all 27 contract gates and 11 portable gates pass;
documentation verification passes. The earlier run at 4ed87c7d stopped because
the mutation sentinel still expected 384 problems; the added ListRecords
ValidationFailed case correctly makes 385. The assertion was updated, not removed.
The passing run includes 140 JavaScript tests and full workspace Rust/Clippy.
This is software verification, not deployed/live-provider or hardware evidence.

Compact parallel allocation continues within this checkpoint:

- Commander: sole shared-surface writer; next integrate host/Workbench exact
  Record routes, then local/provider Search transport and composition. Preserve
  committed outage correction and selector results. No M4 merge/release yet.
- Store agent: completed read-only detail-route map. Retain URL intent across
  profile-authority resets; clear loaded profile data. Keep selected native
  RecordSummary.grain for canonical routes rather than inferring it from display
  mediaKind. Direct selection must be independent of the bounded Library page,
  tracking permission and Library errors. Guard success/error/finally by route,
  generation and authority; refresh selected details after metadata changes.
- Contract agent: completed transport and bounded M5 preparation. Provider page
  acquisition must be POST with browser mutation proof/CSRF and no automatic
  retry because cache misses create immutable receipts. Snapshot/details remain
  GET/read-proof operations. Do not globally require mutation proof for all
  SearchMetadata operations. Add its existing bounded-JSON problem applicability
  with the actual POST binding. M5 exact tracking selection can reuse its own
  profile-state owner and existing key; it does not implement saved intent and
  has no allocated migration or public API yet.
- Local-search agent: completed cache-policy and fixture maps. HTTP private is
  not an account-data classification; the current pinned public-metadata routes
  remain actor/profile/grant partitioned. Do not turn no-store into zero-TTL disk
  writes. Response-policy enforcement remains open. Real-store HTTP fixtures can
  prove offline/cache-hit/CSRF behavior using a counting vault; existing runtime
  closure tests prove online orchestration separately. Those layers are not one
  successful online HTTP-to-provider-to-store integration proof. Do not expose
  private test helpers or weaken production TLS/egress to claim that proof.

Browser route verification must cover legacy and canonical reload, Record 501,
initial sign-in/profile switch/expiry, reversed responses, missing or malformed
routes, unavailable tracking, keyboard navigation and status announcements.
Wrong-grain canonicalization needs an explicit disposition before implementation;
slug is never identity. No new roadmap, framework, scope reduction or speculative
production interface is authorized by these read-only maps.

### M4 canonical Record detail integration — 2026-09-05

The existing exact Record selector is now wired through browser and Desktop
hosts into Workbench. Legacy links remain valid; canonical native-grain/title
segments replace stale presentation segments only after the authorized exact
read. No list-page scan or tracking permission is required to display identity.
URL intent survives sign-in/profile changes while private detail, metadata and
outgoing action feedback are discarded. Request generations and route lifetimes
fence late success/error/cleanup, including A→B→A navigation and metadata refresh.
Unknown tracking evidence is visibly unknown; confirmed tracking changes still
update the existing profile-state owner without implying saved intent. A read
started before a confirmed tracking mutation cannot overwrite that newer state.

Commander remains the only shared-surface writer. The store agent implemented
and released only the two allocated Desktop Rust files; its 12 focused tests and
default-feature Tauri check passed. The contract agent implemented and released
one new browser regression file. The independent review agent found the stale
metadata continuation and tracking-message risks; both are corrected, with no
remaining concrete finding in its final read-only review. UI/web typechecks and
the Tabler policy check passed on the integration diff.

Browser QA first reproduced the broken direct link, then verified the actual UI
using the existing bounded fixtures. The broader run caught a Settings metadata
policy refresh regression; retaining validated navigation identity restores the
authorized refresh without retaining private data. Old fixtures that ignored
the exact selector were corrected to the new contract; production singleton/ID
checks remain strict. The failed-tracking expectation now requires unknown, not
automatic tracking. A narrow dark-mode audit found active-tab contrast of 2.75;
the tab now uses the existing theme text token and a currentColor underline.
Manual screenshot review also caught shrinking/overlapping tab labels at 320px;
the existing native horizontal tab scroller now keeps each button's full width.
Keyboard focus also calls native nearest-alignment reveal: Chromium otherwise
left the focused final button clipped at the prior button's scroll position.

The local QA report and before/after screenshots are under
`.gstack/qa-reports/m4-record-detail.md`. The requested QA workflow was adapted:
no global artifact queue synchronization or memory writes; existing fixture
contract maintenance used native implementation checks rather than claiming the
skill's new-tests-only restriction was followed unchanged. No Codex Security
tools or skills were used. Browser fixtures do not establish live provider,
durable storage, authentication correctness, or complete accessibility conformance.

The clean committed integration still requires its exact canonical PR gate.
Final focused browser verification passed all 16 cases in 17.8 seconds, including
the final tracking-read and tab fixes. An earlier neighboring-suite run passed
70/70 before those last refinements; it is not exact-current 71-case evidence.
Provider-page POST/CSRF, candidate details/actions, local/provider Search
composition and provider response-cache policy remain the next M4 work. This is
not a reduction of scope or a completed M4 claim. M4 retains migration v16,
archive v6 and shared-file ownership; no PR, merge, v17 allocation or C2 activation
release has occurred. Rollback is the bounded host/UI/test commit; no stored data
or prior committed selector/core work needs to change.

Parallel preparation remains bounded. The store agent's response-policy map
identifies existing `send_json` → `ProviderSearchPage` → Search persistence and
`SearchReceiptLifetime` as owners for observation time, Age and shorter deadlines.
No new table is needed. `no-store` must never enter SQLite/WAL or stale fallback;
the current durable-only Page outcome cannot represent a live transient result.
This is a concrete internal outcome gap to resolve before claiming full provider
response-policy support, not permission to fabricate receipt IDs or cut live
Search scope. Just-observed but non-reusable responses and cached Save/refetch
policy also need explicit semantics; fixed deadlines alone cannot encode every
response directive. Detail/credential-check callers of `send_json` must retain
their existing behavior when the shared response envelope changes.

### M4 exact detail verification and bounded future preparation — 2026-09-05

Clean implementation commit `21fb4a9e24b4724341302bf208399a6dda2283a6`, tree
`70267f3c086196b335ae9ab9449b7aecd14612f8`, passed the complete canonical PR gate.
Both local receipts were checked against that exact commit/tree with dirty=false:
27/27 contract gates and 11/11 portable gates passed, plus documentation checks.
The subsequent five-file browser run on the same unchanged source passed 71/71
in 1.7 minutes, including all 16 new detail regressions and the existing Access,
shell, navigation and metadata suites. No live-provider, deployment, full-device
budget or general accessibility certification is implied by those results.

Read-only GitHub quality triage found no M4 PR and no checks for this unpublished
commit. Open code-scanning alerts were zero. PR #89's observed Codacy annotations
did not target this detail/host diff; the historical fake-success finding was
already fixed in source. Existing image-size patch/regressions remain present.
The glib advisory still affects both Desktop and benchmark locks. Its old
benchmark-only disposition was corrected in
`docs/reviews/2026-08-24-dependency-advisory-disposition.md`; no advisory was
dismissed, no ignore was added and no CI job was restarted. Native review only;
Codex Security remains prohibited.

The available agents also completed bounded future preparation, without writes:

- **M6 Discover:** current `discover-view.svelte` composes Search-on-submit,
  not governed rails. Approved preview capabilities are not active registry
  contracts. Reuse provider authority/transport and M4 identity/actions, but
  define real catalog admission instead of forging Search queries/receipts.
  Search's 120-second fresh/600-second stale caps cannot replace the approved
  catalog policy of 15-minute freshness, 15-minute refresh grace and 24-hour
  stale-on-error. M5 owns local smart queries; M7 owns native Collection bindings.
  Shared registry/generator/API/SDK/host/Workbench writes require explicit release.
- **M7 Collections:** existing `nuvio_collections.rs` owns profile-scoped catalog
  configuration, not native Record membership. Preserve its current commands,
  canonical storage, transactional scope checks and no-op identical replacement.
  Its duplicate-ID replacement, unsupported-source dropping and reconstructed
  catalogSources conflict with approved lossless preview/import semantics; settle
  historical compatibility before changing stored canonical reparse. Pack import
  requires preview digest, atomic apply/replay, rollback, lossless unknown fields
  and zero outbound access before activation. Existing Collection-name dialogs
  and empty Workbench bindings are not durable membership. M4/M5/M6 predecessor
  contracts and actual migration/archive handoffs govern implementation; historical
  plan archive-number guesses must not override the current v16/archive-v6 owner.

M4 remains the active writer. Next: the already mapped provider-page POST with
browser mutation proof, candidate details/actions, local/provider result
composition, and response-policy enforcement. The future maps allocate no new
API, table, ID, migration, archive version or production writer. No M4 PR/merge
or C2 activation handoff has occurred.

### 2026-09-05 — Provider Search page transport

Committed implementation `e5afb60817f5772998b014f443bbbda0849a5fe1`, tree
`7aca9d8ba9776b05cae79e27bf41141483697ed3`, preserves every prior M3/M4
commit. The canonical `cargo xtask test pr` gate passed on that clean commit,
using physical `TMPDIR=/mnt/secondary-ssd/cache/home/tmp` and
`PKG_CONFIG=/usr/bin/pkg-config`. The follow-on verification inventory includes
the new Search SDK suite in the canonical gate, not just a focused manual run.
No new dependency, external AI key or Codex Security tool was used.

- **Implemented:** POST `/api/v1/search/providers/{provider_id}`, strict DTOs,
  generated OpenAPI/SDK contracts and `searchProviderPage`, daemon composition
  through existing provider runtime and shared operation gates. Query input stays
  in the body. Browser page acquisition requires mutation proof even offline;
  durable current authority is checked before malformed body/path disclosure.
  Generic/remote routes remain bearer-only. Health/integration routers have no
  Search route. Successful and error responses are private/no-store.
- **Authority:** new enrollment/recovery receives `MetadataSearch` through the
  existing full-owner scope set. Only the exact consumed, active node-owner grant
  receives the unmerged v16 backfill; C1 links that same grant. Delegated grants,
  subject/session epochs and ordinary reopen behavior remain unchanged. Published
  v1–v15 migrations and archive v1–v5 contracts are unchanged. M4 retains v16 and
  archive v6; older disposable unmerged-v16 databases do not rerun this migration.
- **Evidence:** seven real SQLite HTTP regressions cover offline actor-partitioned
  cache hits/misses, zero vault calls, typed input failures, direct/generic/remote
  boundaries, cancellation and authority revocation while waiting for the provider
  gate. Thirty-two SDK checks cover CSRF, no retries, request/response binding,
  caller mutation races, strict unions and bounded large responses. Five migration
  and three authorization regressions cover scope isolation, rollback and activity.
  Focused store Search, broader Rust suites, strict clippy and docs checks passed.
  These fixtures do not claim successful live-provider HTTP/network evidence or
  new browser UI behavior. Prior 71-check canonical-detail browser evidence stays
  attached to its original exact source.
- **Native review:** resolved missing success cache headers, incorrect 501/507
  documentation, schema-binding and typed-path issues, plus an SDK race where a
  caller-mutated request page could change response validation. No remaining
  actionable finding was reported for the reviewed slice. All subagent production
  writes were prohibited; assigned test leaves were non-overlapping and integrated
  by the commander.

Parallel preparation remains bounded in this checkpoint. Candidate GET must use
Search read proof, not the page-mutation extractor. Atomic action POST must preserve
IdentityWrite authorization followed by durable replay before requiring Search
for a new save; otherwise replay after ephemeral expiry would break. Reuse existing
receipt/runtime owners and capture immutable response-binding primitives. M8's
smallest proven locale-to-English acquisition gap touches the active provider and
metadata response/provenance owners; no unused fallback helper or competing writer
was allocated. Existing TMDB attribution is already implemented and was not invented
as a gap. C2 was told that the scope prerequisite prevents a coherent early release.

Next active work: candidate details/actions, local Search transport and Workbench
multi-source composition, then the already mapped response-cache policy enforcement.
These remain in-scope implementation work, not removed or deferred capabilities.
No M4 PR, merge, v17 allocation or shared-file release has occurred.

### 2026-09-05 — Candidate detail and atomic Record action transport

This slice follows clean commit `217ced209f43bdcc603b99f1377ba9cc67b094f1`
(tree `412a22d6eeeed16bba8e85a9552b8929f75e3a9c`), whose canonical PR gate and
both exact-head, clean-tree verification receipts passed. All earlier M3/M4
commits remain preserved. No migration, archive version, dependency, provider
owner, external AI key or Codex Security invocation was added by this slice.

- **Implementation:** candidate GET with explicit offline mode, atomic candidate
  action POST, strict public DTOs, generated OpenAPI and SDK operations
  `readSearchCandidate` and `saveSearchCandidate`. These reuse existing Search
  receipt, provider-details and atomic identity/metadata transaction owners.
  They are mounted through the existing Search router and provider operation
  gates. No Library, progress or watched-state mutation is implied.
- **Authority:** GET uses current Search read proof. POST uses current
  IdentityWrite before parsing malformed input. The existing atomic owner retains
  IdentityWrite → exact durable replay → Search for a new save. Browser POST
  requires CSRF; browser GET does not. Receipt reads retain original lifetime and
  actor/profile partition. Refetch failure never becomes an implicit cached save.
- **Public evidence:** DTOs omit internal actors, grants, configuration and query
  digests. Refetched fields remain separate from the original snapshot. Action
  receipts retain historical timestamps and initial status on replay. SDK retries
  keep one operation ID and identical serialized body; immutable primitive
  bindings reject mismatched action, target, disposition, source, receipt and mode.
  No current-freshness decision is fabricated in the SDK.
- **Focused evidence before the exact-head gate:** 15 real SQLite/router Search
  tests, 8 store authorization tests, 28 provider-runtime Search checks, 27 detail
  SDK checks and 33 action SDK checks passed. The broader SDK/generated-contract
  subset passed 135 checks before the final action leaf was added. Contract
  strict-shape tests, docs validation, generated artifacts and strict clippy passed.
  Both new SDK leaves are included in the canonical gate's explicit inventory.
  The new exact-head canonical gate is required after this slice is committed;
  the prior commit's receipt must not be presented as this slice's proof.
- **Native findings resolved:** serde's internally tagged unit Create accepted an
  extra Record ID; an empty struct variant now rejects it before mutation. Missing
  detail outcomes use the same strict shape. The shared SDK path validator now
  rejects terminal-newline matches and non-string coercion. Fresh action evidence
  must contain an expiry string, not omitted/undefined or null expiry. Tests retain
  legitimately expired historical receipts without recalculating their status.
- **Parallel allocation:** commander retained all shared production writes.
  Store agent owned only the authorization regression leaf, then reviewed the
  transport read-only. HTTP agent owned only the existing Search HTTP test leaf.
  Contract agent owned only the two new SDK regression leaves. All test ownership
  is released. Existing next-slice and M8 preparation maps remain authoritative
  preparation, not permission for overlapping production writes.

These HTTP tests prove real local receipt reads and atomic cached saves, not a
successful live-provider refetch, rendered Search UI or packaged-host behavior.
This is headless transport work; prior browser accessibility evidence retains its
original source identity. Workbench and local/provider composition remain next
active work, followed by the mapped response-cache policy and full programme
scope. Rollback this transport with the matching SDK; no database downgrade or
receipt deletion is needed for this additive slice. M4 still owns v16/archive v6
and all shared integration files; no PR, merge, v17 or C2 activation release.

### 2026-09-05 — Durable local Search transport and complete Record paging

Preserved base `6339775c762e8a1e4893ba45124f669de02efc9c`, tree
`ac38e93652e67845a107390b5101509d87a4307c`, passed the canonical PR gate on
that exact clean source: contract receipt 27/27 and portable receipt 11/11.
Those receipts close the preceding candidate-transport checkpoint, not this diff.

- **Implementation:** `POST /api/v1/search/records` and SDK `searchRecords` reuse
  the durable local index, current Search authority, existing Record summary
  projection, metadata resolver and identifier owner. The body holds the query;
  no provider operation, gate, DNS or credential vault is needed. Browser POST
  is a read without CSRF mutation proof; generic listeners remain bearer-only.
- **Completeness and bounds:** pages contain at most 100 complete Records.
  OpenAPI and generated SDK publish the application-owned 4 MiB response cap.
  Metadata/title matching precedes identifier hydration. SQLite streams the
  selected Record index without sorting unbounded identifier payloads. Escaped
  strings and identifier syntax are charged before copying; only completely
  admitted identifiers are sorted and exposed. Fixed headroom reserves 2,048
  bytes per Record and 1,024 for the envelope. The final serializer is bounded.
  A deferred Record resumes after the last complete match, including on an
  otherwise final page. A single over-capacity Record fails without skipping it
  or deleting evidence. This is a response/hydration bound, not a claim of whole
  process memory bounds or newly bounded historical-activity SQL work.
- **Native findings resolved:** the initial six-times escape estimate rejected
  a valid roughly 763 KiB ASCII-identifier Record. Exact escaped-string sizing
  now admits it. Cold SELECT preparation checks flush the statement cache so
  cached preparation does not distort the comparison; they do not claim to count
  executed statements. Current authority precedes malformed body responses.
  SDK cursor/grain bindings are captured before awaits and safe retries preserve
  identical bytes. First-page cursor authority remains server-owned.
- **Focused evidence:** 8 store-bound regressions passed, including 2,500-identifier
  admission, two complete 8,000-identifier Records across pages, 16,000-identifier
  capacity failure, nonmatching oversized evidence, JSON escaping equality and
  indexed query-plan evidence. Six real-router local Search checks passed with
  605 Records, empty continuation, profile rotation, scope revocation and no
  provider/vault access. The local SDK leaf passed 29 checks; the broader selected
  SDK/contracts run passed 105 before the final constant-equality check was added.
  The whole store library passed 388 tests with 5 explicit ignored worker/perf
  tests. Strict clippy and documentation validation passed. The release-only
  10,000-Record fixture passed 100 measured samples: p50 1.670 ms, p95 1.742 ms,
  max 1.857 ms. These are local synthetic-corpus measurements, not device or
  live-provider latency claims. A new clean exact-head canonical gate is required
  after commit; prior receipts cannot substitute for it.
- **Parallel allocation:** commander retained all shared production writes.
  Store, HTTP and SDK agents each owned only their named new regression leaf;
  all leaves are released. Independent read-only review validated the size fix,
  cursor behavior and authority ordering. The HTTP agent now prepares the bounded
  host/Workbench handoff read-only; existing M5/M8 and later-lane maps remain
  preparation only. No competing writer, new roadmap or speculative API was added.

Ponytail reused the existing owners, stdlib and installed serializer; no dependency,
migration or archive change was needed. Codex Security remains prohibited and was
not invoked. No new browser UI, packaged-host or successful live-provider proof is
claimed by this headless slice. Next work remains Workbench local/provider composition,
host bindings and governed response-cache policy, followed by the full approved
programme. Roll back this additive route with its SDK binding; do not downgrade the
database or delete Search evidence. M4 retains v16/archive v6 and shared ownership;
no PR, merge, v17 allocation or C2 activation release has occurred.

### 2026-09-05 — Shared provider response-policy observation

The preceding local Search commit `bad6f35025883f017900f75a6607c2550e19d588`,
tree `60234bc062aa1b4a82540c80b07cbf9396a79142`, passed the canonical PR gate
on that exact clean source: 27/27 contract and 11/11 portable gates, including
262 SDK checks. Its exact-commit release 10,000-Record rerun passed 100 samples:
p50 1.753 ms, p95 1.796 ms, max 1.817 ms. These are local synthetic measurements,
not hardware, live-provider or rendered Search evidence.

- **Current implementation:** the existing governed JSON boundary retains one
  normalized response policy and header-receipt time. Page and detail parsers
  share that observation; empty and filtered pages cannot lose restrictions.
  The application computes absolute purpose-capped deadlines. HTTP syntax stays
  in the runtime adapter. Missing freshness differs from explicit zero, and
  no-store has no storage deadline rather than a fabricated zero-TTL admission.
- **Source and native review:** [RFC 9111](https://www.rfc-editor.org/rfc/rfc9111.html)
  owns cache syntax, age and variant matching;
  [RFC 5861](https://www.rfc-editor.org/rfc/rfc5861.html) owns stale-if-error.
  Fasti conservatively treats malformed overall Cache-Control as live-only,
  duplicate/invalid freshness as validation-required and invalid stale grace as
  zero. It does not retain request-header variants, so nonempty Vary requires
  validation. Independent review found and fixed oversized valid Age being
  treated as zero and ignored Vary wildcard matching. First Age list members
  remain authoritative even when a later member is long; overflow saturates.
  Numeric work beyond the explicit limit requires validation instead of granting
  freshness. No raw policy headers enter public candidate serialization.
- **Focused evidence:** nine application policy tests and all 83 provider-runtime
  tests pass. The latter include 22 parser tests and five real loopback HTTP
  boundary tests for body timing, populated/empty/filtered pages, detail parsing,
  public JSON, status classification, truncation and the exact 2 MB body limit.
  Strict application/runtime all-target clippy passes. The loopback fixture does
  not bypass or prove governed TLS, DNS or live-provider behavior. A clean
  exact-head canonical gate is still required for this new diff.
- **Implementation boundary:** policy capture is not admission/reuse enforcement.
  Search persistence, offline details, cached and refetched actions, metadata
  refresh, existing Desktop track/apply callers and selected metadata projections
  still need the same restriction checks. A no-store body must not enter SQLite
  or WAL through claims, refresh receipts or a cached-save conversion. Do not
  call this complete, silently grant an explicit-Save exception, fabricate a
  durable receipt, or replace live results with an unavailable placeholder.
- **Bounded parallel allocation:** commander remains the sole shared production
  writer. The three agents wrote only their allocated application policy,
  runtime parser and HTTP-boundary test leaves; all leaves are released. Their
  read-only continuation identified the existing context_json envelope as a
  no-schema policy carrier: keep provider/page at the root, the query context
  digest unchanged, strict canonical decoding and the combined 2,048-byte bound.
  Lookup must select by the existing partition and then validate the envelope;
  exact old context_json equality would miss every new page. Newest restrictive
  evidence must not fall back to older permissive rows. Online coordinates and
  offline payload permission must be separate, including failure snapshots.
  Existing `ProviderSelectionInput` can drive a governed fresh fetch without a
  payload-retaining transient-handle service. It does not prove prior membership
  in a scoped Search receipt. Coordinate-origin Create/Attach and no-store Save
  therefore require an explicit action/history disposition before implementation;
  do not overload existing receipt or provenance fields. M5's updated preparation
  reuses the committed canonical detail owner, complete-Record hydration pattern
  and unknown-versus-absent tracking distinction. It does not reuse Search's
  query cursor or authorization for Library. Exact tracking selection, full
  Library pagination and saved intent remain in scope and unimplemented.

Ponytail reuses the existing response, provider, lifetime and JSON owners plus
the already locked `httpdate` 1.0.3 package; only its direct runtime dependency
edge was added. No new cache service, schema, archive, public API or UI was
introduced. Next, integrate policy-aware admission/reuse and truthful live-only
Search before freezing host/Workbench result shapes. Preserve completed action
history and published migrations. No Codex Security was used. This headless slice
adds no visual/accessibility or packaged-host claim. Rollback is the matching
application/runtime observation diff; no stored data needs deletion or downgrade.
M4 remains local and unmerged, owns v16/archive v6 and all shared integration
surfaces, and has not released v17 or C2 activation. Full programme scope remains.

### 2026-09-05 — Provider-page admission, reuse and live-only results

Preserved base `57f524181ccfeacae49e8479294178c3ade4e225`, tree
`739cf0dcba5189918ce1c800c539cb54c24cfb20`, passed the complete canonical
gate on that exact clean source: 27/27 contract and 11/11 portable checks.
The new implementation below requires its own clean exact-head gate after commit.

- **Page policy is now consumed:** the existing 2,048-byte context JSON envelope
  carries normalized policy alongside unchanged query coordinates. Context and
  partition digests remain stable; published v16 SQL provider/page guards remain
  unchanged. Strict canonical decoding rejects duplicate, unknown and malformed
  evidence. Legacy absence grants no reuse. Lookup validates the newest row in
  the current authorized partition, not an older permissive fallback.
- **Time and admission:** derive all deadlines from the original nanosecond
  observation, then independently canonicalize them to SQLite microseconds.
  Delayed body/queue time does not renew freshness. Compare row lifetimes with
  the policy-derived projection; mismatch is an integrity failure. A newly
  observed persisted response with no remaining fresh interval is Observed,
  never relabeled Fresh. Future observations and expired receipt admission fail.
  Successful cache misses still commit authorized browser activity.
- **No-store pages remain real results:** runtime returns Live with normalized
  candidates and continuation, without invented receipts, lifetimes or sequence.
  The application page validator is shared by live and stored paths for bounds,
  coordinates, grain and advancing continuation. The store rejects payload
  admission. A separate existing-port operation reauthorizes and discards only
  older ephemeral pages/candidates in the same partition before Live is returned.
  It does not create a marker, claim, Record, durable receipt, table or service.
  Revocation or a changed prepared partition prevents deletion and Live output.
- **Contracts and SDK:** the existing provider-page response gains Live and the
  cache-state enum gains Observed. OpenAPI/SDK are regenerated from the existing
  owner. Source/page binding covers both candidate forms; submitted offline mode
  is captured before awaits. Offline Live/Observed and Observed with an upstream
  error are rejected. Optional upstream-problem omission remains valid. POST
  mutation proof, CSRF, byte bounds, cancellation and retry-never are unchanged.
- **Focused evidence:** six application envelope tests, 13 new store policy and
  purge checks, eight runtime policy checks and 47 provider SDK tests passed.
  The full store library passed 401 tests with five explicit ignored workers/perf
  checks before the final shared-validator extraction; the latest runtime suite
  passed 91, Search-related API checks passed 22 and strict all-target clippy
  passed. Native review caught the optional-field mismatch and live admission
  validation gap; both have regression coverage. The route-context tamper test
  now expects an integrity failure from page lookup rather than treating corrupt
  stored coordinates as a cache miss. No test was removed or weakened.
  Two additional actual-purge regressions passed: browser read/wrong-CSRF
  rejection and activity commit/rollback, plus populated Record/claim/action
  preservation and exact completed replay after purge and Search-scope removal.
- **Parallel allocation:** commander alone changed shared application/store,
  runtime, contracts, generator, API and SDK owners. Agents wrote only their
  named envelope, store-policy, runtime-policy and provider SDK test leaves, then
  released them. Independent reviewers checked deletion boundaries and public
  response binding; next candidate-policy preparation remains read-only. Existing
  M5/M8 and later-lane preparation stays intact, with no second roadmap.

This proves page operations, not complete provider-policy coverage. Candidate
offline reads, cached/refetched actions, M2 refresh, legacy Desktop conversion
and selected metadata projection remain the next required shared-policy work.
The existing candidate reader still separates 24-hour retention from page reuse;
that retention must not be mistaken for permission to expose restricted payloads.
Live candidate routing/action origin and no-store user-save disposition must be
resolved without fabricating historical receipt provenance or cutting the scope.
Workbench composition and successful governed live-provider/packaged-host proof
remain required. Headless fixtures establish no new visual or accessibility claim.

Ponytail reused current context, lifetime, validation, transaction and provider
owners. No migration, archive version or dependency was added. Rollback must keep
policy-aware cache reading: an older binary does not understand these new ephemeral
envelopes. Use a forward correction or remove only disposable Search cache rows
through an authorized recovery path; do not downgrade the schema, rewrite durable
action history or delete Records. No Codex Security was used. M4 remains local and
unmerged with v16/archive v6 and shared ownership; no v17 or C2 release is granted.

### 2026-09-05 — Exact provider-page verification and bounded Nuvio preparation

Commit `81c2bc22f5f5b324d05603cf822bf419df8d277e`, tree
`d98f32406328546bc4fda40ff2fa20664b232a46`, passed the canonical PR gate.
Both machine receipts were checked against that exact clean commit and tree:
27/27 contract checks and 11/11 portable checks, every gate passing with exit 0.
This includes workspace tests, strict clippy, generated contract consistency and
277 canonical SDK checks. This checkpoint update is later documentation, not part
of that tested tree. It does not extend the receipt to untested changes or prove
live-provider, Workbench, packaged-host or accessibility behavior.

Read-only M11 preparation compared the existing Scrob pin with upstream
`ce85114902390dbab47e42859ef1586997739a9f` (16 later commits). The
[exact comparison](https://github.com/ellite/scrob/compare/1c4d775b70f489ca0531376b2c3de6a8c3de2a2b...ce85114902390dbab47e42859ef1586997739a9f)
adds two bounded verification cases to the existing M11 workfront:

- A queued Nuvio job cancelled before admission must not transition back to
  running. Reuse planned admission and generation fencing; do not create a
  second job owner.
- Series-level watched rows without season/episode coordinates represent rollups
  in Scrob's importer, not extra episode consumption evidence. Preserve distinct
  summary and episode fixtures. This is observed Scrob behavior, not an
  independently established Nuvio contract.

The [adapter](https://github.com/ellite/scrob/blob/ce85114902390dbab47e42859ef1586997739a9f/backend/core/nuvio.py)
and [README](https://github.com/ellite/scrob/blob/ce85114902390dbab47e42859ef1586997739a9f/README.md)
are unchanged from the pin. There is no new authentication-contract disposition.
Scrob demonstrates snapshot pulls and Library read/merge/replacement; it does not
alone establish every delta/upsert RPC in the approved programme. Those require
the separately pinned official Nuvio evidence. Its ratings exclusion and
TMDB-centric matching do not narrow Fasti's approved scope. Keep the existing
baseline pin; this comparison supplements rather than silently replaces it.

Search candidates, viewed details and Create/Attach receipts are not Library,
progress or watched intent and cannot independently enqueue remote writes or
override response restrictions. No M11 implementation or shared-file ownership
is released by this preparation. All three bounded review agents have released
their test leaves; the commander retains shared integration ownership. The next
active implementation remains candidate policy retention, payload disclosure and
cached-action checks through existing owners, followed by the remaining metadata
and host/Workbench paths recorded above. No second roadmap, scope reduction or
speculative production surface was introduced. Codex Security remains prohibited.

### 2026-09-05 — Retained candidate policy and snapshot-free details

The preceding turn made progress: it verified the exact clean `81c2bc22` code
tree and committed its checkpoint as `7a6c79bce1147c7568b18965bce628ae88601c5a`.
This new implementation remains a separate, unmerged M4 increment and requires
its own clean exact-head canonical receipt after commit.

- **Existing owner:** `StoredSearchCandidate` retains the validated normalized
  response policy instead of dropping it after page decoding. Its one
  `payload_is_reusable(at)` rule separates original observation, exclusive
  freshness and 24-hour retention. No-store/no-cache never permit retained
  payload reuse; must-revalidate requires remaining freshness. Permitted explicit
  historical evidence is not reduced to the 600-second page fallback window.
- **Read versus locator:** public offline reads apply that rule, while internal
  authorized preparation can retain coordinates for a real online fetch. The
  runtime checks the same rule before disclosure, including after I/O. It still
  reauthorizes and verifies the exact fetched identifier against the receipt.
  Successful refetch does not validate or renew the old snapshot.
- **Truthful results:** two strict outcomes, `refetched_without_snapshot` and
  `unavailable_without_snapshot`, carry the validated receipt/provider/grain
  locator without old payload, lifetime or digest. The former contains only new
  normalized details and effective locale; the latter only a source problem.
  Existing four shapes remain unchanged. The runtime returns its already-checked
  `SearchCandidate`, and the API uses the existing DTO mapper rather than parsing
  the provider object again. SDK binding uses captured locator/mode and rejects
  both new outcomes offline. Old strict clients reject unfamiliar outcomes.
- **Atomic Save:** cached preparation checks eligibility, and commit repeats
  that preparation in its existing transaction before writing. Completed replay
  still checks current IdentityWrite before returning durable history and does
  not require Search permission, live cache policy or an ephemeral receipt.
  `metadata_fields()` remains a pure historical projection, not save authority.
  Its stale/zero-freshness representation cannot bypass action admission.
- **Focused evidence:** eight application projection/permission checks, five
  new actual-store policy tests, the full store library (408 passed, five
  explicitly ignored), all 95 provider-runtime tests, 42 candidate-details SDK
  checks and 24 Search-related API checks passed. The API checks include both
  browser and credential actors through real restricted offline read/Save
  routes, plus exact JSON/header assertions through the production projection
  for both snapshot-free outcomes. A short actual-deadline test keeps SQLite
  evidence unchanged while proving pending commit denial and completed replay
  after expiry. Strict all-target clippy passed before the final projection
  extraction; the canonical gate must cover that final code too.
- **Native review:** independent current-slice store/time/replay and
  runtime/contract/SDK reviews found no remaining concrete defect. The review
  checklist prompted direct API-projection proof; it is now present. This is
  not a completed whole-branch pre-landing review: M4 still has the required
  metadata, live-action, host/Workbench and delivery work below. GitHub confirms
  no PR exists for this branch. No Codex Security or packaged authentication
  investigation ran.

The commander remains the sole shared production writer. Agents edited only
the allocated store-policy, runtime-details and SDK-details test leaves and have
released them. Read-only next-slice preparation identified the existing
`metadata_claims` registry as a possible immutable per-claim policy carrier; it
is not yet a migration decision or implemented schema. Ephemeral cache rows
cannot own that restriction because restore intentionally omits them. Admission
must precede the first payload INSERT, not merely roll it back later. Both
single-record projection and batched Record summaries need identical filtering;
excluding a restricted newest claim must not resurrect older same-source data
or erase user overrides. Pure claim freshness cannot encode no-cache permission.

Before that next write, resolve schema/archive disposition explicitly: changing
`migrate_v16` alone does not upgrade an already-version-16 local database.
Preserve published migrations and archive v1-v5 canonical bytes. A nullable new
archive field must not silently add `null` when reserializing old exact rows.
Preserve legacy claims and overrides while recording what unknown response policy
can authorize; absence cannot be called upstream permission. No new migration
number, table, provenance meaning, API or archive format was allocated by the
read-only preparation. No-store explicit Save and coordinate-origin live actions
still require truthful intent/history semantics, never fabricated receipt
provenance or an implicit exception to the recorded no-payload-persistence rule.

Refetched Save, M2 refresh, selected-field reuse and Desktop track/apply/artwork
ordering remain required active work. Existing Desktop side effects occur before
metadata conversion, so a conversion-only guard is insufficient. Workbench,
successful governed live-provider proof and the remaining full programme are
unchanged in scope. This headless slice does not prove rendered accessibility or
packaged-host behavior. It adds no dependency, schema or archive change. Rollback
must retain restriction-aware readers/actions; prefer forward correction and do
not erase user Records or durable action history. M4 retains v16/archive v6 and
shared integration ownership; no v17 or C2 activation release is granted.

### 2026-09-05 — Candidate exact gate and explicit v17/v7 allocation

Candidate-policy commit `e675fde2e23451ac356a63825c4c0d90e695bbdb`, tree
`3315db74d81142129b4100b52304c913020356f6`, passed the complete canonical PR
gate. Both receipts were machine-checked against that exact clean source:
27/27 contract and 11/11 portable checks, each passing with exit 0. The canonical
SDK inventory passed 292 checks. This later checkpoint documentation is not part
of that tested tree. No old receipt is transferred to a newer source.

Access explicitly confirmed no competing owner and allocated **append-only
migration v17 and archive v7 to M4**. Preserve v16/v6 and all earlier published
bytes. The Access programme's next migration is **v18**, conditional on M4's
exact merged commit/tree and explicit shared-file handoff. Access remains
read-only. This supersedes only the previous v16-only allocation, not any
programme gate, scope or preservation requirement. No schema/archive write has
yet occurred for this allocation; M4 is still local, unmerged and without a PR.

Next-writer preparation is complete and read-only:

- **Application/runtime:** carry required response-wide policy in existing
  provider commit commands; do not duplicate it per field or overload provenance.
  Empty responses and the existing separate rating batch need the same gate.
  Pair refetched Search fields with their actual policy; Cached mode derives it
  from the reauthorized receipt. Preserve original Search expiry and observation.
  Validate the entire batch after current authorization/completed replay but
  before the first namespace, Record, identifier, claim or receipt payload write.
- **Store:** the existing immutable `metadata_claims` registry can carry bounded
  nullable policy JSON without a new table. Validate new provider admission
  before `write_field_claim_inner` inserts the payload, not only before its
  later registry INSERT. NULL remains explicit unknown history, not invented
  upstream permission. Preserve user overrides and define same-source
  supersession before filtering so a denied newest claim cannot expose an older
  permissive response. Single and batched Record projections must agree.
- **Migration/archive:** append v17 and pin a genuine v16 schema fingerprint.
  Keep the existing 35-entity v6 list frozen; v7 adds no entity. Carry archive
  version through the existing stream selection, row decoder and post-import
  re-export verifier. Formats through v6 retain the five-column MetadataClaims
  row; v7 carries policy. Merely accepting absent input is insufficient: emitting
  a new null during exact old-stream verification would break compatibility.
  Prove populated v16-to-v17 upgrade and real v6-to-v17 restore with unchanged
  Records, claims, Search action history and revisions; reject crossed versions,
  forged fingerprints and malformed policy. Policy-bearing rows must be checked
  in the existing bounded preflight if rejection is to precede staged payload
  admission: field streams arrive before the claim registry during import, and
  restore staging uses DELETE journaling rather than WAL.

No new adapter, queue, cache service or roadmap is introduced by this preparation.
M5 remains a read-only next lane; it cannot reuse Search receipt actions as
Library intent. The commander remains the sole integration writer. Codex Security
remains prohibited, and no packaged authentication investigation is in scope.

### 2026-09-05 — v17/archive v7 carrier and exact verification

Committed carrier implementation `9012d8fa2191920b1e41512b122ab567443e7abd`
and its generator-format ownership correction
`48e90c020af8ce3acd16b98a47dd803b4dfe1b7c`, tree
`8c5e50320ba45c7b785c5a873347aa4a7dad2178`. The latter exact clean head passed
`TMPDIR=/mnt/secondary-ssd/cache/home/tmp PKG_CONFIG=/usr/bin/pkg-config cargo xtask test pr`.
Both receipts were machine-checked for source commit/tree, dirty=false, exact
gate count, and every pass/exit 0: 27 contract and 11 portable gates. Its workspace
test output includes 419 store tests passed and 5 expected subprocess helpers
ignored; the JavaScript contract/SDK inventory passed 297 checks. This later
checkpoint is documentation, not part of that tested tree. The first carrier
gate stopped on formatting; the correction extends the existing generated-file
ownership exclusion to v7 and formats the authored validator. No gate was weakened.

The allocated append-only v17 adds bounded nullable `response_policy_json` to
the existing immutable `metadata_claims` registry. SQL rejects no-store and
malformed coarse shapes; shared application canonical decoding rejects duplicate,
unknown, omitted and normalized representations without conflating valid
observation syntax with storage permission. NULL remains historical unknown.
No table, service, dependency, capability or public transport was added.

Archive v7 retains the frozen 35-entity v6 inventory. The current registry row
carries policy; old formats retain their exact five-column decoder and SELECT
for post-import descriptor verification. Migration v1–v16 function bytes and
checked-in archive v1–v6 artifacts were compared and remain unchanged. The
genuine v16 schema fingerprint is
`sha256:d7ae3b1ab15c0223245d1a9008833049e58e9ec882a6e1ba70a2a080fa3fd7a6`.
Populated real-v16/v6 restore into v17 and v7 NULL/valid-policy roundtrips compare
all 35 stream descriptors and bytes, preserving claims, Search/refresh receipts,
overrides and profile policy. Hostile field and rating policy cases prove
preflight rejection before named staging or SQLite creation. Native review found
and closed crossed-version preflight acceptance by retaining independent bounded
legacy/v7 row issues until the final manifest selects the version.

This is the durable carrier, not completed metadata policy enforcement. The
commander retains all shared production ownership. The next bounded native
workfront is prepared, without production edits:

- **Restore input hardening:** caller-owned `ReadSeek` bytes can still change
  between passes. Existing digest checks reject changed imports, but cannot prove
  zero earlier staged payload writes. Reuse archive.rs's private unnamed
  `O_TMPFILE` primitive for one bounded input capture and a non-Clone private
  file/preflight carrier. Both passes must consume that inode, never reread the
  original. Preserve original-source rewind, cancellation and recovery behavior.
  Count compressed capture plus database/blob/cleanup against total scratch;
  after capture, check remaining free space only for still-unwritten bytes.
  Prove mutating-reader/file isolation, limit+1, failures, cancellation and
  descriptor cleanup before claiming immutable-input admission. No extra importer
  or named recovery artifact is needed.
- **Whole-response writers:** carry required policy through existing Create,
  Apply and refresh commands; pair refetched Search fields with detail-response
  policy and retain current receipt policy for Cached mode. Fix the existing
  `ProviderCandidate::metadata_fields` conversion-time freshness renewal first.
  Validate the whole batch after authorization/completed replay but before the
  first namespace, Record, identifier or payload INSERT. Encode once per response,
  insert with immutable registry rows, and include policy in identical field and
  rating claim comparisons. No-store must not rely on transaction rollback.
- **Selected-field readers:** single/batch field loaders, rating loaders and
  cached-refresh raw claim views must consume registry policy. Validate all
  selected evidence before overrides can hide corruption. Suppress older claims
  in the same provider/namespace/source-ID/locale/region variant before applying
  eligibility, so a restricted newest observation cannot revive older payloads.
  Preserve other-source fallback, user overrides, original Search expiry and
  constant-query selected-ID batching. Raw Search grams remain candidate indexes,
  never permission to disclose a hidden title.

Response-wide writer/reader integration, immutable archive capture, real Save and
refresh, Desktop ordering, Workbench, governed live-provider proof and programme
delivery remain required active work. Headless tests do not prove rendered or
packaged-host accessibility. Rollback is forward correction; retain claims and
policy-aware archive readers rather than downgrade v17 or erase user data.
There is no push, PR, merge or shared-file release. M4 still owns v17/archive v7;
Access v18 remains conditional on an exact merged commit/tree and explicit handoff.

### 2026-09-05 — Private restore capture closes between-pass substitution

The commander reused the existing descriptor-relative Linux `O_TMPFILE` owner
for one bounded input capture. Verification and import now consume a private,
non-Clone file/preflight carrier. The caller's source is rewound but never read
again after capture. No named capture, recovery artifact, importer, dependency,
migration, archive format or public API was added. This protects against caller
source mutation; it does not claim protection from arbitrary same-privilege host
tampering. Malformed/no-store claim policy still fails before named staging and
SQLite creation. The compressed input itself is temporarily captured first.

Checked scratch admission includes compressed capture, database allowance,
declared blobs and cleanup reserve. Remaining free-space admission does not
charge already-written capture twice. Copy reads/writes remain at most 256 KiB;
an exact-limit archive requires a one-byte EOF probe. Private file destruction
removes capture on errors and process death. Native review found and fixed
cancellation precedence during both initial and final caller-source rewinds.

Pre-commit checks: nine focused capture/capacity tests passed; their isolated
write-failure helper runs through its parent test and is otherwise ignored. The
real kernel file-size limit produces an anonymous-file write error, with rewind
and no named residue. Tests also cover correctly rehashed source replacement,
external file overwrite/truncation, short reads, premature EOF, malformed read
counts, limit+1, arithmetic overflow and exact scratch admission. The existing
SIGKILL/retry matrix passes with capture-created/capture-written points. Before
the final rewind regression additions, the full store run passed 426 unit tests
(5 expected helpers ignored) and 3 integration tests. Formatting and diff checks
pass. These are working-tree checks, not an exact-head canonical receipt.

The bounded workfront remains in the existing programme: commander integrates
and verifies this slice; the native reviewer prepares selected-field policy
eligibility; the constructor reviewer maps whole-response policy threading; the
store reviewer audits database allocation enforcement. All agents' production
access remains read-only; only the two assigned capture test leaves were edited.
No Codex Security was used. Whole-response metadata admission/readers, real Save,
refresh, Workbench, provider/host evidence and delivery remain active scope.

Next resource hardening uses SQLite's existing connection-local page limit
before migration to enforce the main-database allowance. Current admission does
not enforce peak aggregate journal/temp-file usage; a database page cap alone
will not establish that stronger claim. Keep rollback journaling and integrity
checks intact. Rollback remains forward correction; preserve v17/v7 and all
earlier formats. No push, PR, merge, migration allocation or shared-file release
has occurred.

### 2026-09-05 — Exact capture gate and database allocation guard

Private capture commit `db4a9d7b5a82d47c8586d82e4369e3d0b769d8ee`, tree
`46c1d0dafd10a9ce9f91ba125bb9a96030c1ccbb`, passed the canonical PR command
with physical SSD TMPDIR. Both receipts were machine-checked against that exact
clean commit/tree: all 27 contract and 11 portable gates passed with exit 0.
Those receipts apply to capture, not to the following allocation-guard changes.

The next bounded correction enforces the main-database allowance before schema
creation through the existing SQLite connection. It reads actual page size,
rounds the byte budget down to pages, sets and checks `max_page_count`, and
rejects zero or ineffective limits. The same connection owns migration, import,
repairs, Search index rebuild and commit. Existing DELETE/FULL journaling,
integrity checks, final file-size verification and staged cleanup remain intact.
At the existing import outcome boundary, SQLite FULL from direct operations or
row insertion becomes a capacity failure, not a corruption claim. Cancellation
still takes precedence and cleanup failures remain explicit.

This uses [SQLite's page-limit primitive](https://www.sqlite.org/pragma.html#pragma_max_page_count),
not a new allocator, VFS, dependency or migration. Journals and temporary files
remain [separate resources](https://www.sqlite.org/tempfiles.html); neither the
database limit nor scratch admission claims an aggregate filesystem quota.
Native exact-diff review found no remaining defect. Six focused capacity tests
pass, including real allocation failure, subpage/rounded limits, ineffective
lowering, a populated 258-Record archive, cleanup and successful retry. Existing
restore tests also pass, including old formats and SIGKILL recovery. These
pre-commit results are not a new exact-head canonical receipt.

Parallel preparation identified a reader disposition that must be recorded
before implementation: legacy provenance exists to keep pre-M2 claims readable,
while NULL policy grants no new upstream reuse permission. Complete coordinates
can match a newer restrictive observation exactly; incomplete legacy provider
and source-ID coordinates cannot. The approved sources do not specify the
overlap rule. Preserve rows, timestamps and user overrides; do not silently
choose blanket NULL permission or blanket historical suppression. This does not
block the independently prepared whole-response writers or allocation guard.
Commander retains shared production ownership; agent edits were confined to the
assigned capacity-test leaf. No Codex Security, push, PR, merge or shared-file
release occurred. M4 owns v17/v7; Access v18 remains conditional on exact merged
commit/tree and explicit release.

### 2026-09-05 — Allocation guard exact verification and writer entry points

Allocation guard `956bd7d09d9e13feedbdeef4e6455664d7c52bda`, tree
`ff8ccb55175fdc95d14a8061d73b91d1e4437fab`, passed the canonical PR gate.
Both receipts were machine-checked for that exact commit/tree, dirty=false,
27/11 gate counts, and every pass/exit 0. The JavaScript inventory passed all
297 checks. This later checkpoint changes documentation only; it is not part of
the tested tree. No production edits remain uncommitted.

The next writer slice has three actual production command constructors:
Desktop `records.rs` creates `CreateProviderRecordCommand` and
`ApplyProviderMetadataCommand`; provider-runtime `metadata.rs` creates
`CommitMetadataRefreshCommand`. Each must carry one required recorded response
policy. Search Save has a separate existing path through runtime `search.rs`
and store `search_actions.rs`: refetch must pair fields with its detail-response
policy; Cached mode must retain receipt policy and original expiry. Desktop
namespace/artwork side effects precede current store admission and must move
behind admission. Do not infer protection from a later transaction rollback.

Zero-freshness representation already has an owner and regression:
`StoredSearchCandidate::metadata_fields` uses original observation, nullable
expiry and initial Stale when no positive fresh interval exists. Field/rating
domain constructors reject expiry equal to fetched time; metadata-cache entries
permit equal deadlines. Reuse the existing representation without adding one
second or changing published migrations, but do not treat Stale as HTTP reuse
permission: LastKnownGood currently admits it. Runtime refresh's Fresh-field
guard and Refetch receipt/SDK validation must be reconciled with the approved
live-response behavior in the same integrated slice, not activated separately.
Single/batch projection, raw cached-refresh claim views, ratings and replay
must all respect the response-policy owner. Legacy unknown overlap remains the
explicit disposition recorded above, not an implicit blanket permission.

Access reported independent E0/E1 qualification at its merged dev
`62e10d2e` / tree prefix `d6fcea15`; these are coordination-reported identities,
not a new M4 rebase or handoff. Its isolated plans/harnesses leave all M4 shared
surfaces and v17/v7 ownership intact. Access v18 still requires M4's exact merged
commit/tree and explicit release. No Codex Security, push, PR or merge occurred.

### 2026-09-05 — Response-policy writer integration and native verification

Continuation made production and test progress from committed checkpoint
`4d3ba2bcd48af546d247c3474c09c2f04a30ab7a`, tree
`e8e9c19b2cc0ba3ecffce4d6191ea9a8508913cd`. The following work remains dirty;
the earlier 27/11 exact-head receipts do not cover it.

Create, Apply, refresh and Search Refetch now carry the original required
response policy through existing application/runtime/store owners. Cached Save
uses its receipt's policy. Whole-response checks precede payload statements:
NoStore, mixed provenance or coordinates, duplicate keys, observation mismatch
and policy-exceeding claim/cache deadlines fail without payload DML. Field and
rating registry rows store canonical policy; identical retries compare it,
including historical NULL collisions. Published schema/archive versions do not
change. Original observation and zero-freshness Stale/null survive conversion,
Search action receipt decoding and SDK validation. Microsecond storage precision
is applied to a sub-microsecond zero-window save without manufacturing freshness.

Native tests exposed a real refresh-replay defect: runtime supplies unbound
provider fields, but saved response receipts require Record and field bindings.
The shared refresh writer now binds the returned claims and decodes its encoded
historical response before committing. First response and replay use that same
snapshot; an unreadable response cannot become a durable receipt. The existing
complete provider fixture also correctly participates in provider-unavailability
transitions, alongside all 513 title claims.

Dirty-tree checks passed with physical SSD TMPDIR and system pkg-config:

- `cargo check --workspace --all-targets`.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test -p fasti-store --lib --quiet`: 438 passed, 6 explicit ignores.
- `cargo test -p fasti-provider-runtime --lib`: 96 passed.
- SDK build and candidate-action JavaScript leaf: 35 passed.
- Desktop `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets`.
- Core and changed-leaf formatting plus whitespace checks. Whole Desktop
  formatting also reports pre-existing drift in unchanged `lib.rs` and
  `metadata.rs`; those files were not reformatted in this slice.
  Desktop lockfile gained only the already
  used provider-runtime `httpdate` dependency edge, not a package upgrade.

These are focused integration checks, not canonical exact-head, browser,
accessibility, packaged-host or merge evidence. Do not commit/activate this as a
completed policy slice before single/batch projections, rating reads, raw cached
refresh views and payload-bearing refresh replay share policy-aware disclosure.
All receipt claim references, including selected projection provenance, require
that check inside the current authorization transaction. Preserve immutable
receipts; denied historical payload is MetadataClaimStale, not a missing receipt
that re-executes an operation. Search action audit-only replay remains distinct.
Legacy unknown overlap disposition above remains unresolved. Desktop's early
NoStore artwork guard is not full image-response cache governance or proof that
all artwork side effects follow current mutation admission. These remain active
integration work, not scope removal or a delivery claim.

Commander retained all shared production writes. Parallel agents edited only
allocated test leaves; those leaves are released. Native review identified the
precision edge and complete-provenance fixtures. One agent reached a usage limit;
the commander completed validation locally without an API key. No Codex Security
tools, skills or plugin ran. No packaged authentication investigation, push, PR,
merge, v18 allocation or shared-file release occurred. M4 retains v17/v7 ownership.

### 2026-09-05 — Reader evidence integrity and legacy-overlap disposition

Further dirty-tree progress keeps the preceding writer integration intact.
Single-field, bounded batch and rating loaders now join the existing immutable
claim registry and decode canonical response-policy evidence before returning
payloads or resolving overrides. Known policy requires complete provenance and
the same original observation at SQLite microsecond precision. A nullable join
sentinel distinguishes a missing/wrong-scope/wrong-kind registry row from valid
historical NULL policy; the former is IntegrityFailed, not StorageUnavailable.
No registry row or policy is synthesized on read.

Payload-bearing refresh receipts now validate every field, rating and selected
projection claim reference against the scoped registry in one bounded ID query.
Cache references must be represented by their claim evidence. Validation stays
inside the current authorization transaction, including both concurrent replay
branches and cache-hit receipt insertion. Duplicate references preserve original
microsecond timestamp semantics. This checks evidence integrity, not yet reuse
eligibility: valid no-cache or expired policy still needs the filtering step
below. Search audit-only action replay is unchanged.

Native review found the nullable-sentinel error classification; the commander
fixed it and the agent added an isolated test leaf regression. Tests cover
missing/wrong-workspace/wrong-Record/wrong-kind registry evidence through single,
batch, rating and authorized projection reads; user overrides survive and reads
write no payload. Additional tests cover canonical-policy corruption hidden by
an override and rating/projection-only refresh receipt corruption, including
rejection of a new cache-hit receipt without altering old receipt bytes.

Final dirty-tree validation for this evidence-integrity step: full store library
441 passed, 6 explicit ignores; workspace all-target strict clippy passed; core
and allocated-leaf formatting and whitespace checks passed. These do not replace
the exact clean-head canonical gates after complete policy integration.

The 10,000-Record release benchmark now runs both the legacy-only dataset and a
dataset with complete provider claims and canonical stored policy. Each measures
100 requests after warm-up. Dirty-tree p50/p95/max were 1.778409/1.836839/1.966089 ms
for legacy and 2.460878/2.609098/2.643987 ms with observed policy. Both satisfy the
existing 250 ms p95 gate. These synthetic local results are not live-provider,
merged-head or future complete-filtering evidence.

Under the user's existing authorization to select recommended implementation
decisions, adopt the conservative historical overlap rule identified by native
review: preserve historical local display and overrides absent contradictory
known restrictions. Complete provenance uses exact provider/namespace/source-ID/
locale/region variants. Incomplete legacy provenance cannot prove independence:
within the same Record/field/namespace, missing locale/region/provider/source-ID
coordinates potentially overlap a known restriction. A newer NULL-policy row
does not cancel known restrictive evidence. Preserve all rows and independent
complete variants; do not call unknown policy upstream permission. This resolves
the former wildcard-versus-exact-None ambiguity conservatively, with the explicit
tradeoff that ambiguous historical regional data may need refresh or a user
override before display. The user-facing continuation stated this disposition.

Next integration must retain typed policy alongside each decoded claim, suppress
same-variant history before eligibility, and use one captured read time. Original
claim expiry bounds freshness: Saved Search's 120 seconds must not become 24 hours;
zero-freshness Stale/null gives zero fresh duration. The plan's metadata stale
ceiling remains separate from Search-page retention. Existing lifecycle,
profile LastKnownGood and override selection remain the resolution owners; a
local LastKnownGood display is not proof of a provider outage. General projection
and stale-on-error cache contexts must not silently be conflated. Raw cached
refresh views and receipt replay require the same policy-aware disclosure rule,
including denial through a newer overlapping restriction. Eligibility remains
unimplemented here, not a closed gate or scope removal.

No Codex Security, external AI key, packaged authentication investigation,
production migration, archive change, commit, push, PR, merge or shared-file
release occurred. Commander remains the sole shared production writer; agents
provided bounded native review/preparation and one allocated test-leaf change.

### 2026-09-05 — Policy-aware disclosure and full-history restriction barrier

Further uncommitted integration supersedes the preceding “eligibility remains
unimplemented” checkpoint. Single-field, bounded batch and rating readers retain
typed policy through selection. Reuse uses the original observation and expiry,
including zero-freshness, exclusive expiry, backward-clock rejection and the
metadata-specific seven-day ceiling. Known restrictions suppress overlapping
historical values without deleting claims or user overrides. All-NULL histories
preserve the existing resolver's Fresh-over-Stale behavior.

Native investigation reproduced a restriction buried below 256 newer NULL-policy
observations. Payload limits remain 256. A companion query now ranks narrow
policy keys across full scoped history and joins policy evidence after sorting.
Batch readers stream this companion once, retaining one field history rather
than a page-wide map of variants. All-known groups retain their existing fast
path. Locale/region ranking follows domain case normalization. The pinned SQLite
coroutine plan is checked; the entire checked batch stream must pass monotonic
scope validation before returning projections. History scans are not constant
work and have no silent truncation-as-permission fallback.

Refresh receipt disclosure now distinguishes the first live response from later
reuse. A real no-cache response can be returned once with its Stale/null state;
later receipt reads and same-operation commits return MetadataClaimStale when
their evidence is ineligible. They do not erase the receipt or rerun its operation.
A still-fresh cache entry whose references are no longer reusable is a cache
miss. Existing authorization and evidence-integrity checks precede disclosure.
The shared problem text no longer promises a visible LastKnownGood value when
none is eligible. Its code/status/safe-state/retry contract is unchanged; the
canonical example and generated API/SDK projections were updated together.

Native agents owned only allocated test leaves and read-only review. Regressions
cover 256/4,096 newer NULL histories, independent source/locale variants,
incomplete legacy overlap, normalization, single/batch/Search/rating parity,
zero-write replay/cache denial and real stop/reopen. The existing v7 archive
round-trip owner now also proves the buried restriction survives actual staged
restore, all 35 stream byte/descriptor checks and the user override. No production
schema or archive code changed. The final native filtering review found no
remaining concrete disclosure or cross-scope defect in this slice; this is not
the programme-wide or exact committed-diff review gate.

Final focused dirty-tree checks: full store 458 passed/6 explicit ignores,
including the added replay and actual-restore regressions. Application
167 and provider runtime 96 passed; strict workspace all-target clippy passed;
SDK build and 35 candidate-action tests passed; generation emitted 22 artifacts.
Desktop all-target `cargo check` also passed with system pkg-config; its lockfile
still differs only by the existing provider-runtime httpdate dependency edge.
This is compile evidence, not packaged-host or authentication runtime evidence.
Core formatting and whitespace checks passed. Exact committed-head canonical
gates remain open; these focused checks do not substitute for them. The
500-Record mixed-history measurement passed as below.

Synthetic release Search (10,000 Records, 100 requests after warm-up): legacy
p50/p95/max 2.350035/2.437285/3.140173 ms; observed policy
3.973752/4.032642/4.150821 ms. Both meet the existing 250 ms p95 gate. SQLite
3.53.2 companion evidence retained one latest policy at both 256/4,096 NULL
depths; fullscan steps 0/0 and VM steps 2,384/33,104 explicitly show deeper work.
Dense 100-Record, five-field, 256-claim, 4,096-byte-value measurements passed the
192 MiB ceiling: legacy HWM 22,745,088 bytes, median/max 3.109834024/3.160449357 s;
mixed known policies with maximum-width source identifiers HWM 38,866,944 bytes,
median/max 8.828862065/9.351858484 s. Dense-history latency is not a replacement
for the critical Search p95 gate. At 500 Records with mixed known policies and
maximum-width source identifiers, HWM was 56,770,560 bytes and median/max
46.836099338/47.084398340 s across five samples. This remains below 192 MiB but
is deliberately dense stress evidence, not a claim of sub-second dense pages.
The latency remains visible for continued optimization. These are local
synthetic dirty-tree results,
not live-provider, packaged-host, exact merged-head or hardware qualification.

The next proven integration work remains Desktop mutation/artwork ordering and
the distinct image-response policy: current Create/Apply callers can fetch,
store and prune artwork before Store authorization/admission; image responses do
not yet carry their own reuse evidence. Preserve successful Record mutation when
optional artwork fails and reuse the existing response-policy parser. This is
active work, not scope removal. Workbench/runtime/live-provider and exact-head
delivery gates remain open. No Codex Security, AI key, packaged authentication
investigation, commit, push, PR, merge, v18 allocation or shared-file release.
M4 retains v17/archive v7; Access's separate qualification lanes remain disjoint.

### 2026-09-05 — Desktop Save ordering and provider-keyed posters

Continuation revalidated the dirty M4 worktree at committed HEAD
`4d3ba2bcd48af546d247c3474c09c2f04a30ab7a`; no prior committed result was
replaced. The pending Desktop test process was terminal with two failed
projection assertions. The fixture had not enabled BasicInfo after setup;
enabling it through the existing authorized configuration owner fixed the
fixture without weakening Record, identifier, title or claim-ID assertions.

Both real Desktop Create/Apply callers now pass their completed Store result
and the unpolled artwork future through `records::finish_provider_save`.
Authorization/admission failure returns before artwork is polled. Successful
mutation remains successful if optional artwork fails. Dropping the pending
artwork future does not undo or repeat the committed mutation. The new test
leaf uses real Store commands and the shared helper; it does not claim live
image-transport or packaged-UI evidence. Independent native review found no
new issue in this ordering change.

The selected poster lookup now uses the claim's actual provider ID rather
than its external identifier namespace. A regression exercises real stored
TMDB and Google Books poster claims plus the existing artwork cache. Replacing
the lookup temporarily with the old namespace lookup reproduced a null asset
path; restoring the fix passed. The wire `poster.source` remains the namespace,
so no public DTO meaning or identifier mapping changed.

Dirty-tree checks: all 43 no-default-feature Desktop library tests passed;
all 51 default-feature Desktop library tests passed; all-target Desktop check
and strict clippy passed. Clippy first caught an `err().expect()` test idiom;
the test now uses `expect_err` with the same assertion. Desktop formatting
also normalized pre-existing formatting in the metadata refresh adapter and
one provider-lease expression, without semantic changes. These checks are
not a canonical exact-commit receipt, live-provider result or packaged-host
qualification. No commit, push, PR, merge or migration handoff occurred.

The image-response policy remains separate active work: image headers are not
yet observed before storage, and returning a validated local path alone cannot
govern later asset-protocol reads. Read-only preparation identified the pinned
Tauri custom `asset` protocol hook as a candidate for enforcing the serving
boundary while retaining `convertFileSrc` and current CSP; it is not yet
implemented or verified. Preserve live no-cache image display and authorized
offline cache reuse, rather than hiding those cases to make tests pass.

Parallel allocation remains compact: commander owns all production integration;
Mendel prepares the native asset-serving boundary; Anscombe prepares independent
filesystem/policy publication and negative-test evidence. Both lanes are
read-only. Access may own its new isolated
`tests/e2e/access-parallel-regressions.spec.ts` in its separate worktree, with
no changes to shared helpers or production surfaces. M4 retains v17/archive v7;
no v18 allocation or shared-file release. Codex Security remains excluded.

Serving preparation subsequently confirmed pinned Tauri 2.11.5 registers custom
protocols before its fallback `asset` implementation. The applicable hook is
`register_asynchronous_uri_scheme_protocol("asset", ...)`, not
`on_web_resource_request` (which covers only `tauri`). The implementation must
validate an explicitly issued locator and current metadata authority, then
return bounded bytes with `Cache-Control: no-store` to prevent WebView reuse
from bypassing later image-policy checks. For upstream image no-cache/no-store,
fetch at delivery and return the live body, rather than replaying an earlier
Save-time fetch. Eligible persisted images must remain available offline with
zero network/vault access. The current existing-file-only path issuance needs
to mature accordingly. Finite image deadlines, locator authority/lifetime,
wrong-WebView/path traversal, repeated requests and packaged reload evidence
remain implementation requirements, not completed checks.

Independent persistence inspection also found that the current app-cache root
is shared between Fasti data roots while its mutex is instance-local. Existing
file staging syncs the image but not the parent directory; local-path checking
and path opening are separate operations. The upcoming image policy integration
must resolve node scoping, publication ordering, restrictive replacement,
restart integrity and descriptor-safe file access together. Do not claim that
an unsigned sidecar or path-issuance check alone closes these boundaries.

### 2026-09-05 — Physical-node artwork isolation and dirty-tree PR checks

`ArtworkCache::new` now requires the opened kernel's `DataRootIdentity` and
uses the existing stable node-scope hash beneath the platform artwork-cache
directory. Every production and test caller supplies its associated kernel
identity. No new hash implementation, dependency, schema, API or authentication
owner was introduced. The regression uses real kernels: rename and reopen keep
the original image; replacing the configured data path selects a separate cache.
Temporarily restoring the old unscoped constructor reproduced the same-root
failure. The fixed test also seeds a legacy parent-level image, proves it is
not adopted, and proves new writes/pruning leave it unchanged. Legacy cache
bytes were not deleted or attributed to an arbitrary node.

The full Desktop library suite passed all 52 tests after the legacy-isolation
assertions. Desktop strict all-target clippy and formatting passed before that
test-only extension and were rerun afterward. Native read-only review found no
new constructor/caller regression. This fixes node separation, not the remaining
image-response admission or per-request delivery policy.

`cargo xtask test pr` was run with the physical TMPDIR and system pkg-config.
All executed checks before receipt publication passed, including deterministic
generation, authored/generated contracts, JavaScript formatting/typecheck,
299 JavaScript tests, workspace formatting/clippy/tests, HTTP conformance,
workspace build and repository/package guards. The command then correctly
exited 1 because source was dirty. No verification receipt was emitted; the
command removes its previous contract receipt when starting. Documentation
verification and the portable suite follow successful contract verification
and were not reached in this run. Do not label this a passing canonical PR
gate or reuse the old portable receipt as proof of the dirty source.

The next image integration will use one bounded envelope file containing the
canonical policy and validated image bytes, rather than two independently
published files. The private native handler can decode that file, so one atomic
replacement removes the paired-publication problem. It still needs bounded
header/body parsing, original deadlines, key/digest/length binding, no-follow
descriptor safety, ordered restrictive observations and directory sync. A
Record-scoped internal locator can reuse exact `ListRecordsQuery` authorization
and current selected poster on each request, without a provider/URL map. Preserve
the existing Desktop scoped credential authority; do not silently substitute
the browser-selected profile. Recheck authority and poster selection after
asynchronous I/O. These are implementation decisions, not completed capabilities.

Review correction: `DirEntry::metadata` does not traverse symlinks, per the
[Rust standard-library contract](https://doc.rust-lang.org/std/fs/struct.DirEntry.html#method.metadata).
The earlier read-only claim that this call itself follows links was false.
The separately identified pathname-replacement and check/open races remain.

Scoped Access ownership exception: after confirming no M4 changes in the file,
Access may exclusively edit `packages/ui/src/account-security-view.svelte` in
its isolated regression worktree for the reproduced first-run pending-confirmation
header-exit defect. M4 stays read-only on that file during this allocation.
Access must return its exact commit/file hash, review and seven-regression
evidence before integration. No other Workbench file, helper, API, SDK, registry,
schema, portability or migration ownership was released. M4 retains v17/archive
v7; no full handoff, commit, push, PR or merge occurred here. Codex Security stays
excluded.

### 2026-09-05 — Same-handle artwork validation and private staging

The cache reader now opens an image once, checks regular-file type and size on
that descriptor, and reads its bounded prefix from the same file. Unix uses
the already-locked `rustix 1.1.4` filesystem dependency with `NOFOLLOW`,
`NONBLOCK` and `CLOEXEC`; Windows opens the reparse point itself before checking
the handle's file type. No new dependency family or cache framework was added.
Temporary-file permissions now apply to the open staging handle before bytes
are written and synced; post-rename pathname chmod was removed.

The native Unix regression rejects symlinks, directories and a FIFO without
blocking, then proves a held file descriptor continues reading the original
inode after its pathname is replaced. Temporarily removing `NOFOLLOW` made the
symlink assertion fail; restoring it passed. All 53 default-feature Desktop
library tests passed; formatting and whitespace checks passed. Strict all-target
Desktop clippy was rerun on the restored source. Independent native review found
no introduced regression. Windows remains source-reviewed, not native-qualified.

This is a bounded I/O improvement, not completed image delivery: root ancestors
are still pathname-resolved, and `local_path` still returns a path which the
built-in asset handler subsequently reopens. The prepared native asset handler,
single-file policy envelope, root descriptor lifetime and image-response
admission remain the active implementation front. Keep cached offline images,
live no-cache/no-store images, request authorization, original deadlines and
bounded resource use in the required final behavior. Do not substitute these
focused tests for those unimplemented boundaries.

No source commit, PR, merge, new migration or shared-file release occurred.
Access's one-file busy-state allocation remains exclusive; its reported local
commit `bf8526ad4ecbfc270a475428d9120708c7ac60cc` is not an M4 integration or
handoff. M4 remains read-only on `packages/ui/src/account-security-view.svelte`.
Codex Security remains excluded.

### 2026-09-05 — Native artwork delivery and response-policy envelope

The dirty Desktop slice now registers a bounded private native asset handler
instead of granting the built-in handler directory access. Record-scoped locators
contain neither upstream URLs nor filesystem paths. Existing scoped credentials,
exact Record selection and selected poster provenance authorize each request;
selection and the actual main WebView origin are rechecked after asynchronous
work. GET-only requests, exact authority/origin matching and bounded URI/header
input precede task admission. Responses are non-cacheable, image-only, nosniff;
failures carry no private detail. This does not change browser profile authority.

Image cache entries now contain a bounded canonical policy header and image
body in one atomically published file. Key, digest, length, version, image bounds
and original policy are checked on the same opened file. Image freshness is
capped at 24 hours and total stale eligibility at seven days, shortened by the
observed image response policy and age. No-cache requires another observation;
NoStore live bytes are never persisted. The old entry is durably invalidated
before requesting a replacement. Only a pre-representation failure can reuse
eligible old evidence, without renewing its deadlines. Legacy raw image files
are misses, not implicitly adopted evidence. No migration/archive change.

Linux/Android cache operations use the retained directory descriptor for reads,
writes, invalidation, sync and pruning. A real rename/replacement regression
proves the opened directory remains the target and the replacement's same-key
sentinel and inventory are unchanged. Other platforms are not qualified by this
test. Initial root discovery still resolves its configured ancestors.

Independent native review identified an unbounded task queue behind the existing
provider gate. Admission now uses the existing Tokio semaphore before spawning:
64 requests maximum, immediate 503/Retry-After on overload, permit released with
the task. A regression proves the 65th admission fails and capacity recovers.
The global provider gate is an explicit throughput ceiling, not a claimed
performance optimization. Request disconnect cancellation, burst rendering and
image transport behavior still require direct end-to-end evidence.

Verification on this dirty source: all 71 default-feature Desktop library tests
passed, including seven envelope/policy tests, eight protocol/admission tests,
real scoped Record selection and Store-first Save regressions. The shared runtime
cache-policy parser's 22 focused tests passed. Strict Desktop all-target clippy,
formatting and whitespace checks passed after the final two test additions.
The full canonical clean-source receipt remains absent as
recorded above; these checks do not replace it or prove live WebView delivery.

Commander remains sole production writer. Parallel agents owned only the two
test leaves and released them. Final read-only native review found no concrete
introduced defect in descriptor routing or admission lifetime; it explicitly
retained the non-Linux/Android pathname limitation. No commit,
push, PR, merge, migration allocation or shared-surface release occurred.
Access's latest reported local documentation checkpoint is
`f60c3da0278a2458dceae9883315cacba11b0c19`, tree
`9f5b0866e7df1b11dc4b039415454e1c608d0477`; its narrow UI allocation remains
exclusive and is not an M4 handoff. All Codex Security tools, skills and scans
remain excluded by the user's explicit instruction.

### 2026-09-05 — Real image response lifecycle and live CDN qualification

Previous goal turn classification: progress. It added executed directory and
admission regressions, strict checks and an updated checkpoint. This turn
continues the same M4 scope, not a replacement completion definition.

The private artwork lifecycle now accepts the unpolled governed response future.
Fresh cache hits never poll it. The actual authorized client is carried through
body consumption/publication so its existing request permit is not released at
headers. No transport trait, provider stub, runtime command or dependency was
added. This removes the mutable representation-seen flag: only request/status
failures can reach stale fallback; once HTTP 200 is observed, validation failures
cannot restore prior bytes. Native review caught an intermediate refactor that
lost eligible fallback on local invalidation failure. It was corrected before
verification: no response is observed and the request stays unpolled on that
failure, so eligible original bytes remain usable without renewing policy.

Eight response-lifecycle tests now exercise actual reqwest response/body values,
policy parsing and cache I/O. They prove fresh-hit laziness, invalidation before
request polling, eligible original-policy outage fallback, denied stale reuse,
live-only NoStore, no-cache revalidation, invalid/oversized HTTP-200 rejection,
read-only-directory behavior, delayed-body observation time, guard lifetime and
cancelled-reader non-resurrection. Fixtures are explicit; these are not proof of
native WebView cancellation or live DNS/TLS.

The separate opt-in live test uses TMDB's public
[documented image example](https://developer.themoviedb.org/docs/image-basics)
without any credential. It found two real distinctions: negotiated bytes were
WebP despite a `.png` URL; a later lookup returned four IPv4 plus eight IPv6
addresses, exceeding the shared resolver's historical eight-answer cap.

Debug report, investigation workflow:

- Symptom: governed artwork failed with "The host name returned too many addresses."
- Root cause: `system_resolve` rejected a legitimate complete dual-stack CDN
  result before application authorization. The cap originated in the existing
  provider runtime; this was not introduced by the artwork refactor.
- Fix: keep the existing shared resolver and bounded deduplication, raise its
  complete-result cap to 16, reject overflow rather than truncate, and continue
  authorizing every retained address before the pinned request. Both provider
  requests and Desktop node connection checks reuse this owner.
- Regression: the actual collector failed with the old cap on 12 addresses;
  the fixed test accepts complete deduplicated dual-stack results and the exact
  16 boundary, rejects 17/empty, and rejects a public result containing a late
  loopback address through the existing application policy.
- Evidence: all 97 provider-runtime tests and strict all-target clippy passed.
  The original live governed request then passed, returning 8,310 WebP bytes at
  `2026-09-05T12:34:19.846490612Z`, SHA-256
  `b168986ce4b29065c3295eaf41c2b31c4a2f2a0eb751fc88dc8d3d85d8fef86e`.
  Its original reusable policy survived cache reopen; a deliberately unpolled
  replacement future proved the eligible cache path made no second request.
- Status: DONE_WITH_CONCERNS for this bounded resolver bug, not M4 completion.
  More than 16 unique answers still fail closed. The live image test remains
  opt-in because it requires public network availability, and was explicitly run.

After the final lifecycle test, Desktop library verification passed 79 tests
with one network test ignored in the default suite (separately passed above).
Strict Desktop all-target clippy passed. The canonical clean-source receipt
is still required for the integrated slice; no old receipt may be reused.

Read-only native preparation identified installed `tauri-driver`/WebKitWebDriver
and the reusable W3C client in `scripts/smoke-desktop-access-webdriver.py`.
Do not run that script's authentication gate. The product-image scenario needs
disposable Store/artwork fixture seeding, actual native image decoding under the
existing CSP, exact source/binary identities and offline network evidence. The
B1 benchmark shell must stay inert and is not product-image proof. No packaged
authentication investigation, new provider-health override, Codex Security,
schema/archive change or Access shared-file release occurred.

Final read-only native review found no concrete defect in the current complete
DNS answer handling or artwork lifecycle, including governed permit lifetime
and invalidation-failure fallback. The commander is saving this verified logical
unit as a local integration checkpoint before the clean-source canonical gate.
This is not PR delivery, native rendering qualification, or M4 completion; all
remaining programme scope and the current v17/archive-v7 ownership remain intact.

### 2026-09-05 — Exact integration checkpoint verified

Local integration commit `8d03cf1c4d0064621e27efb908ef17691c747352`, tree
`9e64a71f0bfee934b98595d0f21d338c7d39ca2e`, passed `cargo xtask test pr`
with physical TMPDIR and system pkg-config. Both receipts declare `dirty: false`
and that exact commit/tree: `target/fasti-receipts/b1-contract-verification.json`
has 27 passing gates; `target/fasti-receipts/b1-portable.json` has 11 passing
gates. Documentation verification also passed. The worktree was clean when the
command completed. These receipts replace the missing dirty-source receipt
noted earlier; they are software gates, not native product rendering or M4
completion. This following checkpoint update changes documentation only.

Live GitHub inspection found no PR for `codex/nuvio-metadata-programme-m4` and
no open code-scanning alerts. Dependabot alerts 1/3 remain on glib 0.18.5 in the
two Tauri lockfiles; alerts 4/5 remain on image-size 2.0.2. The existing image-size
patch and prior Tauri/glib disposition remain present. Do not describe this as
zero vulnerabilities or dismiss an advisory. No hosted review or merge occurred.

The next vertical integration must replace the old Workbench provider-selection
path with existing `searchProviderPage`, `readSearchCandidate`,
`saveSearchCandidate` and `searchRecords` owners. Preserve receipt IDs, explicit
Cached/Refetch intent, stable operation IDs, offline/partial results, profile
generation fences and existing canonical Record navigation. The browser host
still rejects legacy provider Search; Desktop still uses its legacy commands.
This is a proven implementation gap, not a new API or migration requirement.
Commander owns these shared surfaces; native fixture preparation stays test-only.

Access's existing exclusive `account-security-view.svelte` exception is expanded
only to the source/browser-proven mobile `flex-basis` correction for
`.access-heading > div` and `.task-copy` at the existing breakpoint. Preserve all
controls and desktop behavior. No other file is allocated; delayed-notice layout
shifts are a separate disposition and are not included in this permission.
M4 stays read-only on that component until exact handback. No v18 release.

### 2026-09-05 — Native artwork fixture qualification

The targeted `scripts/smoke-desktop-artwork-webdriver.py` reuses the existing
native W3C client and managed-process cleanup, without running the authentication
gate. Its test-only Rust seeder requires a marked, empty, private disposable root,
uses the actual platform keyring and Store owners, and caches the repository PNG
with explicit synthetic provenance and a ten-minute original policy. No provider
response, provider health or authentication qualification is invented.

The actual release Desktop WebView passed the fixture under a private keyring,
disposable display and separate loopback-only network namespace: exact canonical
Record poster locator, 512-by-512 decode, reload, query-bearing request rejection
with a confirmed image error, and restored decode. The provisional dirty-source
receipt is `target/native-artwork/run-q7jhbcqy/receipt.json`; it is not clean-head
proof. Source and binary hashes are included in the receipt. Public TMDB image
acquisition remains the separate live evidence above, not this offline fixture.

Independent native review tightened exact-source/dimension assertions and added
termination-handler registration and fail receipts for cleanup/cancellation.
The harness `--self-test` passes seven positive/negative image predicate cases
and cleanup registration. Actual cancellation fault injection is not claimed.
Strict Desktop all-target clippy passes. The next step is a clean-source rebuild
and native rerun after this local test-only checkpoint; no production behavior,
schema/archive allocation, authentication work, push, PR or merge changes here.

Read-only Search preparation identified SDK method types as the host contract
owner, per-call browser-client selection, and an explicit native cancellation
seam: Tauri invoke has no AbortSignal support. Generation fencing alone must not
be described as cancelling native provider work. Commander retains shared-file
ownership; the next vertical slice must resolve this in the existing scoped
command/runtime path rather than lose receipt or lifetime semantics.
