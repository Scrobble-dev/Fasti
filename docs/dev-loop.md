# Local dev loop

This page tells you how to start Fasti for local work, how to check that it
works, and how to update it. It uses short sentences. Each sentence states
one fact.

This page does not describe a public release. See the status table in
[README.md](../README.md) for what is and is not released. The web
workbench described below is B4 review-only. It is not mounted by the
production daemon. It is not a release claim.

## Invoke

Run this command from the repository root:

```bash
./scripts/dev.sh
```

This command builds and starts `fastid`, the production daemon. If this
worktree already has an initialized TrailBase root, the launcher starts and
verifies that service first. It never initializes TrailBase. It waits until the
health route answers and the durable initialization route returns its real
authorization response. Then it prints the API URL.

`fastid` prefers `http://127.0.0.1:8420`. By default, startup fails if that
port is already taken. Set `FASTI_PORT_FALLBACK=auto` to have `fastid` ask
the operating system for a free port instead. In that case, the script
prints the port it actually got. A fallback listener omits all C1 browser-
authentication routes. Automatic fallback is unavailable when
`FASTI_API_URL` or `FASTI_PUBLIC_URL` is configured, since those settings
pin the URL other tooling should use. Run `./scripts/dev.sh --status` at
any time to see the live URL.

Some worktrees also carry `apps/web`, the local B4 Workbench and its separate
service diagnostic. If this worktree has `apps/web`, the command also builds
`@fasti/tokens` and `@fasti/sdk` (its workspace dependencies), then starts
Vite. The harness listens at `http://127.0.0.1:5173` and proxies `/api`
requests to whichever URL `fastid` actually started on.

Not every worktree has `apps/web`. Check with `git worktree list`. Then look
for `apps/web` inside each worktree path. If your current worktree does not
have `apps/web`, `./scripts/dev.sh` says so and starts only `fastid`.

Open `/` for the Workbench and `/status` for service diagnostics. The browser
can save one non-secret client service URL. A cross-origin URL requires an
HTTPS reverse proxy that explicitly allows the Workbench origin through CORS.
Direct `fastid` does not provide CORS. Node listener, public URL, provider, and
credential settings remain disabled in the browser because they require the
trusted host.

### Trusted Desktop review host

Use a separate private data root to run the trusted Desktop review host:

```bash
FASTI_DATA_ROOT=/path/to/private/fasti-desktop-data ./scripts/dev.sh --desktop
```

This command builds the static Workbench and starts the Tauri app in the
foreground. The app embeds its local kernel and requires the exact
`127.0.0.1:8420` listener. It auto-starts only an already initialized and
verified TrailBase root. It does not start `fastid` or Vite. Close the window
or press `Ctrl-C` to stop it. `--status` and `--stop` continue to manage only
the daemon, web harness, and scoped container. Desktop remains an unpackaged
review candidate.

Other commands:

```bash
./scripts/dev.sh --stop      # stop the daemon and web harness this started
./scripts/dev.sh --status    # show what is running and its live URLs
./scripts/dev.sh --open      # open the web UI, or the API health check
./scripts/dev.sh --podman    # run fastid in a scoped Podman container instead
./scripts/dev.sh --docker    # same, using Docker
./scripts/dev.sh --desktop   # run the trusted Desktop review host in foreground
./scripts/dev.sh --self-test # verify the launcher's own process handling
```

Initialize TrailBase explicitly once. The default native and Desktop loops
then auto-start that initialized root after verification:

```bash
./scripts/dev.sh --prepare-offline
./scripts/dev.sh trailbase initialize
./scripts/dev.sh trailbase start
./scripts/dev.sh trailbase status
./scripts/dev.sh trailbase stop
```

Initialization must run in the owning terminal because it prints the rotated
administrator password once. See the [TrailBase runbook](operations/trailbase.md)
for native/OCI boundaries, backup, restore, and conformance.

On the exact requested-and-bound `127.0.0.1:8420` durable listener, Fasti
mounts the C1 route set even when TrailBase is not initialized. The projection
then reports the exact unavailable state. Code exchange and new Fasti session
issuance require a verified installation receipt and persisted active
activation. Alternate loopback, generic local, integration, wildcard or
container forwarding, and remote routers omit C1 routes.

Useful environment variables: `FASTI_PORT` (preferred port, default 8420),
`FASTI_LISTEN` (full bind address), `FASTI_PORT_FALLBACK` (`auto` or `fail`
-- `fail` refuses to start rather than picking a different port),
`FASTI_API_URL` (pin the URL other tooling should use instead of trusting
the fallback), `FASTI_PUBLIC_URL` (show a separate reverse-proxy origin), and
`FASTI_DEV_SCOPE` (name this worktree's container so multiple worktrees can
run containers side by side).

Search action receipts are immutable replay evidence. New actions accept up to
10,000 receipts and 163,840,000 bytes of canonical receipt JSON per workspace by
default. Local operators can raise those admission ceilings with
`FASTI_SEARCH_ACTION_RECEIPT_MAX_ROWS` and
`FASTI_SEARCH_ACTION_RECEIPT_MAX_BYTES`; values below the supported defaults,
zero, non-decimal or oversized values stop Store startup. Replays and reads stay
available at the ceiling. Only new actions return `capacity_exceeded` (HTTP 507),
and raising the ceiling then permits them. Fasti does not delete or compact this
audit history automatically.

## Browser and daemon QA

Check the daemon by hand, using the URL `./scripts/dev.sh` printed (or
`./scripts/dev.sh --status` to see it again) -- fastid may be on a fallback
port, not 8420:

```bash
curl --fail --silent http://127.0.0.1:8420/api/v1/health   # replace 8420 if it fell back
```

