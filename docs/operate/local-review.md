# Run a local review

## Outcome

Start the current Fasti daemon on loopback and verify the exact state that it
exposes.

## Current support state

This is a source-run review path. It is not a supported installation or public
release.

## Before you start

Install Git and Rust `1.97.1` or later. A warm build takes about two minutes on
the documented development path. A cold build takes longer.

## Start the health-only daemon

From the repository root, run:

```bash
cargo run --locked -p fastid
```

In another terminal, run:

```bash
curl --fail --silent http://127.0.0.1:8420/api/v1/health
```

## Expected result

```json
{"status":"healthy","version":"0.1.0"}
```

This result proves that the current daemon answers on loopback. Without
`FASTI_DATA_ROOT`, it does not prove durable setup, browser sessions,
observations, Records, backup, restore, or release readiness.

## Enable the durable local router

Stop the daemon. Choose a private directory that is not inside the repository.
Then run:

```bash
FASTI_LISTEN=127.0.0.1:8420 \
FASTI_DATA_ROOT=/path/to/private/fasti-data \
cargo run --locked -p fastid
```

The initialization route must return `403` when a required local bootstrap
secret is absent. A `404` means that the durable router is not mounted.

Do not put the bootstrap secret, initialization proof, or enrolled credential in
a command line, URL, shell history, log, screenshot, or browser storage. Use a
trusted host flow that keeps these one-time values in memory.

## Problems and recovery

- If port `8420` is in use, stop the other process or use the documented
  loopback fallback.
- If the durable route returns `404`, check `FASTI_DATA_ROOT` and restart.
- If a public or wildcard listener is requested, stop and read the network
  trust rules. Automatic port fallback is local only.

[Read typed problems](/integrate/problems/)

## What to learn next

[Trace a first observation](/integrate/first-observation/)

## Source and review evidence

- [Local development loop](https://github.com/Scrobble-dev/Fasti/blob/dev/docs/dev-loop.md)
- [Network configuration](https://github.com/Scrobble-dev/Fasti/blob/dev/docs/network-configuration.md)

Content state: STE-controlled draft.
