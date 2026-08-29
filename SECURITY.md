# Security Policy

Fasti is currently a development source tree. No version is supported for production use and no patched public build is promised yet. Security reports are still welcome because the project is defining identity, evidence, access, recovery, and local distribution boundaries where mistakes would be costly later.

## Reporting a vulnerability

Do not open a public issue for an undisclosed vulnerability.

Report privately through [GitHub Security Advisories](https://github.com/Scrobble-dev/Fasti/security/advisories/new) or email `security@scrobble.dev`. Include the affected commit or artifact, impact, reproduction steps or a minimal proof, and any suggested mitigation. Do not include real personal media history, credentials, tokens, or private provider data when a synthetic fixture can reproduce the issue.

The project will acknowledge and investigate reports as maintainer availability permits, keep the reporter informed, and agree on disclosure timing where coordination is appropriate. This development-stage policy does not promise a fixed response SLA or a published patched binary.

## Implemented review controls

- Native `fastid` binds to `127.0.0.1:8420` unless `FASTI_LISTEN` is set to an explicit `IP:PORT` value. Automatic collision recovery stays on the requested loopback address. It never moves a public or wildcard listener.
- Client and public origins reject credentials, paths, queries, and fragments. Non-loopback origins require HTTPS and platform certificate validation.
- Non-loopback durable routes require an explicit data root, `FASTI_REMOTE_TRUSTED_PROXY=true`, and an absolute HTTPS `FASTI_PUBLIC_URL`. Remote bootstrap routes remain absent.
- PR A keeps only a dormant Fasti browser-session foundation. It does not expose production human-account, sign-in, session-issuance, inventory, or revocation routes. The earlier PR #93 `BrowserUser`, local password, development account, custom TOTP, WebAuthn-shaped, backup-code, and fabricated OpenID Connect paths are superseded and cannot be used as security controls.
- The dormant session design requires an opaque random secret, digest-only storage, an exact opaque public session identifier, idle and absolute expiry, rotation, bounded activity updates, Origin and Host checks, strict cross-site request forgery protection, and session-local selection of an existing authorized profile grant. These are PR A implementation and test obligations, not proof of an active production session.
- Production browser authentication remains `Unavailable` until C1 proves TrailBase exchange, the durable TrailBase anchor, membership and role authorization, administrator continuity, session issuance, and exact-head negative controls. TrailBase runs as a separate pinned service and never authorizes a Fasti operation without Fasti application authorization.
- The local OCI image deliberately binds to `0.0.0.0:8420`, runs as the non-root `fasti` user, and requires the operator to publish a host port. Durable routes remain disabled unless a detected container boundary and `FASTI_EXTERNAL_BIND_IP` explicitly establish the outer loopback-only port forward. Native wildcard listeners cannot replay that assertion.
- Repository automation has read-only contents permission and cannot log in to GHCR, push images or attestations, publish packages, or create GitHub Releases.
- The event-submission route is absent rather than returning an unauthenticated false committed receipt.
- Planned export, restore, and verify commands exit nonzero and change no data.
- The active source contains no analytics or phone-home implementation.
- The B2 review kernel opens owner-only data directories and files, requires SQLite foreign keys, WAL, `synchronous=FULL`, and a bounded busy timeout, and refuses unsupported settings.
- Node initialization and first-client enrollment are transactional. The one-time proof expires, is consumed once, and is cleared after successful enrollment. Long-lived credential material is stored as a digest rather than plaintext.
- Credential authentication binds the selected profile, client, grant, credential epoch, and workspace. Ambiguous active grants, ungranted profiles, revoked state, and cross-workspace grants fail closed.
- Evidence upload authorizes before temporary-file creation, enforces concurrent and byte budgets, hashes while streaming, rechecks authorization before durable promotion, and verifies existing content before deduplication.
- Operation receipts bind workspace, client, operation, capability, and semantic digest. Replay and receipt streams remain profile/client scoped and bounded.
- Durable setup publishes `already_initialized`, `bootstrap_closed`, `integrity_failed`, and `storage_unavailable`. Authentication, cursor, evidence, identity, and review failures remain staged until their public routes activate.
- `cargo-deny` (`deny.toml`) gates the main workspace's dependency licenses, advisories, and sources in CI; a documented allowlist keeps every dependency compatible with distributing Fasti under AGPL-3.0-or-later as a dependency, not a derivative.

These controls make the development baseline and B2 review implementation safer. Durable local routes require an explicit data root and direct loopback or an explicitly declared loopback-only container port forward. The authenticated remote subset excludes human-account and browser-session routes until C1 activates them with explicit trusted-proxy and HTTPS-origin evidence. This does not make Fasti a supported service.

## Temporary dependency exception

The desktop crate inherits `glib 0.18.5` and `RUSTSEC-2024-0429` from Tauri 2's GTK3 stack. Fasti does not depend on `glib` directly. The desktop lockfile audit ignores only this advisory and still fails for every other advisory. Remove the exception when the [upstream GTK4 migration](https://github.com/tauri-apps/tauri-docs/issues/3143) is available.

## Security Assurance Case

Fasti provides a formal security assurance case structured around four core pillars:

1. **Threat Model & Protected Assets**: Local identity integrity, raw immutable observations, authorization grants, deterministic receipts, and private runner credentials.
2. **Trust Boundaries & Mediation**: Strict boundary between untrusted network/IPC inputs and the domain kernel. The production daemon binds loopback by default with link-local SSRF guards.
3. **Secure Design Principles**: Fail-closed authorization, economy of mechanism, complete mediation, least privilege, and zero runtime telemetry.
4. **Common Weakness Mitigations**: Parameterized SQLite queries (anti-SQLi), 100% safe Rust in Fasti application code (anti-buffer overflow / memory corruption; note that fasti-store's bundled SQLite and zstd-sys dependencies introduce native C/FFI boundaries), bounded request/archive sizes (anti-DoS / decompression bombs), and strict JSON Schema 2020-12 allowlists.

### Current Threat Model

The current source tree protects these assets:

- stable local identity and media history;
- original observations and evidence;
- profile, client, credential, grant, and receipt integrity;
- generated contract meaning;
- benchmark and hardware qualification evidence;
- private runner bundles and CI results.

Treat unauthenticated network clients, hostile local processes, provider input, archives, paths, structured data, pull requests, dependencies, actions, base images, and concurrent file changes as untrusted.

Current trust boundaries are the production loopback listener, the feature-gated conformance listener, delivery adapters into application policy, source and CI into generated artifacts, and runner files into exact-commit bundles. Native, OCI, and later package formats must expose the same governed behavior.

The system must fail closed when authorization, durability, limits, source identity, evidence, or hardware identity is missing or stale. Missing behavior must not return a success receipt. Provider data must not become canonical identity. Secrets must not enter URLs, arguments, logs, screenshots, fixtures, or proof bundles.

## Remaining proof obligations

B2-B8 must still prove, rather than merely document:

- PR A's dormant session migration, deterministic fixtures, restart, expiry, rotation, fixation, exact identifier, Origin, Host, cross-site request forgery, concurrent revocation, and profile-isolation behavior without mounting a production browser-session route;
- C1's pinned TrailBase exchange, trust root, account lifecycle, subject anchor, membership, administrator continuity, browser binding, production session issuance, refresh-session cleanup, outage behavior, and clone fencing before browser authentication becomes available;
- first-client enrollment, closed bootstrap, rotation, revocation, expiry, current-epoch authorization, and profile isolation under process-crash, restart, concurrency, and supported physical-storage tests;
- strict body, stream, byte, temporary-space, archive, and concurrency limits before expensive work;
- streamed evidence hashing, same-filesystem durable promotion, orphan quotas, and safe cleanup;
- SQLite durability settings verified by readback, bounded writer transactions, receipt replay, and crash or controlled power-cut survival;
- archive traversal, decompression-bomb, Unicode/path ambiguity, SQL injection, header confusion, stale-credential, SSRF, and hostile-JSON defenses;
- secrets external to images and proof bundles, permission-restricted credential delivery, and no credentials in command arguments or logs;
- browser origin/proxy allow-list hardening and hostile cross-origin deployment evidence before a supported remote release;
- dependency license compliance and SBOM generation, now covered in CI by `cargo-deny` and `cargo-cyclonedx`/`cyclonedx-npm` (`.github/workflows/release.yml`, `deny.toml`) — see [B8b release readiness](docs/architecture/b8b-release-readiness.md);
- signing, trust-root, update, and recovery evidence, still deferred to B8's own release action.

Provider adapters and metadata enrichment cannot bypass application authorization, write storage directly, or make a provider canonical. The documented Remote recipe is development-only; a supported production exposure guide still requires the remaining threat-model and release gates.

Provider and other governed outbound adapters must evaluate the shared application policy after DNS resolution. Provider declarations are maximum grants; operator allow lists only narrow them and deny rules win. Adapters must reject redirects, system proxies, empty DNS results, any unsafe answer, and DNS rebinding. Provider secrets use request headers or platform credential stores, never URLs or browser storage.

See [the constitution](docs/constitution.md), [capability ledger](docs/capability-ledger.md), and [Definition of Done](docs/definition-of-done.md).
