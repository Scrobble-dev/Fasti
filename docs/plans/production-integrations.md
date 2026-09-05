# Production integrations

Status: Plex, Tautulli, Jellyfin, and Emby webhook adapters and the NuvioTV
endpoint are implemented and covered by the governed B1 contract spine. The
desktop MPRIS observer and the upstream Nuvio client-side provider remain
separate, not-yet-implemented work; see `docs/integrations/nuvio.md` and
`docs/integrations/media-server-webhooks.md` for what is actually live.

The existing Connections interface must remain. Mock or static states must be replaced with runtime-backed states; useful interface content must not be removed to hide unfinished behavior.

Production integration adapters must treat provider identifiers (IMDb, TMDB, TVDB, etc.) only as typed evidence/claims attached to provider-neutral Fasti Records, must use the existing governed observation service, and must not create canonical identity, merge or split Records, or redefine domain/identity rules.

The implementation must use the existing authenticated observation service, scoped client credentials, durable idempotency, profile isolation, bounded inputs and queues, redacted diagnostics, offline-safe delivery, and repository QA gates. Generated contract surfaces (OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD, SDK and CLI surfaces, permissions, typed problems, examples, and documentation) are required per capability as declared in `contracts/registry/v1/capabilities.yaml`, not uniformly for every integration surface — for example `b1_integration_status` marks JSON-LD not applicable and defers CLI and SDK bindings to B2. Authoritative schemas are the sources of truth; generated artifacts (SDKs, docs, etc.) are outputs derived from them.

Tabler is the first-choice design system. CI must reject competing UI systems unless a narrow documented exception exists.

Keep the pull request as a draft until code, tests, contracts, documentation, security review, UI QA, and exact-head checks pass for the remaining scope (desktop MPRIS observation and the upstream Nuvio client).
