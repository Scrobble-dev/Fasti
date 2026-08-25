# Security Policy

Fasti is currently a development source tree. No version is supported for production use and no patched public build is promised yet. Security reports are still welcome because the project is defining identity, evidence, access, recovery, and local distribution boundaries where mistakes would be costly later.

## Reporting a vulnerability

Do not open a public issue for an undisclosed vulnerability.

Report privately through [GitHub Security Advisories](https://github.com/Scrobble-dev/Fasti/security/advisories/new) or email `security@scrobble.dev`. Include the affected commit or artifact, impact, reproduction steps or a minimal proof, and any suggested mitigation. Do not include real personal media history, credentials, tokens, or private provider data when a synthetic fixture can reproduce the issue.

The project will acknowledge and investigate reports as maintainer availability permits, keep the reporter informed, and agree on disclosure timing where coordination is appropriate. This development-stage policy does not promise a fixed response SLA or a published patched binary.

## Implemented review controls

- Native `fastid` binds to `127.0.0.1:8420` unless `FASTI_LISTEN` is set to an explicit `IP:PORT` value.
- The local OCI image deliberately binds to `0.0.0.0:8420`, runs as the non-root `fasti` user, and requires the operator to publish a host port.
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
- Tauri connection tests accept only HTTP or HTTPS origins, reject credentials and URL suffixes, disable redirects, use platform certificate trust, and bound time and response bytes.
- Google Books search runs only in the trusted Tauri host. Credentials use environment secrets or platform credential storage and never return to the webview.
- Provider access composes declaration maximums with operator allow and deny settings. Deny wins. DNS answers are classified before connection, redirects are disabled, and private, loopback, link-local, multicast, unspecified, and documentation networks are outside the current declaration.
- Android uses its application data directory, Android Keystore-backed credential storage, an explicit network-security configuration, and no certificate-validation bypass.

These controls make the development baseline and B2 review implementation safer. Production mounts only the durable loopback setup slice when `FASTI_DATA_ROOT` is explicit. This does not make Fasti a supported service.

## Current threat model

The current source tree protects these assets:

- stable local identity and media history;
- original observations and evidence;
- profile, client, credential, grant, and receipt integrity;
- generated contract meaning;
- benchmark and hardware qualification evidence;
- private runner bundles and CI results.

Treat unauthenticated network clients, hostile local processes, provider input, archives, paths, structured data, pull requests, dependencies, actions, base images, and concurrent file changes as untrusted.

Current trust boundaries are the production loopback listener, the feature-gated conformance listener, Tauri IPC, provider DNS and HTTP responses, delivery adapters into application policy, source and CI into generated artifacts, and runner files into exact-commit bundles. Native, OCI, desktop, and Android source builds must expose the same governed behavior.

The system must fail closed when authorization, durability, limits, source identity, evidence, or hardware identity is missing or stale. Missing behavior must not return a success receipt. Provider data must not become canonical identity. Secrets must not enter URLs, arguments, logs, screenshots, fixtures, or proof bundles.

## Remaining proof obligations

B2-B8 must still prove, rather than merely document:

- first-client enrollment, closed bootstrap, rotation, revocation, expiry, current-epoch authorization, and profile isolation under process-crash, restart, concurrency, and supported physical-storage tests;
- strict body, stream, byte, temporary-space, archive, and concurrency limits before expensive work;
- streamed evidence hashing, same-filesystem durable promotion, orphan quotas, and safe cleanup;
- SQLite durability settings verified by readback, bounded writer transactions, receipt replay, and crash or controlled power-cut survival;
- archive traversal, decompression-bomb, Unicode/path ambiguity, SQL injection, header confusion, stale-credential, SSRF, and hostile-JSON defenses;
- secrets external to images and proof bundles, permission-restricted credential delivery, and no credentials in command arguments or logs;
- device-level Android runtime tests for custom origins, `.internal` DNS, system and explicitly bundled private CAs, Keystore persistence, process restart, and network-namespace guidance;
- authenticated browser routes before the ordinary web build can manage provider credentials or search external providers;
- dependency, SBOM, signing, trust-root, update, and recovery evidence before B8 publishes a release.

Provider adapters and metadata enrichment cannot bypass application authorization, write storage directly, or make a provider canonical. A supported network-exposure guide cannot exist until the access and threat-model gates pass.

See [the constitution](docs/constitution.md), [capability ledger](docs/capability-ledger.md), and [Definition of Done](docs/definition-of-done.md).
