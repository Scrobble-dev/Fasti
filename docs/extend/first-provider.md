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

### Response policy observation

The shared JSON response boundary records cache policy and the time headers
arrive, before the bounded body is read. Search pages retain that observation
even when empty or filtered; detail candidates retain it too. Raw headers and
the internal policy are not part of public candidate JSON.

HTTP syntax stays in the adapter. The application policy computes absolute
purpose-capped deadlines without renewing the observation time. It distinguishes
`no-store`, validation before every reuse, validation once stale, and permitted
reuse. It accounts for `Date`, `Age`, `Expires`, `max-age` and `stale-if-error`.
Unproven `Vary` matches require validation. Malformed policy cannot grant a more
permissive fallback. The parser reuses the existing locked HTTP-date dependency.

Provider-page acquisition and reuse apply this policy. The original observation
and deadlines survive persistence without renewal. Missing legacy policy cannot
grant reuse; a newer restrictive page cannot fall back to an older permissive
page. No-store page payloads do not enter SQLite or the WAL. After reauthorization,
the store removes only older ephemeral pages in that same partition.

Offline candidate payload reads and cached Record actions enforce retained-use
permission independently of receipt retention. No-cache evidence cannot be
reused; must-revalidate evidence requires remaining freshness. Metadata refresh,
refetched Record actions, durable selected-field reuse and legacy Desktop
conversion still need policy integration. Do not claim complete response-policy
coverage across those separate consumers from Search tests.

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

A successful response contains a durable page, live-only results, or a
source-unavailable outcome.
A page contains at most 100 candidates, durable receipt IDs, cache state,
freshness and expiry times, and an optional next page. It does not contain grant
IDs, credential references, configuration digests, or raw provider responses.
All HTTP results use `Cache-Control: private, no-store`; the governed server cache
is separate from HTTP caching. Offline mode does not access the credential vault
or fetch from providers. An eligible stale page remains distinguishable from a
fresh page. No matching cache entry produces an explicit unavailable outcome.

`outcome: live` contains normalized candidates and continuation only. It has no
receipt IDs, cache lifetime or permission to save cached evidence. It represents
an actual no-store response, not an unavailable placeholder. A newly acquired
durable page whose freshness has already ended uses `cache_state: observed`;
it does not claim to be fresh or eligible for later reuse. Neither Live nor
Observed can answer an offline request. Cached pages remain Fresh or StaleOnError.
The SDK binds all outcomes to the submitted source, page and offline mode.
Live candidate details and Record actions remain part of active M4 integration;
never place a provider ID into the durable receipt route.

Acquiring a page does not create a Record or change Library state. Workbench
Search composition remains a separate M4 integration gate.
The current transport tests prove real SQLite offline behavior; they are not
live-provider network evidence.

### Local Record Search

`POST /api/v1/search/records` searches the durable local index without contacting
providers or accessing credentials. Use `searchRecords(request, options)` in the
SDK. The body contains `query` (1–256 UTF-8 bytes), `grains` (up to 16; empty means
all grains), and optional `after`. This is a Search-authorized read: browser CSRF
is not required, and safe retries preserve the same serialized body. The query
does not appear in the URL. Responses are private and not HTTP-cacheable.

Each page returns up to 100 complete existing Record summaries, including all
identifiers, resolved fields and latest activity. The response limit is 4 MiB,
published on the OpenAPI operation and used by the SDK. The store reserves small
fixed envelope/field headroom and charges actual escaped string bytes before
copying identifiers. A page stops before a Record would exceed that budget; it
never drops identifiers. A single Record that exceeds the bounded page capacity
returns a typed capacity error without deleting evidence or skipping that Record.

Pass the returned `next` cursor unchanged, including after an empty page. It
tracks inspected positions, not only returned matches, and binds the server's
current workspace, profile, authority and query. Missing `next` means complete.
An unrelated large Record cannot block matching results: title resolution happens
before identifier hydration. Local Search works offline; provider-cache Search
continues to use the separate provider operation above.

### Candidate details and Record actions

`GET /api/v1/search/candidates/{provider_id}/{grain}/{candidate_receipt_id}`
requires an explicit `offline=true` or `offline=false` query parameter. The SDK
operation is `readSearchCandidate(providerId, grain, receiptId, query, options)`.
This read accepts current Search authority without browser CSRF mutation proof.
Offline reads return the original authorized, reusable snapshot, including its original
lifetime. They do not renew the receipt or access the provider credential vault.
Online reads refetch the stored provider coordinate and recheck authority before
returning details or a source failure. Refetched fields stay separate from the
snapshot; its lifetime is not the refetch time. Missing or inaccessible evidence
returns the same missing outcome. All outcomes remain private/no-store.

Retained coordinates can still authorize an online fetch when the original
payload requires validation. In that case `refetched_without_snapshot` returns
the existing receipt locator plus newly fetched details and effective locale;
`unavailable_without_snapshot` returns the locator and source problem only.
Neither outcome contains the original snapshot, its lifetime or response digest.
The service checks the exact fetched identity against the retained coordinate
before disclosure; a successful fetch does not prove the old payload unchanged.
Both outcomes are online-only, and the SDK binds their locator to the submitted
request. Old strict clients reject unfamiliar outcomes instead of misreading them.

`POST` to that candidate path followed by `/actions` accepts only an
`operation_id`, an `action` and an `evidence_mode`. Actions are `{ "kind": "create" }`
or `{ "kind": "attach", "record_id": "rec_…" }`; evidence mode is `cached` or
`refetch`. The SDK operation is `saveSearchCandidate(providerId, grain, receiptId,
request, options)`. No caller metadata, provider URL or authorization fields are
accepted. Browser saves require CSRF proof. Current `identity_write` permission
is checked first. A new save also requires Search authority. A completed exact
replay does not require its ephemeral Search receipt or Search permission, but
still requires current identity-write authority and the original actor/profile.
Cached saves recheck retained-use permission in both preparation and the commit
transaction. Permitted historical evidence keeps its separate 24-hour retention
window; it does not gain new metadata freshness or become Library membership.

The existing transaction creates or reuses a Record, or attaches evidence to the
explicit target. It does not change Library, progress or watched state. Refetch
failure never silently saves cached evidence. The SDK may retry with the same
operation ID and identical serialized body; changed intent conflicts. A saved
response is immutable acceptance history, not current Record state. Its original
timestamps and initial status remain unchanged on replay, even after expiry.
Read the Record through its canonical route for current state.

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
