# Review the provider boundary

## Outcome

Understand what a provider adapter can do before proposing code or a manifest.

## Current support state

Fasti has governed provider and integration patterns. A new provider is not
active because a converter, fixture, manifest, or UI row exists. Runtime
transport and exact evidence must pass the capability gate.

## Provider responsibility

A provider adapter can:

- accept bounded provider input;
- validate and normalize that input;
- translate provider identifiers into typed evidence claims;
- submit a neutral application request;
- report typed problems and safe retry state.

A provider adapter cannot:

- create a second identity model;
- make a provider identifier the Fasti primary key;
- write directly to Fasti storage;
- bypass scopes or object authorization;
- place credentials in a URL, browser, log, screenshot, or error message;
- claim production support from a fixture or health probe.

## Network boundary

Resolve every destination address and authorize every resolved address. Use a
pinned, proxy-free, redirect-free client before credential access. Reject private
or changed destinations according to the provider network policy.

## Verify a proposal

The proposal names one capability owner, bounded context, authorization rule,
contract disposition, runtime transport, failure cases, test fixture, negative
network test, and rollback path.

## Problems and recovery

If a destination, identifier, redirect, credential, or response is unsafe, stop
before mutation. Keep the prior local state and report a redacted typed problem.

[Read the security boundary](/security/)

## What to learn next

[Prepare a first contribution](/contribute/first-change/)

Content state: STE-controlled draft.
