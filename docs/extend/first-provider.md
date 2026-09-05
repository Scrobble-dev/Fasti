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

## Provider Search transport

`POST /api/v1/search/providers/{provider_id}` acquires a bounded result page.
It uses the existing `metadata.search` capability and `metadata_search` scope.
The SDK operation is `searchProviderPage(providerId, request, options)`.
The JSON body contains `query`, `page`, `grains`, `offline`, and optional
`locale` and `region`. The query accepts 1–256 UTF-8 bytes. Page numbers start
at one. Current TMDB and Google Books Search routes do not filter by region.

Page acquisition can persist candidate receipts. Browser requests therefore
require session cookies and CSRF proof, including cache-hit and offline requests.
Browser authority is accepted only on the exact direct Access listener. Generic
local and remote listeners require bearer credentials. The SDK does not retry
this POST automatically. Cancellation remains supported.

A successful response contains either a page or a source-unavailable outcome.
A page contains at most 100 candidates, durable receipt IDs, cache state,
freshness and expiry times, and an optional next page. It does not contain grant
IDs, credential references, configuration digests, or raw provider responses.
All HTTP results use `Cache-Control: private, no-store`; the governed server cache
is separate from HTTP caching. Offline mode does not access the credential vault
or fetch from providers. An eligible stale page remains distinguishable from a
fresh page. No matching cache entry produces an explicit unavailable outcome.

Acquiring a page does not create a Record or change Library state. Candidate
details, explicit Record actions, local Search transport and Workbench Search
composition remain separate M4 integration gates. The current transport tests
prove real SQLite offline behavior; they are not live-provider network evidence.

The unmerged v16 migration adds Search permission only to the enrolled node-owner
grant. C1's first human administrator links that same grant. Delegated grants are
not expanded, and opening a database again does not restore a removed permission.
Published migrations v1–v15 remain unchanged. Use disposable databases for local
v16 qualification; an older unmerged v16 database does not rerun the migration.

For rollback, return to the previous application before upgrading a database, or
restore a pre-upgrade backup with its matching binary. Do not lower `user_version`
or edit historical migrations in place.

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
