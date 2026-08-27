# Production integrations

Status: Plex, Tautulli, Jellyfin, and Emby webhook adapters and the NuvioTV
endpoint are implemented and covered by the governed B1 contract spine. The
desktop MPRIS observer and the upstream Nuvio client-side provider remain
separate, not-yet-implemented work; see `docs/integrations/nuvio.md` and
`docs/integrations/media-server-webhooks.md` for what is actually live.

The existing Connections interface must remain. Mock or static states must be replaced with runtime-backed states; useful interface content must not be removed to hide unfinished behavior.

The implementation must use the existing authenticated observation service, scoped client credentials, durable idempotency, profile isolation, bounded inputs and queues, redacted diagnostics, offline-safe delivery, generated OpenAPI and AsyncAPI contracts, and repository QA gates.

Tabler is the first-choice design system. CI must reject competing UI systems unless a narrow documented exception exists.

Keep the pull request as a draft until code, tests, contracts, documentation, security review, UI QA, and exact-head checks pass for the remaining scope (desktop MPRIS observation and the upstream Nuvio client).
