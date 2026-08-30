# Fasti documentation portal build plan

Status: planning gate

Source branch: `dev`

Pinned source commit: `d035933bd2b804f23db1a5402ee564eba7ce5b0c`

Canonical public URL: `https://fasti.scrobble.dev`

This plan builds a public documentation portal. It does not declare Fasti a
supported release. Product support claims continue to come from the capability
registry, the release gates, and exact-head evidence.

## Decisions already fixed by the brief and repository

- Use Docusaurus `3.10.2` in `apps/docs`.
- Keep documentation, contracts, and the site in the Fasti monorepo.
- Keep `contracts/registry/v1/capabilities.yaml` authoritative for capability
  meaning. Generated pages are projections.
- Publish only files selected by `docs/site.yaml`.
- Map the exact persona identifiers in
  `tests/conformance/identity-uat-matrix.v1.csv` into five outcome tracks.
- Treat `audhd_user` and `screen_reader_user` as cross-cutting personas.
- Use semantic HTML first, Docusaurus primitives second, Tabler controls third,
  and Fasti token styling after those layers.
- Keep TrailBase and Loco outside this static documentation runtime. ADR-0005
  remains controlling.
- Use GitHub Pages as the static origin. Use a DNS-only Cloudflare CNAME for
  `fasti.scrobble.dev`.
- Keep the production deployment choice visible but unavailable until its
  release gate passes.
- Do not add telemetry, hosted search, external search requests, fake data,
  fake reviews, fake support claims, or secrets in URLs or browser storage.

## What already exists

- `contracts/registry/v1/capabilities.yaml` owns capability identifiers,
  lifecycle, contract surfaces, body ownership, problems, examples, and UAT
  relationships.
- `xtask/src/generate.rs` and `xtask/src/verify.rs` provide deterministic
  generation, drift checks, mutation sentinels, receipts, and the canonical PR
  gate.
- `packages/tokens` owns the browser design tokens.
- `packages/ui` and `apps/web` prove Tabler-first patterns, but their Svelte
  components are not React dependencies.
- `brand/DESIGN.md`, `brand/logos/`, and `brand/assets/` own the established
  archival field-guide identity.
- `tests/conformance/identity-uat-matrix.v1.csv` contains the canonical persona
  identifiers. It is an acceptance inventory, not implementation evidence.
- OpenAPI, AsyncAPI, JSON Schema, JSON-LD, OKF, problem, capability, and SDK
  artifacts already exist under their governed owners.
- `scripts/check-no-publish.sh` and its mutation test fail closed on public
  publication. They need one named documentation-only exception.

## Segment 1: Product record and public content model

### User outcome

A reader can choose a path based on a real task and can see what Fasti can do
today without mistaking a contract, fixture, staged implementation, or preview
for a supported release.

### Source of truth

- `README.md`
- `ROADMAP.md`
- `docs/constitution.md`
- `docs/definition-of-done.md`
- `docs/capability-ledger.md`
- `docs/network-configuration.md`
- `tests/conformance/identity-uat-matrix.v1.csv`
- `contracts/registry/v1/capabilities.yaml`
- `brand/DESIGN.md`

### Exact files

- `PRODUCT.md`
- `docs/site.yaml`
- `docs/personas.yaml`
- `docs/schemas/site.schema.json`
- `docs/schemas/personas.schema.json`
- `docs/style/ste/termbase.yaml`
- `docs/style/ste/abbreviations.yaml`
- `docs/style/ste/reviews.yaml`
- `docs/start/*.md`
- `docs/use/*.md`
- `docs/operate/*.md`
- `docs/integrate/*.md`
- `docs/extend/*.md`
- `docs/contribute/*.md`
- `docs/reference/*.md`
- `docs/accessibility/index.md`
- `docs/security/index.md`

### Dependencies and framework rung

Reuse repository facts and Markdown. Use YAML and JSON Schema only for the two
new governed manifests. Do not introduce a CMS, database, runtime service, or a
second content model.

`docs/schemas/personas.schema.json` validates the record shape. It does not
duplicate the persona enum. `cargo xtask docs verify` reads the persona column
from `tests/conformance/identity-uat-matrix.v1.csv` and requires exact set
equality with `docs/personas.yaml`. Each persona has exactly one primary track.
The verifier also requires `audhd_user` and `screen_reader_user` to carry the
cross-cutting flag.

