# AGENTS.md

## Start here

Before changing code, read these files in order:

1. [`docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md`](docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md)
2. [`docs/handoffs/FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-24.md`](docs/handoffs/FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-24.md)

The master handoff defines the durable product boundary, source-of-truth order, architecture and security invariants, programme model, required evidence, and first 48-hour onboarding sequence.

The dated context save records the active pull-request topology, current B0-B8 disposition, known evidence gaps, exact continuation order, relationships, and required handoff output for a new harness with no access to prior chat or local gstack state.

Then inspect the current repository state, active pull requests, exact-head checks, and controlling plans. A dated handoff never overrides newer source or evidence.

## Skill routing

When the user's request matches an available skill, invoke it via the Skill tool. When in doubt, invoke the skill.

Key routing rules:
- Product ideas/brainstorming → invoke /office-hours
- Strategy/scope → invoke /plan-ceo-review
- Architecture → invoke /plan-eng-review
- Design system/plan review → invoke /design-consultation or /plan-design-review
- Full review pipeline → invoke /autoplan
- Bugs/errors → invoke /investigate
- QA/testing site behavior → invoke /qa or /qa-only
- Code review/diff check → invoke /review
- Visual polish → invoke /design-review
- Ship/deploy/PR → invoke /ship or /land-and-deploy
- Save progress → invoke /context-save
- Resume context → invoke /context-restore
- Author a backlog-ready spec/issue → invoke /spec

## Design System

Always read [`brand/DESIGN.md`](brand/DESIGN.md) before making any visual or UI decisions.
All font choices, colors, spacing, and aesthetic direction are defined there.
Do not deviate without explicit user approval.
In QA mode, flag any code that does not match `brand/DESIGN.md`.
