# Prepare a first change

## Outcome

Make one bounded change in the correct owner and produce evidence that another
reviewer can reproduce.

## Before you start

Read `AGENTS.md`, the constitution, the Definition of Done, and the source owner
for the changed behavior. Active development targets `dev`.

## Change order

Use this order when the behavior crosses layers:

```text
user outcome
  -> domain rule
  -> application capability and authorization
  -> contract and fixtures
  -> storage or network adapter
  -> API, CLI, desktop, web, or documentation presentation
```

A generator or UI cannot skip an owner.

## Make the change

1. Confirm the exact source commit and clean worktree boundary.
2. Search for the existing owner and every caller.
3. Make the smallest complete change.
4. Add one runnable check for each new branch, error path, and user-visible state.
5. Update source documentation and generated surfaces together.

## Verify the result

Run the canonical pull-request gate:

```bash
cargo xtask test pr
```

The gate can run on intended changes, but its source-bound receipt requires a
clean committed tree. Keep local test results, pull-request checks, merge state,
and deployment state separate.

For rendered UI, also run the browser, accessibility, design, and manual review
gates named in the Definition of Done.

## Publication flow

The allowlisted documentation, persona rules, and versioned contracts go through
one generator and one artifact gate. GitHub Pages publishes that artifact.
Cloudflare provides a DNS-only CNAME for the public domain.

![Fasti documentation sources flow through cargo xtask, Docusaurus, Pagefind, artifact checks, GitHub Pages, and a DNS-only CNAME.](/diagrams/documentation-publication.svg)

## Problems and recovery

If the source head changes, fetch and classify the drift before rebasing or
merging. If another worktree owns the same surface, stop and preserve that
boundary. Do not reset, clean, stash, or overwrite another writer's changes.

## What to learn next

[Read the contract inventory](/reference/contracts/)

Content state: STE-controlled draft.