The exact response is `{"status":"healthy","version":"0.1.0"}`.

The launcher also sends an empty initialization request without the bootstrap
secret. The expected response is `403`. A `404` means the durable router is not
mounted, so the launcher stops instead of reporting a false success.

Run the full contributor gate before you open a pull request:

```bash
cargo xtask test pr
```

See [Definition of Done](definition-of-done.md) for the B1 conformance
fixture and the Playwright/axe gates that apply once a change touches those
surfaces.

Run the C1 gate suite with:

```bash
cargo xtask test milestone --body C1
```

The command writes `target/fasti-receipts/access-c1.json` as the in-scope C1
delivery receipt. Its locked desktop tests cover the trusted-host source
boundary only. They do not prove packaged Tauri authentication. That WebView,
cross-platform, and packaged assistive-technology work is deferred to
`C1-TAURI-AUTH`; no packaged desktop authentication support is claimed.
C1 ordinary-browser delivery merged in [PR #119](https://github.com/Scrobble-dev/Fasti/pull/119).
Review, exact-head CI, and merged-tree evidence remain required for subsequent
changes; see the [canonical checkpoint](plans/trailbase-authentication-remediation.md#24-c1-delivery-and-c2-foundation-checkpoint).

## Desktop QA

The mocked Tauri Playwright journeys prove UI behavior and IPC payload shape.
They do not prove a packaged Desktop runtime. For source-run acceptance, use an
isolated `FASTI_DATA_ROOT`, launch `--desktop`, and verify these facts:

- the Fasti window renders and closes cleanly;
- the launcher did not start `fastid` or Vite;
- browser mode sends provider credentials only to the authenticated Fasti API
  over a loopback or HTTPS endpoint and never stores or reads them back;
- the trusted host accepts a configured provider search and creates one Record
  through `track_provider_candidate`;
- the success status shows the exact returned Fasti Record ID.

Do not record a token, credential field, provider request header, or data-root
content in screenshots or logs. A real provider acceptance requires an
operator-owned credential. Mocked IPC is not that evidence. Packaged Orca,
NVDA, and VoiceOver checks remain open release gates.

## Update

`apps/web` hot-reloads. Vite picks up file changes on its own. You do not
need to restart it.

`fastid` does not hot-reload. Restart it after a Rust change:

```bash
./scripts/dev.sh --stop
./scripts/dev.sh
```

Pull the latest code and dependencies:

```bash
git pull --ff-only
cargo fetch --locked
pnpm install --frozen-lockfile
```

## Docker and Podman

The root `Dockerfile` builds the release image only. It contains `fastid`
and `fasti`. Build it with:

```bash
podman build --tag fasti:b0 .
```

`./scripts/dev.sh --podman` (or `--docker`) runs that image locally, scoped
to this worktree by container name so more than one worktree can run a
container at the same time without colliding. The image listens on its wildcard
container socket, while the launcher publishes it only on host
`127.0.0.1`. The launcher sets `FASTI_EXTERNAL_BIND_IP=127.0.0.1` so `fastid`
can distinguish this trusted loopback port forward from a remotely exposed
wildcard listener. It runs the process as the invoking non-root user so the
worktree-owned `.dev-data` directory stays writable without changing its
ownership. Podman also keeps that user ID mapped into its rootless user
namespace. Do not set the exposure assertion for a public port mapping.

C1 browser authentication is unavailable in this Fasti container-forwarding
topology. The launcher does not mount or export the TrailBase root to the Fasti
container. Use the native or Desktop loop for C1 review.

The native (default) dev loop above does not use a container at all. If your
local dev environment happens to run inside a container that shares your
host's network (for example, a distrobox or toolbox container), a port
opened by `fastid` or `apps/web` will not show up as a "published port" in
Podman Desktop or BoxBuddy. This is expected. A host-networked container
never publishes ports the way `podman run -p` does. Check that a port is
alive with `curl` or `ss`, not by looking for it in a container manager UI.

### The `local` build target: one container with the web UI

The Dockerfile has a second, optional target named `local`. It adds the
built web UI on top of the same release image. Build it with:

```bash
podman build --target local --tag fasti:local .
```

Plain `docker build .` (no `--target`) still builds the release image only.
CI's own image builds pass `--target runtime` explicitly, so they are not
affected by this second target. The two images share every stage up to
`runtime` -- there is one Dockerfile, not two, so the build recipe for
`fastid` itself never drifts between them.

`fastid` serves the UI itself. There is no separate reverse proxy and no
second process. It does this with one environment variable:

- `FASTI_STATIC_DIR` names a directory of pre-built static files (the output
  of `apps/web`'s `vite build`). The `local` image sets this by default. Any
  request that does not match an `/api/*` route falls back to the files in
  this directory, and a missing file falls back further to `index.html`, so
  the web UI's own client-side router can take over. See
  `with_static_fallback` in `crates/fasti-api/src/lib.rs`.

This is unrelated to `FASTI_DATA_ROOT` and `FASTI_EXTERNAL_BIND_IP`, which
control the durable API (see above). A bare `podman run fasti:local` with no
other flags serves the UI on a safe, health-only backend -- the UI does not
turn on durable routes by itself. To get the full product -- UI and durable
API together, on one URL -- pass all three: a data volume, `FASTI_DATA_ROOT`,
and `FASTI_EXTERNAL_BIND_IP`. The README's container quick start shows the
exact commands.

This does not change `apps/web`'s status as B4, review-only, and not a
release claim (see the top of this page). The `local` target exists so
anyone can try Fasti in one command. It is not what CI validates as the
release artifact -- that remains the plain `runtime` target, unchanged.