### Prohibited changes

- Do not change capability lifecycle or release status to make the site look
  complete.
- Do not publish handoffs, reviews, private plans, evidence payloads, or imported
  artifacts.
- Do not claim ASD-STE100 certification. Public prose remains a
  `machine_checked` controlled draft until a human review entry exists.
- Do not invent personas outside the canonical matrix.

### Tests and evidence

- Manifest schema and path validation.
- Exact persona inventory parity.
- Every first task exists and is public.
- Every unavailable task names its blocker and safe alternative.
- Heading, link, support-state, controlled-language, abbreviation, and termbase
  checks.
- Generated public inventory contains no path outside the allowlist.

### Rollback

Delete the new content model and generated staging output. No product runtime,
contract meaning, or stored data changes.

### Gate status

Engineering and DevEx review cleared. Implementation pending.

## Segment 2: Deterministic documentation projection

### User outcome

Readers and integrators get one coherent site whose reference pages and raw
files come from the same exact Fasti commit.

### Source of truth

Segment 1 manifests plus existing governed contracts and generated artifacts.

### Exact files

- `xtask/src/docs.rs`
- `xtask/src/main.rs`
- `xtask/src/orchestration.rs`
- `xtask/Cargo.toml` only if an existing workspace dependency cannot parse a
  required format
- `tests/js/docs-publication.test.mjs`
- `package.json`
- `.prettierignore` or root format globs only when generated staging needs an
  explicit exclusion

### Dependencies and framework rung

Use Rust standard library, existing `serde`, `serde_json`, `serde-saphyr`,
`sha2`, `tar`, and existing contract outputs. The commands are:

```text
cargo xtask docs generate
cargo xtask docs verify --locked
cargo xtask docs package --locked
```

`cargo xtask test pr` calls `docs verify --locked`.

The PR-path verifier performs source parsing, schema checks, two-run projection
comparison, link and route checks, controlled-language checks, and generated
inventory checks. It does not run Webpack, Pagefind, Playwright, or axe. Full
site compilation, indexing, and browser checks run in the documentation build
job and the deep/UI gates. This keeps the canonical PR gate bounded while still
making documentation drift a PR failure.

### Data flow

```text
governed source files + site.yaml + personas.yaml
  -> validate exact owners and public paths
  -> target/docs-site/content and target/docs-site/static
  -> generated navigation, status matrix, raw references, llms.txt, manifest
  -> Docusaurus build
  -> local search index
  -> one immutable Pages directory
```

### Prohibited changes

- Do not make generated HTML or generated Markdown authoritative.
- Do not scan all of `docs/` and infer publication from frontmatter.
- Do not fetch remote data during generation.
- Do not write generated staging into the source tree.
- Do not emit unescaped Markdown that MDX v3 can parse as JSX. Contract prose,
  type signatures, braces, and angle brackets are emitted in fenced code or
  escaped text according to their meaning.

### Tests and evidence

- Two isolated generations are byte-identical.
- Generated inventory and digests are stable.
- Drift, missing source, duplicate route, traversal, symlink escape, invented
  persona, stale UAT row, and unpublished first-task mutations fail.
- Raw OpenAPI, AsyncAPI, JSON Schema, JSON-LD, capability, problem, OKF, and SDK
  documentation files resolve from the exact build.
- Every canonical RFC 9457 documentation path from
  `crates/fasti-application/src/problems.rs` resolves at its exact root route,
  such as `/v1/problems/capability-unavailable`.
- `llms.txt` lists only public routes and raw public resources.

### Rollback

Remove the docs command and its PR-gate call. Existing contract generation and
verification remain byte-for-byte unchanged.

### Gate status

Engineering and DevEx review cleared. Implementation pending.

## Segment 3: Docusaurus presentation and local search

### User outcome

A user, operator, integrator, extension author, or contributor can reach a
meaningful first result, verify it, recover from a problem, and choose the next
task. Search stays on the device.

### Source of truth

- `target/docs-site/` generated by Segment 2
- `packages/tokens`
- `brand/DESIGN.md`
- Docusaurus `3.10.2`
- Tabler `1.4.0`

### Exact files

