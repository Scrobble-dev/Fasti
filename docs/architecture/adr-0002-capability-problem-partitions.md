# ADR-0002: Keep public and staged capability problems separate

- Status: Accepted for the B2 draft review
- Date: 2026-08-23
- Pull request: [#14](https://github.com/Scrobble-dev/Fasti/pull/14)

## Context

A capability can have two different states at the same time during staged delivery:

1. Its active public contract can still belong to an earlier body.
2. Its local implementation can already return a later failure during internal review.

The first B2 storage paths exposed this difference. The store correctly returned failures such as `bootstrap_closed` and `authentication_failed`. The capability table listed only the finalized B1 public problems. A defensive check then treated the safe B2 failure as undeclared and stopped the process.

Adding every implemented failure to the public B1 list would be incorrect. It would publish contract meaning before the related B2 surface, examples, schemas, documentation, and client behavior were ready.

A broad internal bypass would also be incorrect. It would remove capability ownership and could hide a wrong problem mapping.

## Decision

Each capability owns one problem policy with two explicit, fixed sets:

- **Public problems** are part of the active public contract. Contract generation and public metadata can iterate only this set.
- **Staged problems** are implemented local failures that runtime validation can accept, but that are not public yet.

Runtime membership is the union of the two declared sets. There is no catch-all failure set and no global bypass.

The two sets must satisfy these executable invariants for every capability:

- each public set contains no duplicate;
- each staged set contains no duplicate;
- a problem cannot be both public and staged for the same capability;
- public iteration is byte-for-byte equivalent to the declared public set;
- staged problems are accepted by runtime membership;
- staged problems do not appear through public iteration.

A staged problem can move to the public set only when its owning public capability body is activated with the required contract, examples, documentation, and client surfaces.

## Consequences

### Positive

- Internal fail-closed behavior can return the correct typed problem.
- OpenAPI, AsyncAPI, JSON Schema, JSON-LD, OKF, CLI, and SDK output do not change by accident.
- Each capability keeps an exact list of the failures it can own.
- Reviewers can see which behavior is implemented and which behavior is public.
- Tests detect duplicate, overlapping, or leaked problem entries.

### Cost

- A new internal failure needs an explicit capability-table entry.
- Activation requires moving the problem between named sets and updating all public surfaces in one governed change.
- A capability with many staged failures needs careful review, even though the runtime lookup remains bounded.

## Security properties

- The design keeps authorization and failure ownership fail-closed.
- It does not accept arbitrary problem codes.
- It does not reveal later-body behavior through public discovery.
- It does not weaken the safe-state or retry rules owned by each problem.
- It prevents a valid denial from becoming a process panic because public activation is later.

## Offline, package, and performance effect

The policy uses fixed static slices in the application crate. It performs no network call, file operation, database query, process launch, or background task. It behaves the same in a native binary, a package, and an OCI image.

Membership checks are bounded by the small problem lists owned by one capability. The exhaustive partition checks run only in tests.

## Public contract effect

This decision changes no public route or data shape by itself. Public contract artifacts must change only when a staged problem is intentionally activated.

| Surface | Rule |
| --- | --- |
| OpenAPI | Add the response only with public capability activation. |
| AsyncAPI | Add or change a message only when the event path can emit the public problem. |
| JSON Schema | Change only when a public payload changes. |
| JSON-LD and OKF | Change only when public meaning changes. |
| CLI and SDK | Add typed behavior only with the public contract. |
| Knowledge documentation | Add recovery guidance with public activation. |

## Verification

The application unit suite checks every capability partition. The contract generator consumes only public iteration. Exact-head CI and the governed contract conformance workflow verify that generated artifacts have no drift.

## Related records

- [B2 continuation security and QA review](../reviews/2026-08-23-b2-continuation-security-qa.md)
- [Capability ledger](../capability-ledger.md)
- [Contract ownership guide](../../contracts/README.md)
- [Security policy](../../SECURITY.md)
