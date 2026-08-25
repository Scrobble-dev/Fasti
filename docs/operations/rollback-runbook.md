# Rollback Runbook

## Scope

Fasti has no auto-updater and none is planned. Rollback is always a manual redeploy of a prior tagged artifact, using the same distribution paths a fresh deploy uses. This runbook does not invent new verification tooling: "rollback succeeded" is defined as "the existing smoke gate a fresh deploy has to pass, passes again" against the restored artifact.

## OCI rollback

1. Identify the last-known-good tag (a `v*.*.*` git tag that passed `.github/workflows/release.yml`).
2. Stop and remove the current container.
3. Run the prior tag's image with the same volume and port mapping the current deployment uses.
4. Re-run `scripts/smoke-oci.sh <image>` against the redeployed container. It proves non-root execution, health over loopback, an explicit failure for the network-guarded verify command, and the idle-memory budget — the same bar a fresh deploy has to clear.

## Native rollback

1. Identify the last-known-good release archive for the host architecture.
2. Stop the `fastid` service.
3. Replace the `fastid`/`fasti` binaries with the prior release's binaries.
4. Restart the service.
5. Re-run `scripts/smoke-native.sh <target-dir>` against the restored binaries. It proves the guarded CLI fails explicitly offline, the daemon serves health over loopback with the network denied, and the idle-memory budget holds.

## Data and migration rollback

Not defined here. Migration rollback is a proof obligation of B8a (native/OCI/source-build/migration/rollback readiness), not invented in this runbook. If B8a's migration proof does not yet cover a rollback scenario, treat data rollback as out of scope until it does — do not attempt to improvise migration rollback semantics against a live data root.

## Verification

Both procedures above conclude with an existing, already-enforced smoke script. No new rollback-specific verification tooling exists or is needed: a rollback that passes the same gate a normal deploy passes is, by definition, a working deployment.
