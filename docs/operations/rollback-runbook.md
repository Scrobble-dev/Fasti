# Rollback Runbook

## Scope

Fasti has no auto-updater and none is planned. Rollback is always a manual redeploy of a prior tagged artifact, using the same distribution paths a fresh deploy uses. This runbook does not invent new verification tooling: "rollback succeeded" is defined as "the existing smoke gate a fresh deploy has to pass, passes again" against the restored artifact.

## OCI rollback

1. Identify the last-known-good tag (a `v*.*.*` git tag that passed `.github/workflows/release.yml`).
2. Rebuild the image from that exact tag: `git checkout <tag> && docker build --target runtime --tag fasti:rollback .`. Fasti does not yet publish a built OCI image anywhere (deferred to B8's signing work; see `docs/architecture/b8b-release-readiness.md`) — the release workflow's `checksums` job builds and hashes the same artifact only to self-verify in CI, then discards it; only the resulting `checksums-<arch>.sha256` manifest is published. There is no `image@sha256:<digest>` to pin against, and no pre-built archive to download.
3. Download that tag's release-run `fasti-b8b-checksums-<arch>` artifact for the host architecture and save `docker image save fasti:rollback -o oci-<arch>.tar`, then verify the rebuild reproduces the tagged release's exact bytes with `sha256sum --ignore-missing -c checksums-<arch>.sha256` in the directory containing both the manifest and the freshly-saved `oci-<arch>.tar` (`--ignore-missing` skips the manifest's `fastid`/`fasti` lines for this OCI-only check instead of failing on their absence; rebuild them too per the native rollback steps below to verify the full manifest in one pass).
4. Stop and remove the current container.
5. Run the verified `fasti:rollback` image with the same volume and port mapping the current deployment uses.
6. Re-run `scripts/smoke-oci.sh fasti:rollback` against the redeployed container. It proves non-root execution, health over loopback, an explicit failure for the network-guarded verify command, and the idle-memory budget — the same bar a fresh deploy has to clear.

## Native rollback

1. Identify the last-known-good release tag for the host architecture.
2. Rebuild from that exact tag: `git checkout <tag> && cargo build --locked --release --bin fastid --bin fasti`. As with the OCI path, Fasti does not publish built binaries anywhere; only their hashes are.
3. Download that tag's release-run `fasti-b8b-checksums-<arch>` artifact and verify the freshly-rebuilt `target/release/fastid` and `target/release/fasti` reproduce the tagged release's exact bytes with `sha256sum -c checksums-<arch>.sha256` (run from a directory containing both the manifest and the two rebuilt binaries under those exact names).
4. Stop the `fastid` service.
5. Replace the `fastid`/`fasti` binaries with the verified rebuild.
6. Restart the service.
7. Re-run `scripts/smoke-native.sh <target-dir>` against the restored binaries. It proves the guarded CLI fails explicitly offline, the daemon serves health over loopback with the network denied, and the idle-memory budget holds.

## Data and migration rollback

General live-data rollback is not active. It remains a proof obligation of the
package that introduces each production migration. Do not run an older binary
against a data root that a newer binary opened.

PR A has one narrower, unreleased developer procedure for the v9-to-v10 truth
reset. It replaces PR-only simulated browser-account tables. It does not migrate
real human identities. Rehearse it only on a development root:

1. Run `./scripts/dev.sh --stop` from the worktree that owns the root.
2. Resolve the exact root with `realpath .dev-data`. If `FASTI_DATA_ROOT` was
   set, use `realpath -- "$FASTI_DATA_ROOT"` instead. Stop if the result is not
   the intended development root.
3. Confirm that `current/fasti.sqlite3-wal`, `current/fasti.sqlite3-shm`, and
   `current/fasti.sqlite3-journal` do not exist. A sidecar means the root is not
   ready for a filesystem copy.
4. Copy the closed `current/fasti.sqlite3` to an owner-only backup directory.
   Keep the v9 binary or exact source revision beside that copy.
5. Start the PR A binary through `./scripts/dev.sh`. Normal startup performs the
   forward migration. Stop and start it once more to prove restart.
6. To return to the old binary, stop PR A, retain the v10 file separately, copy
   the closed v9 backup into `current/fasti.sqlite3`, restore owner-only file
   permissions, and start only the retained v9 binary. Never point the old
   binary at the v10 file.

The store tests rehearse a populated v9 root, an injected failed-forward
transaction and retry, a v10 restart, and restoration of a closed v9 copy. The
v10 migration preserves unrelated Fasti rows but deliberately removes the
unreleased simulated browser credentials and sessions. A production Access
backup, TrailBase depot backup, activation generation, clone fencing, and joint
rollback remain blocked until their owning packages implement them.

## Verification

Both procedures above conclude with an existing, already-enforced smoke script. No new rollback-specific verification tooling exists or is needed: a rollback that passes the same gate a normal deploy passes is, by definition, a working deployment.