- `apps/docs/package.json`
- `apps/docs/tsconfig.json`
- `apps/docs/docusaurus.config.ts`
- `apps/docs/sidebars.ts`
- `apps/docs/src/css/custom.css`
- `apps/docs/src/pages/index.tsx`
- `apps/docs/src/pages/status.tsx`
- `apps/docs/src/components/LocalSearch.tsx`
- `apps/docs/src/components/SupportState.tsx`
- `apps/docs/src/theme/Root.tsx`
- `apps/docs/static/.nojekyll`
- `tests/e2e/docs-portal.spec.ts`
- `playwright.docs.config.ts`
- `.gitignore`
- `scripts/check-js-workspace.mjs`
- `.github/workflows/ci.yml`
- `package.json`
- `pnpm-lock.yaml`

### Dependencies and framework rung

Use Docusaurus for routing, Markdown, navigation, static generation, metadata,
and sitemap. Use semantic HTML for page structure. Use Tabler only for controls,
forms, alerts, badges, tables, and icons. Reuse `@fasti/tokens`. Add Pagefind
only for the local generated index. Add no component framework or analytics
package.

`routeBasePath` is `/`. The docs application receives a generated sidebar and
content directory from `target/docs-site`. It also receives generated root
problem pages for every canonical `v1/problems/*` path. `apps/docs/.docusaurus/`
and `apps/docs/build/` are ignored build outputs.

Clean-clone order is explicit: the root documentation build first runs
`cargo xtask docs generate`, then the Docusaurus build, then Pagefind. The
JavaScript CI job runs generation before Nx build and typecheck. No package
assumes that `target/docs-site` already exists.

### Prohibited changes

- Do not import the Svelte `@fasti/ui` package into React.
- Do not add gradients, glass, decorative motion, card-wall layouts, fake
  testimonials, marketing claims, or an Ask AI control.
- Do not load a hosted search service or font service.
- Do not hide unavailable features.

### Tests and evidence

- Docusaurus production build from generated staging.
- Search finds allowlisted content and cannot find a handoff marker.
- Canonical URLs, sitemap, robots, alternate Markdown links, `llms.txt`, and
  page JSON-LD are valid.
- Keyboard navigation, visible focus, 44-pixel targets, heading order, landmark
  structure, reduced motion, error state, empty state, and screen-reader names.
- Playwright and axe at 320, 375, 768, and 1440 CSS pixels in light and dark
  themes.
- Manual AskTog, Gestalt, Nielsen, IxDF, WCAG 2.2 AA, EN 301 549,
  ADHD/AuDHD, and screen-reader evidence ledger.
- Impeccable detector, screenshots, and independent finish review.
- `scripts/check-js-workspace.mjs` recognises both `apps/docs` and
  `packages/deploy-plan` as private buildable workspaces and rejects any
  unregistered workspace.

### Rollback

Remove `apps/docs` and its package entries. Source documents and contract owners
remain intact.

### Gate status

Engineering and DevEx review cleared. Implementation pending.

## Segment 4: Experimental deployment planner

### User outcome

A reader can choose a truthful local review mode, download deterministic files,
verify the result, and roll it back. The site explains why production is not
available.

### Source of truth

- `README.md`
- `docs/dev-loop.md`
- `docs/network-configuration.md`
- `Dockerfile`
- `scripts/dev.sh`
- release-gate status from the generated support model

### Exact files

- `packages/deploy-plan/package.json`
- `packages/deploy-plan/tsconfig.json`
- `packages/deploy-plan/src/index.ts`
- `packages/deploy-plan/src/index.test.ts`
- `apps/docs/src/pages/deploy.tsx`
- `apps/docs/src/css/deploy.css`
- `tests/e2e/docs-deploy-planner.spec.ts`
- `package.json`
- `pnpm-lock.yaml`

### Dependencies and framework rung

The package is a pure TypeScript function with no React dependency. Use Web
Crypto in the browser for transient secret generation. Use native form controls
and Tabler presentation. Add one small archive dependency only if browser-native
download APIs cannot represent the required multi-file bundle safely.

The package boundary is an explicit objective because the generator must remain
testable without React and reusable by the site build. It is not replaced with
an app-local helper. It has one implementation and no speculative adapter or
factory.

### Modes

