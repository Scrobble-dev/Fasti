# Production integrations

Status: Draft implementation vehicle.

This branch tracks the production connection work for NuvioTV, Plex and Tautulli, Jellyfin and Emby, and the desktop MPRIS observer.

The existing Connections interface must remain. Mock or static states must be replaced with runtime-backed states; useful interface content must not be removed to hide unfinished behavior.

The implementation must use the existing authenticated observation service, scoped client credentials, durable idempotency, profile isolation, bounded inputs and queues, redacted diagnostics, offline-safe delivery, generated OpenAPI and AsyncAPI contracts, and repository QA gates.

Tabler is the first-choice design system. CI must reject competing UI systems unless a narrow documented exception exists.

This initial commit records scope only. It does not claim that any production adapter is implemented. Keep the pull request as a draft until code, tests, contracts, documentation, security review, UI QA, and exact-head checks pass.
