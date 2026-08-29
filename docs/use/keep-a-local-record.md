# Keep a local Record

## Outcome

Understand how Fasti can keep a media entity without giving one metadata
provider ownership of its identity.

## Current support state

Durable Record, identifier, and namespace routes are implemented on the governed
local runtime. The browser Workbench is a pre-production review surface. Fasti is
not a supported public release.

## Record, identifier, and observation

A Fasti Record identifies a local media entity. A provider identifier is a claim
attached to that Record. An observation reports what a source observed. These
concepts are related, but they are not the same object.

For example, changing the preferred provider from TMDB to TVDB must not change
the Fasti Record ID. If two exact identifiers conflict, Fasti must keep the
conflict visible and avoid an unsafe automatic merge.

## What Fasti preserves

- stable local Record identity;
- original observations and evidence;
- typed provider identifiers;
- unresolved and partially resolved state;
- append-only interpretations and corrections;
- receipts that record the completed durability boundary.

## What Fasti does not infer safely

A title match alone does not prove identity. A provider redirect, reused ISBN,
alternate cut, remaster, season order, or anime numbering system can refer to a
different entity or segment.

## Verify the model

Inspect [Capabilities](/reference/capabilities/) and find
`identity.record.create`. Compare its runtime and support state with
`observation.accept`. The two capabilities have separate owners and scopes.

## Problems and recovery

If exact identifiers disagree, retain both claims and use the governed conflict
path. Do not delete the item to make the conflict disappear.

[Read typed problems](/integrate/problems/)

## What to learn next

[Trace a first governed observation](/integrate/first-observation/)

## Source and review evidence

- [Fasti constitution](https://github.com/Scrobble-dev/Fasti/blob/dev/docs/constitution.md)
- [Identity UAT matrix](https://github.com/Scrobble-dev/Fasti/blob/dev/tests/conformance/identity-uat-matrix.v1.csv)

Content state: STE-controlled draft.