1. Native local development.
2. Podman local review.
3. Docker local review.
4. Trusted HTTPS proxy, advanced and bounded.
5. Production deployment, visible and unavailable.

### Prohibited changes

- Do not publish an image, package, or supported installer.
- Do not put secrets in URLs, local storage, session storage, logs, manifests,
  fixtures, screenshots, or analytics.
- Do not generate a public listener or trust arbitrary proxy headers.
- Do not claim the generated files are supported production configuration.
- Do not submit a native form with `GET` or place planner state in browser
  history. Secret fields use controlled state, `autocomplete="off"`, and
  `spellcheck="false"`. Reset, route change, `pagehide`, and back-forward cache
  restoration clear transient secret state. The page does not overwrite the
  user's clipboard as a cleanup strategy.

### Tests and evidence

- Deterministic non-secret output and file digests.
- Path, hostname, URL, bind, data-root, and proxy validation.
- Secret redaction, reset clearing, and no-storage browser tests.
- Playwright asserts that the URL, history state, cookies, local storage, and
  session storage contain no generated token before and after navigation,
  reset, reload, and back-forward cache restoration.
- Every mode has README, VERIFY, ROLLBACK, and manifest output.
- Production mode cannot be selected and names the B8 gate.
- Rapid resubmit, back navigation, reload, JavaScript-disabled explanation,
  keyboard-only, focus recovery, and download tests.

### Rollback

Remove the planner route and pure package. The documentation site remains usable
and the existing runtime is unchanged.

### Gate status

Engineering and DevEx review cleared. Implementation pending.

## Segment 5: GitHub Pages policy and deployment

### User outcome

The exact verified artifact from `dev` is available at
`https://fasti.scrobble.dev` with HTTPS and no hidden publication path.

### Source of truth

- GitHub Pages custom-workflow requirements.
- `scripts/check-no-publish.sh`.
- Cloudflare zone `scrobble.dev`.

### Exact files and external state

- `.github/workflows/docs-pages.yml`
- `scripts/check-no-publish.sh`
- `scripts/test-no-publish-policy.sh`
- `scripts/validate-docs-pages-workflow.mjs`
- `scripts/canary-docs-live.sh`
- GitHub repository Pages settings and `github-pages` environment
- Cloudflare DNS record `fasti CNAME Scrobble-dev.github.io`, DNS only, TTL Auto

### Dependencies and framework rung

Use GitHub's official Pages actions pinned to immutable commits. The build job
has `contents: read`. Only the deploy job gets `pages: write` and
`id-token: write`. Cloudflare provides DNS only.

`scripts/check-no-publish.sh` does not skip the Pages workflow by filename. It
passes that one exact file to `scripts/validate-docs-pages-workflow.mjs`, which
parses YAML with the already installed YAML package and enforces the complete
allowed structure: push to `dev`, optional manual dispatch, read-only top-level
permissions, one build job, one `github-pages` deploy job, only the two required
write permissions in that deploy job, immutable official action SHAs, and no
other publishing command or permission. Every other workflow and script remains
subject to the existing blanket prohibition.

### Prohibited changes

- No `contents: write`, packages, releases, images, PR mutation, arbitrary
  deployments, wildcard DNS, Cloudflare proxy, global API key, or deploy branch.
- No deploy from pull requests or a branch other than `dev`.
- No rebuild between artifact upload and deployment.

### Tests and evidence

- Policy accepts only the named workflow, permissions, job, environment, action,
  event, and branch.
- Mutation tests reject renamed workflows, extra write permissions, moved
  permissions, broader triggers, altered jobs, publish commands, packages,
  releases, and container pushes.
- The live canary uses bounded retry and checks DNS, TLS, HTTP status,
  canonical URL, `llms.txt`, raw references, the Pagefind index, the deployment
  planner, and a marker that proves internal handoffs are absent.
- Exact-head CI passes before merge.
- GitHub Pages reports the deployed commit and URL.
- DNS returns the intended CNAME chain.
- HTTPS, canonical, redirects, security headers available from the platform,
  sitemap, robots, `llms.txt`, raw contracts, search, planner, keyboard, mobile,
  and no-external-request canaries pass live.

### Rollback

Disable the Pages workflow and remove the exact `fasti` DNS record. Preserve the
source branch and artifact evidence. Do not alter unrelated DNS records.

