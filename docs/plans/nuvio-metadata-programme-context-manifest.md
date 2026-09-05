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
