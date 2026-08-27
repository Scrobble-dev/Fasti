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

This command builds and starts `fastid`, the production daemon. It waits
until `fastid` answers its health check. Then it prints the API URL.

`fastid` prefers `http://127.0.0.1:8420`. By default, startup fails if that
port is already taken. Set `FASTI_PORT_FALLBACK=auto` to have `fastid` ask
the operating system for a free port instead. In that case, the script
prints the port it actually got. Automatic fallback is unavailable when
`FASTI_API_URL` or `FASTI_PUBLIC_URL` is configured, since those settings
pin the URL other tooling should use. Run `./scripts/dev.sh --status` at
any time to see the live URL.

Some worktrees also carry `apps/web`, a pre-production health and
interface-quality harness (see the "Browser QA harness" row in the status
table above). If this worktree has `apps/web`, the command also builds
`@fasti/tokens` and `@fasti/sdk` (its workspace dependencies), then starts
Vite. The harness listens at `http://127.0.0.1:5173` and proxies `/api`
requests to whichever URL `fastid` actually started on.

Not every worktree has `apps/web`. Check with `git worktree list`. Then look
for `apps/web` inside each worktree path. If your current worktree does not
have `apps/web`, `./scripts/dev.sh` says so and starts only `fastid`.

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
the fallback), `FASTI_DEV_SCOPE` (name this worktree's container so multiple
worktrees can run containers side by side).

## QA

Check the daemon by hand, using the URL `./scripts/dev.sh` printed (or
`./scripts/dev.sh --status` to see it again) -- fastid may be on a fallback
port, not 8420:

```bash
curl --fail --silent http://127.0.0.1:8420/api/v1/health   # replace 8420 if it fell back
```

The exact response is `{"status":"healthy","version":"0.1.0"}`.

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
container at the same time without colliding.

The native (default) dev loop above does not use a container at all. If your
local dev environment happens to run inside a container that shares your
host's network (for example, a distrobox or toolbox container), a port
opened by `fastid` or `apps/web` will not show up as a "published port" in
Podman Desktop or BoxBuddy. This is expected. A host-networked container
never publishes ports the way `podman run -p` does. Check that a port is
alive with `curl` or `ss`, not by looking for it in a container manager UI.