### Gate status

Engineering and DevEx review cleared. Implementation pending. External writes occur only after local
and pull-request evidence passes.

## Segment 6: Separate Scrobble.dev integration

### User outcome

Scrobble.dev readers can discover Fasti documentation without implying that
Scrobble.dev owns, endorses, or requires Fasti.

### Source of truth

- Live Fasti documentation after Segment 5 passes.
- Scrobble.dev `main` at the exact remote head at implementation time.
- Scrobble.dev repository guidance and public validators.

### Exact files

Resolve in a new clean Scrobble.dev worktree after Fasti is live. The change is
limited to the existing project catalogue source, shared footer source, and the
generated JSON, CSV, OKF, JSON-LD, and human page outputs owned by those sources.

### Dependencies and framework rung

Reuse the existing catalogue and generator. Add no new component, route, or
navigation system.

### Prohibited changes

- Do not edit the current dirty `docusaurus` checkout.
- Do not add Fasti to primary navigation.
- Do not mirror Fasti documentation.
- Do not imply ownership, endorsement, exclusivity, or a required relationship.
- Do not expose the private identifier-resolution initiative.

### Tests and evidence

- `npm install` and `npm run build` in the isolated exact-head worktree.
- `npm run validate:okf` and `npm run validate:public` when present.
- Generated JSON, CSV, OKF, JSON-LD, human catalogue, and footer agree.
- Source and live links resolve.
- Separate pull request, exact-head checks, merge evidence, and production
  canary.

### Rollback

Revert the separate Scrobble.dev commit. Fasti documentation remains separate
and available.

### Gate status

Blocked by Segment 5 live acceptance. This is a dependency, not a reason to
pause earlier implementation.

## Cross-segment failure modes

| Failure | User-visible result | Prevention and recovery |
| --- | --- | --- |
| An internal file enters the public corpus | Private implementation context is exposed | Strict allowlist, path and symlink guards, negative search test, remove artifact and redeploy |
| Status dimensions collapse | Readers mistake a contract for a supported runtime | Separate contract, implementation, runtime, and support fields with source links |
| Generated reference drifts | Integrators copy an operation that does not match source | Exact-head generation, digest manifest, two-run determinism, PR drift gate |
| Planner leaks a secret | Credential can persist in browser or evidence | Web Crypto, memory-only state, redaction and storage tests, reset clears state |
| Pages exception broadens | Repository gains an unrelated publish path | Named-workflow structural policy plus mutation tests |
| DNS is proxied through two CDNs | TLS, cache, or routing becomes ambiguous | DNS-only CNAME and live DNS/TLS canary |
| Scrobble.dev link lands early | Neutral site points to an unavailable service | Fasti live acceptance is a hard dependency |

## NOT in scope

- A supported Fasti release, package, image, installer, or production deployment.
- TrailBase or Loco runtime adoption.
- Atlas runtime deployment.
- Hosted search, analytics, comments, accounts, personalization, or an AI chat
  interface.
- Contract lifecycle changes not required to describe current truth.
- A new Scrobble.dev route, primary-navigation item, or documentation mirror.
- Repair of unrelated worktrees, open PRs, dirty files, or pre-existing CI debt.

## Execution stop conditions

- Exact remote head changes before the branch is ready to publish: fetch,
  classify drift, and update this plan before rebasing or merging.
- An existing worktree owns a file needed by this branch: stop and preserve the
  owner boundary.
- A generated claim conflicts with the capability registry or release evidence:
  keep the claim unavailable and record the conflict.
- Cloudflare or GitHub credentials are broader than the required zone/repository
  scope: do not use them.
- A live canary fails: do not start Scrobble.dev integration.

## Final acceptance

The work is complete only when the Fasti site is live and passes the production
canary, the separate Scrobble.dev change is merged and live, and both deployments
are bound to their exact source commits. Local builds, pull requests, merged
commits, and deployed artifacts are reported as separate states.

## Engineering review record

Review mode: full review

Scope decision: proceed with the complete governed scope. The work spans more
than eight files because the requested outcome has five distinct owners:
content, deterministic projection, presentation, deployment planning, and
publication. Combining those owners would create a less testable boundary.

### Accepted corrections

