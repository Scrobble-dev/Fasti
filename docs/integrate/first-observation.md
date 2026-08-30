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

Use the raw <a href="/openapi.json">OpenAPI document</a> or the generated TypeScript SDK for
the exact request and response schema. Do not copy a fixture request into a
production-runtime claim.

## Submit with the generated SDK

Build the SDK as described in the [generated SDK guide](https://github.com/Scrobble-dev/Fasti/blob/dev/packages/sdk/README.md).
Store the enrolled credential and the observation request in separate,
permission-restricted files. Do not put either value in a command argument or
shell history.

The request file has this production `SubmitObservationRequest` shape. Replace
every `replace-with-` value with evidence from the real source event. Do not use
these non-production values as an observation.

```json
{
  "kind": "consumption_occurrence",
  "source": "replace-with-real-bounded-source",
  "source_event_id": "replace-with-stable-source-event-id",
  "observed_at": "replace-with-RFC3339-observation-time",
  "target_grain": "episode",
  "identifiers": [
    {
      "namespace": "imdb.title",
      "grain": "series",
      "value": "replace-with-real-provider-identifier"
    }
  ],
  "title": "replace-with-source-reported-title",
  "progress_percent": 100
}
```

Set `FASTI_CREDENTIAL_FILE` and `FASTI_OBSERVATION_FILE` to the two file paths.
Then run this command from the repository root:

```bash
node --input-type=module <<'EOF'
import { readFile } from "node:fs/promises";
import { FastiClient } from "./packages/sdk/dist/transport.js";

const credentialFile = process.env.FASTI_CREDENTIAL_FILE;
const observationFile = process.env.FASTI_OBSERVATION_FILE;
if (!credentialFile || !observationFile) {
  throw new Error("Set FASTI_CREDENTIAL_FILE and FASTI_OBSERVATION_FILE.");
}

const readCredential = async () =>
  (await readFile(credentialFile, "utf8")).trim();

/** @type {import("./packages/sdk/dist/transport.js").SubmitObservationRequest} */
const observation = JSON.parse(await readFile(observationFile, "utf8"));

if (JSON.stringify(observation).includes("replace-with-")) {
  throw new Error("Replace every example value with real source evidence.");
}

const client = new FastiClient({
  baseUrl: "http://127.0.0.1:8420",
  credential: readCredential,
});
const result = await client.submitObservation(observation);

console.log({
  receipt_id: result.receipt_id,
  operation_id: result.operation_id,
  disposition: result.disposition,
  resolution: result.resolution,
});
EOF
```

The generated client validates the request as `SubmitObservationRequest` before
it sends data. It resolves the credential for the request and does not send it
to the unauthenticated health route. The command prints receipt metadata only.
It does not print the credential or the submitted media activity.

Use a request that follows the production `SubmitObservationRequest` schema.
Use a stable `source_event_id` from the source system. Do not use a sample event
identifier for a real source event.

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
