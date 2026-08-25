# Fasti handoffs

This directory holds onboarding documents for engineers and autonomous harnesses joining Fasti without prior conversation context.

## Precedence

Read in this order. A document lower in the list never overrides one above it.

| Order | Document                                                                                                 | Kind           | Purpose                                                                                                                           |
| ----- | -------------------------------------------------------------------------------------------------------- | -------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Current source, PR diff, and exact-head evidence                                                         | live           | The only authority on what is true now                                                                                            |
| 2     | [`FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-25.md`](FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-25.md) | dated snapshot | Canonical current handoff snapshot: B4 delivery, PR #44 topology, multi-domain UI, test receipts, and continuation vectors        |
| 3     | [`FASTI_MASTER_INTEGRATOR_HANDOFF.md`](FASTI_MASTER_INTEGRATOR_HANDOFF.md)                               | evergreen      | Product boundary, source-of-truth order, architecture and security invariants, programme model, required evidence, first 48 hours |
| 4     | [`FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-24.md`](FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-24.md) | dated snapshot | Pull-request topology, B0-B8 disposition, evidence gaps, continuation order, machine-readable context envelope                    |
| 5     | [`FASTI_EXTERNAL_HARNESS_HANDOFF_2026-08-24.md`](FASTI_EXTERNAL_HARNESS_HANDOFF_2026-08-24.md)           | dated snapshot | Implementation companion: exact-head QA receipts, the B3 correction slice, its security review, and the CI regression post-mortem |
| 6     | [`FASTI_BRANCH_TOPOLOGY_CHANGE_2026-08-24.md`](FASTI_BRANCH_TOPOLOGY_CHANGE_2026-08-24.md)               | dated snapshot | Branch and pull-request topology only. Overrides the branch and PR facts in documents 4 and 5, which are otherwise unchanged      |
| 7     | [`FASTI_B3_CONTINUATION_2026-08-24.md`](FASTI_B3_CONTINUATION_2026-08-24.md)                             | dated snapshot | Verified B0-B2 status with the three activation gates, and the state of the in-progress B3 export slice                           |

Branch and pull-request facts in documents 3 and 4 are superseded by document 5. Documents 3 and 4 were written on the same day by different sessions and overlap. They are both retained because each carries material the other does not. When they disagree, prefer document 3 for programme state and document 4 for the B3 implementation slice, and prefer live source over both.

## Rules

- A dated handoff records what was true when it was written. It is evidence, not permission.
- Never treat a status table in these files as more current than the repository.
- Do not mark an implementation body complete because code exists. Completion requires its declared evidence.
- After a material session, add a new dated handoff rather than rewriting an old one. Use the maintenance template in the master handoff.

## Superseded

- The 2026-08-23 draft of `FASTI_MASTER_INTEGRATOR_HANDOFF.md` (commit `fccf48a6`) was replaced by the fuller 2026-08-24 version during the PR #17 reconciliation. Its content is preserved in git history and is a strict subset of the current file.
