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
bash scripts/dev.sh
```

This command starts `fastid`, the production daemon. It waits until
`fastid` answers its health check. Then it prints the health URL.

`fastid` listens at `http://127.0.0.1:8420` by default. Its port is not
scoped to this worktree: something else on the machine (a canary or
monitoring container, another worktree's dev session) can hold 8420 at the
same time. If `scripts/dev.sh` finds 8420 taken, it says what has it and
tries the next port instead of failing. Set `FASTI_DEV_PORT` to pin one
yourself. Run `scripts/dev.sh status` to see which port is actually in use.

Some worktrees also carry `apps/web`, the pre-production health and
interface-quality harness (see the "Browser QA harness" row in the status
table above). If this worktree has `apps/web`, the command builds
`@fasti/tokens` and `@fasti/sdk` first (its workspace dependencies), then
starts Vite. The harness listens at `http://127.0.0.1:5173` and proxies
`/api` requests to whichever port `fastid` actually started on.

Not every worktree has `apps/web`. Check with `git worktree list`. Then
look for `apps/web` inside each worktree path. If your current worktree does
not have `apps/web`, `scripts/dev.sh` says so and starts only `fastid`.

Other commands:

```bash
bash scripts/dev.sh stop     # stop what this script started
bash scripts/dev.sh status   # show what is running
bash scripts/dev.sh logs     # tail the daemon and web log files
bash scripts/dev.sh open     # open the running web UI in a browser
bash scripts/dev.sh podman   # run the release container image instead
bash scripts/dev.sh desktop  # run the Tauri desktop shell, once it exists
bash scripts/dev.sh help     # show all commands
```

`podman` and `desktop` are separate modes, not part of the default dev loop.
`podman` runs the built release image (see the Docker/Podman section below).
`desktop` runs the Tauri shell once `apps/desktop` exists; today it prints a
clear message and exits, because desktop packaging is B8 work and not yet in
this repository.

## QA

Check the daemon by hand:

```bash
curl --fail --silent http://127.0.0.1:8420/api/v1/health
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
bash scripts/dev.sh stop
bash scripts/dev.sh start
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

This release image is not part of the local dev loop. `scripts/dev.sh` never
builds or starts a container.

If your local dev environment happens to run inside a container that shares
your host's network (for example, a distrobox or toolbox container), a port
opened by `fastid` or `apps/web` will not show up as a "published port" in
Podman Desktop or BoxBuddy. This is expected. A host-networked container
never publishes ports the way `podman run -p` does. Check that a port is
alive with `curl` or `ss`, not by looking for it in a container manager UI.
