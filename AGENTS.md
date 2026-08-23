# AGENTS.md

## Master integrator handoff

Before planning or implementation, read:

[`docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md`](docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md)

This document is the repository entry point for new integrators. It explains the product boundary, source-of-truth order, B0-B8 roadmap, architecture rules, current PR context, required evidence, and known mistakes to avoid.

## Skill routing

When the user's request matches an available skill, invoke it. When in doubt, use the review or planning skill before implementation.

Key routing rules:

- Product ideas or brainstorming → `/office-hours`
- Strategy or scope → `/plan-ceo-review`
- Architecture → `/plan-eng-review`
- Design-system or design-plan review → `/design-consultation` or `/plan-design-review`
- Full review pipeline → `/autoplan`
- Bugs or errors → `/investigate`
- QA or behavior testing → `/qa` or `/qa-only`
- Code or diff review → `/review`
- Visual polish → `/design-review`
- Shipping, deployment, or pull requests → `/ship` or `/land-and-deploy`
- Save progress → `/context-save`
- Resume context → `/context-restore`
- Backlog-ready specification or issue → `/spec`

## Read before changing the repository

Read these surfaces before planning or implementation:

1. [`README.md`](README.md)
2. [`docs/constitution.md`](docs/constitution.md)
3. [`docs/definition-of-done.md`](docs/definition-of-done.md)
4. [`ROADMAP.md`](ROADMAP.md)
5. [`docs/capability-ledger.md`](docs/capability-ledger.md)
6. [`contracts/README.md`](contracts/README.md)
7. [`SECURITY.md`](SECURITY.md)

The full guidance remains below this handoff entry point.