# Fasti handoffs

This directory holds onboarding documents for engineers and autonomous harnesses joining Fasti without prior conversation context.

## Precedence

Read in this order. A document lower in the list never overrides one above it.

| Order | Document | Kind | Purpose |
| --- | --- | --- | --- |
| 1 | Current source, PR diff, and exact-head evidence | live | The only authority on what is true now |
| 2 | [`FASTI_AGENT_MEMORY_2026-08-26.md`](FASTI_AGENT_MEMORY_2026-08-26.md) | dated consolidation | Latest cross-session synthesis of product doctrine, architecture, PR/review history, gstack learnings, current #61/#67 cautions, operator context, and next-agent rules. Verify every status statement against live evidence before use |
| 3 | [`FASTI_MASTER_INTEGRATOR_HANDOFF.md`](FASTI_MASTER_INTEGRATOR_HANDOFF.md) | evergreen | Product boundary, source-of-truth order, architecture and security invariants, programme model, required evidence, first 48 hours |
| 4 | [`FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-25.md`](FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-25.md) | dated snapshot | Historical B4 and PR #44 snapshot. Verify every branch, PR, status, and next-step statement against live evidence before use |
| 5 | [`FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-24.md`](FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-24.md) | dated snapshot | Pull-request topology, B0-B8 disposition, evidence gaps, continuation order, machine-readable context envelope |
| 6 | [`FASTI_EXTERNAL_HARNESS_HANDOFF_2026-08-24.md`](FASTI_EXTERNAL_HARNESS_HANDOFF_2026-08-24.md) | dated snapshot | Implementation companion: exact-head QA receipts, the B3 correction slice, its security review, and the CI regression post-mortem |
| 7 | [`FASTI_BRANCH_TOPOLOGY_CHANGE_2026-08-24.md`](FASTI_BRANCH_TOPOLOGY_CHANGE_2026-08-24.md) | dated snapshot | Branch and pull-request topology only. Overrides the branch and PR facts in documents 5 and 6, which are otherwise unchanged |
| 8 | [`FASTI_B3_CONTINUATION_2026-08-24.md`](FASTI_B3_CONTINUATION_2026-08-24.md) | dated snapshot | Verified B0-B2 status with the then-current activation gates, and the state of the in-progress B3 export slice |

The 26 August agent memory consolidates later repository and review history but remains a dated document. It does not override current source, exact-head checks, current PR comments, or newer handoffs.

Branch and pull-request facts in the two original 24 August programme handoffs are superseded by the dedicated topology document. Those handoffs were written on the same day by different sessions and overlap. They are both retained because each carries material the other does not. When they disagree outside branch and pull-request topology, prefer the programme-state handoff for programme state and the implementation companion for the B3 implementation slice. Prefer live source over every handoff.

## Rules

- A dated handoff records what was true when it was written. It is evidence, not permission.
- Never treat a status table in these files as more current than the repository.
- Do not mark an implementation body complete because code exists. Completion requires its declared evidence.
- A passing receipt belongs only to the exact commit and artifact set it names.
- After a material session, add a new dated handoff rather than rewriting an old one. Use the maintenance template in the master handoff.
- When automation, bots, or conflict-resolution tools change code, inspect the semantic diff. A clean merge or review result is not proof that mocked, insecure, or stale behavior was not reintroduced.

## Superseded

- The 2026-08-23 draft of `FASTI_MASTER_INTEGRATOR_HANDOFF.md` (commit `fccf48a6`) was replaced by the fuller 2026-08-24 version during the PR #17 reconciliation. Its content is preserved in git history and is a strict subset of the current file.
