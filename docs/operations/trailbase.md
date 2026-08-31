# TrailBase development operations

This runbook covers the pinned TrailBase identity service used by Fasti
development and conformance tests. C1 implements the TrailBase-to-Fasti
exchange in local source. This runbook does not claim a supported package,
merge, release, or deployment.

## Locked release

Fasti uses TrailBase `v0.33.5` at upstream commit
`b4c85d5152d4e5f472e0b5da5303f7c938e3a083`.
[`third_party/trailbase/release.json`](../../third_party/trailbase/release.json)
binds both Linux native archives, the executable hashes, the OCI index and
platform graphs, and the reviewed licence text. Floating tags are rejected.
It also binds TrailBase `v0.33.4` native artifacts as a test-only adjacent
upgrade and rollback fixture. `v0.33.4` is not a supported runtime selection.

TrailBase remains alpha. The [licence review](../../third_party/trailbase/LICENSE-REVIEW.md)
requires it to remain a separate, unmodified process.

## Prerequisites

- Linux, Python 3, `curl`, and `ss`.
- Native mode: user systemd with cgroup v2 for the enforced CPU and memory scope.
- OCI mode: Podman or Docker. Set `FASTI_CONTAINER_RUNTIME` to the installed
  runtime before offline preparation.
- Network access for the first preparation. Later start and conformance commands
  use the verified local cache.

## Prepare offline inputs

Run this once with network access:

```bash
./scripts/dev.sh --prepare-offline
```

The command fetches the locked Rust and pnpm inputs and verifies and caches the
exact TrailBase native and OCI artifacts and the exact prior-version upgrade
fixture. A later offline start and conformance run use only those verified
inputs.

## Initialize

Run initialization in the owning terminal:

```bash
./scripts/dev.sh trailbase initialize
```

The command requires an interactive terminal. It starts TrailBase privately,
captures the upstream first password in memory, changes it through TrailBase's
documented authentication APIs, verifies the replacement, and prints the new
administrator password once. Redirected output is rejected. No password is
written to the bootstrap receipt, command arguments, or logs.

Each installation has its own TrailBase administrator credential. Store that
credential in an operator-selected password manager or equivalent private
installation record. Fasti never copies, stores, logs, receipts, or browser-
persists the administrator password. Do not create one shared administrator
password across installations and do not add a Fasti secret store for it.

Each person gets a distinct TrailBase account. Do not share the installation
administrator credential as a human sign-in account. If the administrator
credential is lost, use TrailBase's documented password-reset flow. Do not edit
its database.

The worktree-local root is `.dev-trailbase`. The root, depot, cache, receipt,
and files must remain owned by the current user and inaccessible to group and
other users. Symlinks and unsupported file types are rejected. Native and OCI
start repair only a stopped, current-user `runtime.lock`; active, linked, or
foreign-owned lock files fail closed.

## Start and inspect

Native:

```bash
./scripts/dev.sh trailbase start
./scripts/dev.sh trailbase status
```

OCI:

```bash
./scripts/dev.sh trailbase start --podman
./scripts/dev.sh trailbase status
```

Use `--docker` instead of `--podman` only on a host with Docker and the exact
image already prepared.

The default native Fasti launcher and the Desktop review launcher auto-start
TrailBase only when this worktree already has an initialized root. They verify
the root, exact release, runtime identity, and installation receipt before they
pass the root to Fasti. They never initialize TrailBase. An uninitialized root
keeps sign-in unavailable and reports `trailbase_trust_unavailable`.

The route-exposure contract is:

| Mode | Account routes | Admin routes | Record API | Readiness evidence |
|---|---|---|---|---|
| Native | `127.0.0.1:4000` | `127.0.0.1:4001` only | Not configured; public root must return 404 | Health check plus public-route boundary |
| OCI | `127.0.0.1:4000` | Container loopback only; not host-published | Not configured; public root must return 404 | Exact running image plus health and route boundary |
| Remote | Unavailable | Unavailable | Unavailable | No claim; do not expose this release remotely |

TrailBase `v0.33.5` accepts protocol-relative values in its shared redirect
validator. Therefore remote account and OAuth routes are unavailable. The next
action is to keep the service loopback-only until a pinned release proves safe
redirect validation and the remote TLS and trusted-proxy package passes review.
A reverse proxy, TLS endpoint, or public route is not part of this package.

## Fasti Access activation