1. Register `apps/docs` and `packages/deploy-plan` in the strict JavaScript
   workspace inventory.
2. Generate the exact root `v1/problems/*` routes already embedded in canonical
   RFC 9457 problem types.
3. Generate staging before any clean-clone JavaScript build or typecheck.
4. Escape or fence MDX v3 delimiter characters in generated prose.
5. Parse and structurally validate the one Pages workflow. Do not bypass the
   no-publish scanner by filename.
6. Clear planner secret state on reset, navigation, page hide, and back-forward
   restoration. Prove that browser persistence surfaces contain no token.
7. Ignore Docusaurus build outputs so verification does not dirty the worktree.
8. Add a bounded automated live documentation canary.
9. Keep the PR docs verifier fast. Run site compilation, search indexing, axe,
   and Playwright in documentation build and UI/deep gates.

### Rejected or modified outside-voice proposals

- Keep `packages/deploy-plan`. It is an explicit objective and the one reusable
  pure boundary between deployment semantics and React presentation.
- Do not hard-code 29 persona values into JSON Schema. The CSV remains the
  inventory owner; the verifier derives and compares the exact set.
- Do not modify `scripts/check-doc-links.mjs` for root-relative routes. It
  already defers `/`-prefixed routes at lines 60-63. The docs verifier owns
  route existence after projection.
- Do not promise to overwrite or zero the user's clipboard. Clear application
  state and prove no browser persistence instead.
- Keep STE checks deterministic and termbase-driven. Do not add an NLP parser.

### Test-flow map

```text
site.yaml/personas.yaml
  -> schema + exact CSV parity + route confinement
  -> two isolated projections compare byte-for-byte
  -> root problem URLs and raw references resolve
  -> Docusaurus compiles the clean projection
  -> Pagefind indexes only the projection
  -> Playwright/axe exercise navigation, search, status, and planner states
  -> Pages uploads that exact directory
  -> live canary checks DNS, TLS, routes, search, and absence markers
```

Every conditional branch in the new Rust and TypeScript code requires a unit or
mutation check. The common reader, search, and planner journeys require browser
tests. Deployment and DNS require live canaries because mocks cannot prove
those boundaries.

### Completion summary

- Scope challenge: scope accepted as-is.
- Architecture review: 3 issues found and folded into the plan.
- Code quality review: 2 issues found and folded into the plan.
- Test review: flow diagram produced; 3 gaps found and folded into the plan.
- Performance review: 1 issue found and folded into the plan.
- Critical failure gaps remaining: 0.
- Unresolved decisions: 0.
- Outside voice: Antigravity CLI plan review ran; 9 verified corrections were
  accepted or adapted.
- Parallelization model: five implementation lanes; source-model and projection
  work precede three parallel presentation, planner, and policy lanes; live
  deployment and Scrobble.dev integration stay sequential.
- Lake score: 9 of 9 accepted recommendations retain the complete requested
  outcome.

## Developer-experience review record

Review mode: DX POLISH

Product type: documentation portal for a local daemon, CLI, HTTP API, generated
TypeScript SDK, and extension contracts

### Developer persona card

```text
TARGET DEVELOPER PERSONA
========================
Who:       An integration developer building a client, import, player adapter,
           automation, or provider boundary against Fasti.
Context:   They are evaluating a pre-release local system and need to separate
           executable behavior from contract, fixture, and future-body claims.
Tolerance: Five minutes to a truthful first result; fifteen minutes to a first
           governed observation path before they stop and open an issue.
Expects:   Copyable commands, exact outputs, typed contracts, clear auth and
           idempotency rules, local operation, recovery guidance, and source links.
```

The user, operator, extension-author, contributor, AuDHD, and screen-reader
personas remain first-class. The integration developer is the primary developer
lens for API, CLI, SDK, and reference decisions.

### Developer perspective

I open the repository and see a two-command health quick start. It is clear that
Fasti is a system of record and not a player. I can run `cargo run --locked -p
fastid`, but the first cold build may take longer than the stated warm path. The
health response proves that a daemon answers; it does not prove durable setup,
authentication, or observation acceptance. To understand that difference, I
must move between the current-status table, the development section, the SDK
README, the network guide, the capability ledger, generated OpenAPI, and the
problem catalogue.

