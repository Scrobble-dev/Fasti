# ADR-0005: Keep the Fasti core small and use TrailBase for human identity

- Status: Amended; framework boundary accepted and TrailBase selection final
- Date: 2026-08-29
- Public contract: Unchanged; no route, event, schema, SDK method, CLI command, or permission is activated
- Reviewed references: Loco 1.1.0 and TrailBase 0.33.5

The approved
[TrailBase authentication programme](../plans/trailbase-authentication-remediation.md)
supersedes this ADR's earlier optional-issuer, local-account, migration, and
rollback proposal. The earlier rationale remains below where it still explains
the Loco and bounded-context decisions.

## Context

Fasti already has a Rust application core, Axum delivery, a controlled SQLite
writer, explicit SQL, generated contracts, a native daemon, a desktop review
host, and native plus OCI development paths. An earlier PR branch also proposed
local browser accounts and active browser sessions. That proposal is
superseded. The current foundation has scoped API credentials, profile grants,
object authorization, and a dormant Fasti browser-session model.

Loco supplies useful defaults for a new Axum application. Its main value is the
pre-wired application context, route inventory, generators, SeaORM model and
migration flow, authentication scaffold, task runner, and background workers.
TrailBase supplies password and social sign-in, email verification and reset,
short-lived signed access tokens, stateful refresh tokens, PKCE, an optional
authentication UI, an admin UI, migrations, generated clients, and a local MCP
server.

Both projects can reduce work in a new product. Fasti is not a new product shell
anymore. Replacing the current ownership model would add a second composition
root, database model, migration model, authorization model, and contract source.
That would increase the number of ways an agent can make a change without
following Fasti's existing rules.

## Decision

1. Keep Axum, Tokio, rusqlite, and the current application boundaries as the
   Fasti runtime.
2. Do not adopt Loco as the Fasti application framework.
3. Use Loco as a reference for developer-experience patterns, not as a runtime
   dependency.
4. Use TrailBase as the selected separate, private human-account platform.
5. Do not use TrailBase Record APIs, realtime APIs, migrations, ACLs, or SQLite
   database as Fasti domain storage.
6. Remove the proposed local `BrowserUser`, password, and development-account
   path. It is not a compatibility surface.
7. Activate Fasti browser sessions only through C1 after the TrailBase
   exchange, subject anchor, membership, role, administrator continuity, and
   session-issuance gates pass. Current local C1 source uses only the exact
   direct `127.0.0.1:8420` listener; package and delivery evidence remains
   pending.
8. Add no second broad application framework beside TrailBase. Named external
   identity integrations must remain bounded adapters under their approved
   package plans.

## Why Loco is not the core framework

Loco can mount an existing Axum router. That does not remove the main overlap.
Its application context, SeaORM entities, generated migrations, authentication,
queue configuration, and task lifecycle would sit beside systems that Fasti
already owns.

The largest conflict is persistence. Fasti must commit identity, Chronicle
state, receipts, changes, and review work through one controlled SQLite writer.
The transaction boundary is part of the product. A generated active-record path
or a second database pool cannot become another way to mutate those tables.

The useful Loco lessons are smaller:

- one command should show routes, configuration, and health;
- generators should create a complete feature slice, not isolated files;
- background work should have one interface and an explicit durability mode;
- configuration errors should fail before the server accepts traffic;
- local setup should include a doctor command and clear repair actions.

Fasti can add those behaviors to `xtask`, `fasti`, and `scripts/dev.sh` without
changing its runtime framework.

## TrailBase boundary

TrailBase owns only the human-account functions proven by the selected release
and its current official evidence, including where supported:

- password and social sign-in;
- email verification and password reset;
- account profile and account deletion;
- access-token and refresh-token issuance;
- browser or native sign-in through the documented authorization-code and PKCE
  flow.

Fasti continues to own:

- the durable `issuer + subject` binding;
- opaque Fasti browser sessions and their inventory and revocation;
- workspaces and profiles;
- roles and administrator state;
- devices, clients, credentials, and grants;
- capability scopes and object authorization;
- audit, receipts, Chronicle, identity, metadata, and portability;
- local automation and Nuvio-specific delegated authorization.

The systems communicate through a versioned adapter. They do not share tables.
A short-lived TrailBase proof identifies a subject. It does not grant a Fasti
scope. Fasti validates the proof through the documented TrailBase boundary and
then checks the current Fasti subject state, membership, role, profile,
grant, scope, and epoch state before it issues an opaque Fasti browser session.

A short-lived proof can remain valid until it expires after related upstream
state changes. Its lifetime and validation path are explicit Fasti security
policy and test conditions. TrailBase refresh sessions remain TrailBase state.
Fasti stores neither their plaintext value nor TrailBase database rows. Secrets
do not enter Fasti logs, URLs, exports, screenshots, or ordinary browser
storage.

Do not assume a TrailBase function until the selected release's current
documentation and source prove it. Passkeys, recovery codes, OpenID Provider
discovery, and a TV device-authorization grant are not part of the TrailBase
boundary without that evidence. Their approved Fasti packages keep the methods
visible and unavailable until their own server-owned capability passes.

