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

This command builds and starts `fastid`, the production daemon. It waits until
the health route answers and the durable initialization route returns its real
authorization response. Then it prints the API URL.

`fastid` prefers `http://127.0.0.1:8420`. By default, startup fails if that
port is already taken. Set `FASTI_PORT_FALLBACK=auto` to have `fastid` ask
the operating system for a free port instead. In that case, the script
prints the port it actually got. Automatic fallback is unavailable when
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

Other commands:

```bash
./scripts/dev.sh --stop      # stop the daemon and web harness this started
./scripts/dev.sh --status    # show what is running and its live URLs
./scripts/dev.sh --open      # open the web UI, or the API health check
./scripts/dev.sh --podman    # run fastid in a scoped Podman container instead
./scripts/dev.sh --docker    # same, using Docker
./scripts/dev.sh --self-test # verify the launcher's own process handling
```

Useful environment variables: `FASTI_PORT` (preferred port, default 8420),
`FASTI_LISTEN` (full bind address), `FASTI_PORT_FALLBACK` (`auto` or `fail`
-- `fail` refuses to start rather than picking a different port),
`FASTI_API_URL` (pin the URL other tooling should use instead of trusting
the fallback), `FASTI_PUBLIC_URL` (show a separate reverse-proxy origin), and
`FASTI_DEV_SCOPE` (name this worktree's container so multiple worktrees can
run containers side by side).

## QA

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