I can find `observation.accept` and the SDK bootstrap methods, but I must assemble
the safe order myself: choose a private data root, stay on loopback, initialize
once, retain the one-time proof safely, enroll a scoped client, submit an
observation, and inspect the typed result. The repository is honest, but the
knowledge is distributed. A failed request gives a governed problem type, yet
the canonical URL does not currently open a human recovery page. I trust the
project more because it refuses false release claims. I will make progress faster
when one task page joins the exact commands, output, safety boundary, recovery,
and next task without making me remember which document owns each fact.

### Competitive DX benchmark

No comparable source publishes a verified time-to-hello-world measurement, so
the plan does not invent one.

| Reference | Observed onboarding choice | Measured TTHW | Plan response |
| --- | --- | --- | --- |
| Docusaurus 3.10.2 | Static build, versioned docs, GitHub Pages support | Not published | Use it only as the presentation layer |
| Cinephage reference supplied by the user | Task-led IA and a custom local deployment tool | Not published | Recreate the useful pattern with Fasti truth and provenance |
| Current Fasti README | Clone, run daemon, verify exact health JSON | About 2 minutes warm; cold build longer | Preserve this as the first truthful result |
| Planned Fasti portal | Choose path, run health, then follow one bounded integration task | Under 5 minutes to health; under 15 minutes to first observation | Competitive target within local-build constraints |

Target tier: competitive. A hosted sandbox is not the selected shortcut because
it would add an operated service and could blur Fasti's unsupported-release
boundary.

### Magical moment specification

The developer sees that one capability has one traceable story: current runtime
state, required scope, exact HTTP and SDK shapes, copyable local command, typed
result, canonical problem recovery, contract sources, and release/support state.

Delivery vehicle: a task-led copy-paste local guide backed by a generated
capability page. The page opens with `observation.accept`, but it makes the
health-only result available first. It shows expected output after each command,
stops before any secret would enter shell history, and links to the next safe
host-side step. The local planner is a later operational aid, not a substitute
for the developer quick start.

### Developer journey map

| Stage | Developer does | Current friction | Planned resolution | Status |
| --- | --- | --- | --- | --- |
| Discover | Reads purpose and current status | Product, contract, runtime, and support state require careful cross-reading | Path chooser plus separate status dimensions with sources | Addressed |
| Install | Clones source and runs `fastid` | Cold build time and tool prerequisites | Exact prerequisites, warm/cold expectation, native and container alternatives | Addressed |
| Hello world | Calls `/api/v1/health` | A green health route can be over-read as durable readiness | Page states exactly what health proves and what it does not prove | Addressed |
| Real usage | Initializes local state and submits an observation | Safe bootstrap, auth, scope, and SDK sequence is distributed | One task page with exact sequence, expected output, redaction, and recovery | Addressed |
| Debug | Follows an RFC 9457 problem type | Canonical `v1/problems/*` URLs have no public human page | Generate every exact problem route with cause, safe state, and next action | Addressed |
| Upgrade | Looks for release migration guidance | There is no supported release or upgrade contract | State unavailable truthfully; link source changes and the B8 gate | Addressed without false promise |

### First-time developer confusion report

```text
FIRST-TIME DEVELOPER REPORT
===========================
Persona: Integration developer
Attempting: Verify Fasti, then find the first real observation path

T+0:00  Opens README Quick start and starts the locked daemon.
T+0:45  Sees a cold Rust build. The warm two-minute estimate does not apply yet.
T+2:00  Receives the exact health JSON. Initially assumes the API is ready.
T+2:30  Reads Current status and learns that no data root means health only.
T+4:00  Finds durable setup and one-time credential warnings in Development.
T+6:00  Moves to the SDK README to reconstruct browser versus integration auth.
T+8:00  Finds observation.accept in the capability ledger but still needs the
        exact end-to-end task order and recovery pages.
T+10:00 Understands the architecture, but has not yet submitted an observation.
```

All confusion points are in scope for the portal. The plan does not hide cold
build time, create a hosted demo, or simplify credential boundaries.

### Eight-pass DX scorecard

