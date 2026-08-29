# Capability reference

The capability registry is the authoritative public ledger. This page is a
human entry point for its generated projection.

## Read each dimension

Do not use one “implemented” badge. Read these fields separately:

- contract body and contract state;
- implementation owner and evidence;
- runtime body and runtime availability;
- public support state;
- authorization and scopes;
- required or not-applicable surfaces;
- examples, problem codes, and UAT relationships.

## Local inspection

From the repository root:

```bash
cargo run --locked -p fasti-cli -- capability list
cargo run --locked -p fasti-cli -- capability show observation.accept --output json
```

These commands read the generated public registry. They do not activate a
runtime capability.

## Public raw resource

The site publishes the exact generated registry at
[capabilities.json](/capabilities.json).

## Source and review evidence

- [Authored registry](https://github.com/Scrobble-dev/Fasti/blob/dev/contracts/registry/v1/capabilities.yaml)
- [Capability ledger guide](https://github.com/Scrobble-dev/Fasti/blob/dev/docs/capability-ledger.md)

Content state: STE-controlled draft.
