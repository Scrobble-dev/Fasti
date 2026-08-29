# Rollback Runbook

## Scope

Fasti has no auto-updater and none is planned. Rollback is always a manual redeploy of a prior tagged artifact, using the same distribution paths a fresh deploy uses. This runbook does not invent new verification tooling: "rollback succeeded" is defined as "the existing smoke gate a fresh deploy has to pass, passes again" against the restored artifact.

## OCI rollback

1. Identify the last-known-good tag (a `v*.*.*` git tag that passed `.github/workflows/release.yml`).
2. Rebuild the image from that exact tag: `git checkout <tag> && docker build --target runtime --tag fasti:rollback .`. Fasti does not yet publish a built OCI image anywhere (deferred to B8's signing work; see `docs/architecture/b8b-release-readiness.md`) — the release workflow's `checksums` job builds and hashes the same artifact only to self-verify in CI, then discards it; only the resulting `checksums-<arch>.sha256` manifest is published. There is no `image@sha256:<digest>` to pin against, and no pre-built archive to download.
3. Download that tag's release-run `fasti-b8b-checksums-<arch>` artifact for the host architecture and save `docker image save fasti:rollback -o oci-<arch>.tar`, then verify the rebuild reproduces the tagged release's exact bytes with `sha256sum -c checksums-<arch>.sha256` in the directory containing both the manifest and the freshly-saved `oci-<arch>.tar` (the manifest also lists `fastid`/`fasti`; ignore those lines for the OCI rollback, or rebuild them too per the native rollback steps below to satisfy the full check).
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

Not defined here. Migration rollback is a proof obligation of B8a (native/OCI/source-build/migration/rollback readiness), not invented in this runbook. If B8a's migration proof does not yet cover a rollback scenario, treat data rollback as out of scope until it does — do not attempt to improvise migration rollback semantics against a live data root.

## Verification

Both procedures above conclude with an existing, already-enforced smoke script. No new rollback-specific verification tooling exists or is needed: a rollback that passes the same gate a normal deploy passes is, by definition, a working deployment.