Only a Fasti process with a data root that both requests and binds
`127.0.0.1:8420` mounts the C1 browser-authentication and session route set.
The routes remain mounted when TrailBase is unconfigured so Account and
security can report the unavailable state and next action. Exchange and new
Fasti session issuance remain disabled until Fasti verifies the installation
receipt and persists active activation.

Port fallback, alternate IPv4 or IPv6 loopback, generic local, integration,
wildcard or container forwarding, and remote routers omit C1 routes. The Fasti
OCI launcher does not mount the TrailBase root into the Fasti container and
reports browser authentication unavailable. Use the native or Desktop launcher
for C1 review.

The process uses one CPU and the repository's 192 MiB memory ceiling. OCI mode
also uses no extra swap, a 128-process limit, a read-only root filesystem,
`no-new-privileges`, no Linux capabilities, and no container log driver.
Native logs use warning level and go to `.dev-logs/trailbase.log`. Never place a
password, token, social secret, or email verification link in an evidence file.

The launcher rejects a second native or OCI process for the same worktree. It
does not hide failure with automatic restart. An operator inspects status and
starts the process again after fixing the reported condition.

## Stop

```bash
./scripts/dev.sh trailbase stop
```

The command stops the tracked native process and the exact scoped container.
Run status after stopping. It must report `STOPPED`.

## Backup and restore

Stop TrailBase before backup:

```bash
./scripts/dev.sh trailbase stop
./scripts/dev.sh trailbase backup
```

The backup contains the complete depot and the non-secret bootstrap receipt.
It excludes the downloaded runtime and OCI cache. Every entry has an exact
type, mode, size, and SHA-256 value in the archive manifest. Active-depot
backup, symlinks, unsafe paths, unexpected types, digest changes, and release
mismatches fail closed.

Restore only to a new isolated directory:

```bash
./scripts/dev.sh trailbase restore \
  .dev-trailbase-backups/trailbase-v0.33.5-<timestamp>.zip \
  /absolute/private/isolated-trailbase-root
```

The target and its parents must be private. Restore creates a fresh owner-only
runtime lock, publishes atomically, and never overwrites an existing root. The
lock is not restored from the archive. Verify the restored root before activation:

```bash
python3 -B scripts/trailbase_runtime.py verify-root \
  /absolute/private/isolated-trailbase-root
```

## Upgrade and rollback boundary

The conformance gate proves an adjacent `v0.33.4` to `v0.33.5` artifact change.
It creates and verifies an account on `v0.33.4`, stops the process, creates a
digest-bound full-depot backup, restores that backup to an isolated upgrade
root, starts `v0.33.5`, and verifies the account across a restart. The exact
`v0.33.4` binary then starts only against a second restore of the untouched old
backup and verifies the account across a restart.

The two releases have the same embedded database migrations and depot schema.
This proves adjacent artifact replacement and backup-based rollback. It does
not prove a database migration, downgrade, or rollback of a newer depot. Never
run an older TrailBase binary against a depot opened by a newer version.

A future upgrade must pin the new source tag, native artifacts, OCI graph,
executable, and licence; cache them offline; take a stopped full-depot backup;
restore it to an isolated root; run account and migration conformance there;
and activate only after its exact milestone gate passes.

## Conformance

On a prepared machine, run:

```bash
cargo xtask test milestone --body B
```

The gate verifies the release lock, mutation sentinels, launcher syntax and
invariants, native and OCI lifecycle, the combined 192 MiB resource ceiling,
and a hermetic account lifecycle. The lifecycle includes local
registration, verification, password change and reset, password-plus-TOTP,
refresh-session revocation, account deletion, identity administration, and a
local OIDC authorization-code social flow with PKCE. Collision and provider
outage fail closed. It also verifies the adjacent-version upgrade and
backup-based rollback boundary above.

TrailBase v0.33.5 social callbacks do not prove TOTP for the current sign-in.
It also has no documented per-account disabled state. Those limits stay
visible and cannot be simulated in Fasti. Its unsafe redirect acceptance keeps
remote account and OAuth exposure unavailable.

Run the local C1 gate suite with:

```bash
cargo xtask test milestone --body C1
```

The command writes `target/fasti-receipts/access-c1.json`. That file is a gate-
suite receipt, not a C1 closure manifest. Package smoke, packaged WebView,
cross-platform, assistive-technology, final review, exact-head CI, merge, and
release evidence remain pending.