| Dimension | Current | Planned | Evidence and acceptance |
| --- | ---: | ---: | --- |
| Getting Started | 5/10 | 9/10 | One truthful path under five minutes warm; cold builds remain an honest constraint |
| API, CLI, and SDK | 6/10 | 9/10 | One capability view joins exact names, scope, transport, SDK, examples, and support state |
| Error messages | 5/10 | 10/10 | Every canonical problem URL resolves to cause, safe state, retry rule, and next action |
| Documentation | 4/10 | 10/10 | Outcome tracks, progressive disclosure, local search, raw references, provenance, and recovery |
| Upgrade path | 3/10 | 8/10 | Truthful unavailable state and governing B8 gate; no fake migration guide before a release exists |
| Developer environment | 7/10 | 9/10 | Existing locked native/container paths preserved and connected; no new runtime service |
| Community | 6/10 | 9/10 | Contribution task, exact gates, issue and sponsor boundaries, edit links, and source evidence |
| DX measurement | 2/10 | 8/10 | CI timing, broken-path tests, live canary, and issue feedback without runtime telemetry |

Overall: 4.8/10 current, 9.0/10 planned.

### DX implementation checklist

- [ ] A reader can choose a persona path within one viewport.
- [ ] Warm time to verified health is under five minutes and has exact output.
- [ ] Cold-build expectations and prerequisites are visible before the command.
- [ ] The first observation task has copyable commands, exact results, safe
  secret transitions, verification, and recovery.
- [ ] Every error page states problem, cause, safe state, recovery, and source.
- [ ] API, CLI, SDK, event, schema, JSON-LD, OKF, and problem names match their
  owners exactly.
- [ ] Local search covers only allowlisted pages and makes no external request.
- [ ] Examples use real supported or explicitly labeled fixture paths.
- [ ] Unsupported upgrade and production paths remain visible with exact gates.
- [ ] Contributor docs name the single canonical PR command and clean-tree
  receipt rule.
- [ ] Edit and issue links carry the exact public source path.
- [ ] CI records docs generation and build duration without collecting reader
  telemetry.
- [ ] The live canary reports which public task or raw route failed.

### DX implementation tasks

- [ ] **DX1 (P1)** - Write the health-to-first-observation task with exact output,
  secret-safe transitions, verification, and recovery.
- [ ] **DX2 (P1)** - Generate root problem pages from canonical problem contracts.
- [ ] **DX3 (P1)** - Generate the capability status view with separate contract,
  implementation, runtime, and support dimensions.
- [ ] **DX4 (P1)** - Make local search keyboard-operable and prove that internal
  content is absent.
- [ ] **DX5 (P1)** - Give every persona a public first task and recovery task.
- [ ] **DX6 (P2)** - Record clean-clone, warm, and cold documentation build timing
  in CI evidence without adding analytics.
- [ ] **DX7 (P2)** - Give unavailable production and upgrade paths a precise gate,
  owner, safe alternative, and next review condition.
- [ ] **DX8 (P2)** - Add source-edit, issue, accessibility, security, and support
  links at the point where a developer needs them.

Unresolved DX decisions: none. The user authorized the recommended option at
workflow gates, so the review selected the integration-developer persona,
competitive TTHW target, copy-paste local magical moment, and DX POLISH mode.

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
| --- | --- | --- | ---: | --- | --- |
| CEO Review | `/plan-ceo-review` | Scope and strategy | 0 | Not run | The objective and architecture were already fixed by the governing brief |
| Codex Review | `/codex review` | Independent second opinion | 0 | Not run | Antigravity outside voice was used instead |
| Eng Review | `/plan-eng-review` | Architecture and tests | 1 | CLEAR | 9 issues folded into the plan; 0 critical gaps remain |
| Design Review | `/plan-design-review` | UI and UX gaps | 0 | Pending implementation | Impeccable finish review and manual UX matrix are required before deployment |
| DX Review | `/plan-devex-review` | Developer experience gaps | 1 | CLEAR | Score 4.8/10 to planned 9.0/10; TTHW 8-15 minutes to under 5 minutes for health and under 15 for observation |

**CROSS-MODEL:** Antigravity challenged the plan; verified findings were folded into the engineering review, while proposals that duplicated owners or contradicted the explicit deploy-plan boundary were rejected.

**VERDICT:** ENG + DX CLEARED - implementation may start. Design, QA, security, deployment, and live canary gates remain open.

NO UNRESOLVED DECISIONS
