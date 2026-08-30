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
