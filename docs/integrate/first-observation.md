# Trace a first observation

## Outcome

Understand the safe path from a local durable node to one governed observation
request and typed receipt.

## Current support state

`POST /api/v1/observations` is implemented on the durable local runtime. It
requires a scoped bearer credential or a governed browser session. Fasti is not
a supported public release.

## Before you start

Complete [Local review](/operate/local-review/) with a private data root and a
loopback listener.

The one-time bootstrap secret, initialization proof, and enrolled credential are
different values. A trusted host must keep each value out of URLs, shell history,
process arguments, logs, screenshots, and browser storage.

## Safe setup sequence

1. Read the local data-root bootstrap secret through a trusted host process.
2. Send an empty JSON object to `POST /api/v1/node/initialization` with that
   secret as the bootstrap bearer value.
3. Keep the returned `initialization_proof` in memory.
4. Send the proof in the JSON body of `POST /api/v1/client-enrollments`.
5. Keep the returned bearer credential in the trusted client credential store.
6. Submit a `SubmitObservationRequest` to `POST /api/v1/observations`.

Do not repeat initialization or first-client enrollment automatically. Both
operations have `retry: never` in the generated TypeScript SDK.

## Observation rules

- `source_event_id` remains stable when the same source event is retried.
- `source` names the bounded source, not the identity owner.
- `observed_at` records when the adapter observed the event.
- Provider identifiers are typed evidence.
- Title text is evidence. It is not an irreversible identity key.
- A changed semantic payload with the same operation identity returns an
  idempotency conflict and does not mutate prior state.

Use the raw [OpenAPI document](/openapi.json) or the generated TypeScript SDK for
the exact request and response schema. Do not copy a fixture request into a
production-runtime claim.

## Expected result

A committed response contains a receipt ID, operation ID, observation ID,
evidence ID, source client ID, workspace ID, profile ID, disposition, resolution,
payload digest, and timestamps. Record and review identifiers can be absent when
the evidence cannot support an automatic identity decision.

## Verify the result

Confirm that:

- the response matches `SubmitObservationResponse`;
- the receipt and operation identifiers are present;
- a retry with the same source event and semantic payload returns the governed
  replay behavior;
- a changed payload does not overwrite the prior result.

## Problems and recovery

Read the response as `application/problem+json`. Use `safe_state` before a retry.
Use `retryability` to decide whether a retry is permitted. Follow one named
`next_actions` item.

[Read typed problems](/integrate/problems/)

## What to learn next

[Read the contract inventory](/reference/contracts/)

## Source and review evidence

- [Generated SDK guide](https://github.com/Scrobble-dev/Fasti/blob/dev/packages/sdk/README.md)
- [Production OpenAPI source](https://github.com/Scrobble-dev/Fasti/blob/dev/contracts/generated/v1/openapi.json)

Content state: STE-controlled draft.