## TrailBase integration and activation gates

TrailBase selection is final. Production activation is blocked until one exact,
pinned TrailBase release passes all applicable checks:

1. Password and one social sign-in complete through the real browser flow.
2. Desktop/mobile authorization-code and PKCE complete without putting tokens in
   URLs or local storage.
3. Fasti validates the exact proof through TrailBase's documented public trust
   boundary and rejects substitution, replay, expiry, and subject confusion.
4. A TrailBase subject maps to one Fasti subject without using a TrailBase row ID
   as a Fasti primary key.
5. Revocation and disabled-account behavior are bounded by a documented access
   token lifetime.
6. No local password, `BrowserUser`, development account, or compatibility
   layer is present.
7. Rollback disables the TrailBase exchange and production Fasti session path
   without changing Fasti records or Chronicle state.
8. Native, desktop, and OCI packaging keep data and secrets in explicit durable
   locations.
9. TrailBase outage behavior for an already issued Fasti session is explicit,
   bounded, and tested.
10. Backup and restore state clearly which identity data is included and how a
    restored node regains administrator access.
11. Startup, idle memory, failure isolation, upgrade, and downgrade stay within
    the repository budgets.
12. Security review covers token substitution, issuer confusion, key rotation,
    CSRF, redirect validation, account takeover, refresh replay, and log
    redaction.

A failed gate blocks production activation. It does not reopen the platform
decision or trigger a second Fasti authentication implementation.

## Local development modes

| Goal | Command or shape | Rule |
| --- | --- | --- |
| Fast Rust and web iteration | `./scripts/dev.sh` | Native daemon and Vite. An already initialized, verified TrailBase root starts beside Fasti; the exact direct listener mounts C1. The launcher never initializes TrailBase. |
| Trusted desktop review | `FASTI_DATA_ROOT=/private/path ./scripts/dev.sh --desktop` | One Tauri host with the embedded local kernel. An already initialized, verified TrailBase root starts beside it; package evidence remains pending. |
| Daemon/CLI container proof | `docker build .` | Produces the runtime-equivalent default image. Docker requires BuildKit; Podman/Buildah must keep unused-stage pruning enabled. |
| One-container product review | `docker build --target local -t fasti:local .` | Adds the pre-built Workbench to `fastid`; remains review-only until release gates pass. |
| TrailBase integration package | Explicit pinned profile | Starts TrailBase beside Fasti as a separate process with a separate data directory and clear health state. Verified activation can issue C1 sessions only on the exact direct listener; other topologies omit the routes. |

Docker's deprecated legacy builder is not supported. The Dockerfile contains a
modern-builder feature gate, so CI and local builds fail instead of silently
executing the unrelated web stage. Docker Engine 23 and later use BuildKit by
default. Current Podman/Buildah also skip unused stages by default.

The normal edit loop stays native. Containers prove packaging and give users a
one-command review path. They are not a substitute for incremental compilation
and hot reload.

## Agent change rules

Before an agent adds a framework, service, generator, queue, database, or auth
library, it must show all of the following:

1. The existing owner cannot meet the requirement with a smaller change.
2. The dependency removes more duplicate code or operational work than it adds.
3. It does not create a second path to write Fasti state.
4. It preserves offline use, native packaging, OCI packaging, memory limits, and
   rollback.
5. It has one named capability, one application owner, one contract disposition,
   and one negative test that proves the guard can fail.
6. It updates the relevant architecture decision before production code lands.

A new feature still follows this order:

```text
user outcome
  -> domain rule
  -> application capability and authorization
  -> contract and fixtures
  -> storage or network adapter
  -> API, CLI, desktop, or web presentation
```

A generated scaffold cannot skip a layer. An admin dashboard cannot make an
unreviewed production schema change. A provider or identity adapter cannot write
directly to Chronicle or identity tables.

## Consequences

- Fasti does not receive Loco's generators automatically.
- Contributors keep one runtime and one persistence model.
- TrailBase owns proven human-account flows without becoming Fasti's
  application backend.
- The proposed local account path is removed. C1 now implements the final Fasti
  session model in local source without making a package, merge, or release
  claim.
- Future agents have a fail-closed repository check against accidental Loco
  adoption and unpinned Docker bases.
- OpenAPI, AsyncAPI, JSON Schema, JSON-LD, SDK, and CLI outputs remain unchanged
  because this decision activates no capability.

## References

- [Fasti authentication boundaries](authentication.md)
- [Fasti local development loop](../dev-loop.md)
- [Loco: coming from Axum](https://loco.rs/docs/explanation/coming-from-axum/)
- [Loco: add a model](https://loco.rs/docs/how-to/add-model/)
- [TrailBase authentication](https://trailbase.io/documentation/auth/)
- [TrailBase installation and local MCP](https://trailbase.io/getting-started/install/)
- [TrailBase migrations](https://trailbase.io/documentation/migrations/)
