# Read typed problems

## Outcome

Use a Fasti problem response to identify the failed capability, unchanged state,
retry rule, and next safe action.

## Problem shape

Fasti uses RFC 9457 Problem Details. A governed problem can contain:

- `type`: the canonical documentation URL;
- `title`: the stable short title;
- `status`: the HTTP status;
- `detail`: the bounded explanation;
- `capability_id`: the failed capability;
- `code`: the stable Fasti problem code;
- `correlation_id`: the request correlation identifier;
- `safe_state`: what remained unchanged or active;
- `retryability`: whether and when a retry is safe;
- `next_actions`: the allowed recovery action;
- `violations`: field-specific validation evidence.

## Recovery order

1. Stop automatic retry when `retryability` is `not_retryable`.
2. Read `safe_state` before changing local state.
3. Correct the named parameter or precondition.
4. Follow the named next action.
5. Keep the correlation ID. Do not include credentials or private payload data in
   a public report.

## Exact problem routes

Every generated problem type under `/v1/problems/` resolves to a human page.
The page content comes from the canonical application problem contract. It does
not redefine the problem.

## Empty and unknown states

If a response does not match the governed problem schema, retain the status,
headers, correlation data, and a redacted body. Treat the response as an unknown
transport failure. Do not invent a retry rule.

## What to learn next

[Read capability states](/reference/capabilities/)

## Source and review evidence

- [Canonical problem owner](https://github.com/Scrobble-dev/Fasti/blob/dev/crates/fasti-application/src/problems.rs)
- [Generated problem catalogue](https://github.com/Scrobble-dev/Fasti/blob/dev/contracts/generated/v1/problems.json)

Content state: STE-controlled draft.
