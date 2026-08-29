# Rollback Runbook

## Scope

Fasti has no auto-updater and none is planned. Rollback is always a manual redeploy of a prior tagged artifact, using the same distribution paths a fresh deploy uses. This runbook does not invent new verification tooling: "rollback succeeded" is defined as "the existing smoke gate a fresh deploy has to pass, passes again" against the restored artifact.

## OCI rollback

1. Identify the last-known-good tag (a `v*.*.*` git tag that passed `.github/workflows/release.yml`).
2. Download that tag's release-run `fasti-b8b-checksums-<arch>` artifact and verify the redeployed image archive against `checksums-<arch>.sha256` with `sha256sum -c` before running it. Fasti does not yet push OCI images to a registry (deferred to B8's signing work; see `docs/architecture/b8b-release-readiness.md`), so there is no `image@sha256:<digest>` to pin against yet — the checksums manifest is the current integrity check.
3. Stop and remove the current container.
4. Run the prior tag's image with the same volume and port mapping the current deployment uses.
5. Re-run `scripts/smoke-oci.sh <image>` against the redeployed container. It proves non-root execution, health over loopback, an explicit failure for the network-guarded verify command, and the idle-memory budget — the same bar a fresh deploy has to clear.

## Native rollback

1. Identify the last-known-good release archive for the host architecture.
2. Download that tag's release-run `fasti-b8b-checksums-<arch>` artifact and verify the `fastid`/`fasti` binaries against `checksums-<arch>.sha256` with `sha256sum -c` before replacing anything.
3. Stop the `fastid` service.
4. Replace the `fastid`/`fasti` binaries with the prior release's (now-verified) binaries.
5. Restart the service.
6. Re-run `scripts/smoke-native.sh <target-dir>` against the restored binaries. It proves the guarded CLI fails explicitly offline, the daemon serves health over loopback with the network denied, and the idle-memory budget holds.

## Data and migration rollback

Not defined here. Migration rollback is a proof obligation of B8a (native/OCI/source-build/migration/rollback readiness), not invented in this runbook. If B8a's migration proof does not yet cover a rollback scenario, treat data rollback as out of scope until it does — do not attempt to improvise migration rollback semantics against a live data root.

## Verification

Both procedures above conclude with an existing, already-enforced smoke script. No new rollback-specific verification tooling exists or is needed: a rollback that passes the same gate a normal deploy passes is, by definition, a working deployment.
