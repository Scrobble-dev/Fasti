# Product

<!-- impeccable:product-schema 1 -->

This record is inferred from the explicit documentation-portal brief and the
accepted Fasti repository sources. The user authorized the recommended option
at workflow gates. No product fact below is inferred from visual taste.

## Platform

web

## Users

Fasti documentation serves five outcome groups:

- people who want to understand and keep portable media activity;
- operators who run and recover a private local node;
- integration developers who use the HTTP API, CLI, SDK, or event contracts;
- extension authors who add provider, namespace, or mapping behavior;
- contributors who change and verify Fasti.

The exact persona identifiers come from
`tests/conformance/identity-uat-matrix.v1.csv`. AuDHD and screen-reader use are
cross-cutting requirements for every outcome group.

## Product Purpose

The portal explains Fasti from the same source commit as its code and governed
contracts. It helps a reader complete a real task, verify the result, recover
from a problem, and find the next task. It keeps contract, implementation,
runtime, and support states separate.

Success means that a reader can understand what Fasti can do now without
mistaking a contract, fixture, staged implementation, preview, or plan for a
supported release.

## Positioning

The portal is a deterministic projection of Fasti's governed source. A strict
publication allowlist selects public human content. The same build also exposes
the exact OpenAPI, AsyncAPI, JSON Schema, JSON-LD, OKF, capability, problem, and
SDK reference artifacts for that source commit.

## Operating Context

The portal is a static Docusaurus site in the Fasti monorepo. GitHub Pages hosts
one verified build at `https://fasti.scrobble.dev`. Cloudflare provides DNS only.
The site has no account, telemetry, hosted search, runtime database, or external
search request.

## Capabilities and Constraints

- Docusaurus owns routing, Markdown, navigation, and static rendering.
- `cargo xtask` owns public-content selection, deterministic projection,
  verification, and packaging.
- `contracts/registry/v1/capabilities.yaml` owns capability meaning.
- `docs/site.yaml` owns public visibility and routes only.
- `docs/personas.yaml` maps canonical personas to public task paths.
- `packages/deploy-plan` owns pure deployment-plan generation.
- The deployment planner supports bounded local review modes. Production stays
  visible and unavailable until its release gate passes.
- No public page may claim a supported Fasti release while the repository's B8
  evidence gate remains open.
- No secret may enter a URL, browser storage, log, manifest, fixture, screenshot,
  or telemetry event.

## Brand Commitments

The portal inherits `brand/DESIGN.md`, `brand/tokens/tokens.json`, the Fasti
logos, the editorial field-guide identity, and the rule “Fasti records. Players
play.” It uses direct institutional copy. It does not add gradients, ornamental
glows, generic card walls, engagement traps, or false confidence.

## Evidence on Hand

- Repository purpose and live support limits in `README.md`.
- Engineering rules in `docs/constitution.md` and
  `docs/definition-of-done.md`.
- Capability state in `contracts/registry/v1/capabilities.yaml`.
- Product and identity acceptance inventories in `tests/conformance/`.
- API, event, schema, JSON-LD, OKF, problem, and SDK artifacts in `contracts/`
  and `packages/sdk`.
- Approved brand assets in `brand/`.

There are no approved customer testimonials, public-release claims, pricing
claims, service-level claims, or ASD-STE100 certification claims. Future work
must not fabricate them.

## Product Principles

1. Show the exact current state before asking a reader to act.
2. Organize content around user outcomes, not repository structure.
3. Link each public claim to its authoritative source and exact build commit.
4. Keep unavailable work visible with its blocker, owner, and safe alternative.
5. Prefer local, static, accessible platform features over new services.

## Accessibility & Inclusion

The portal targets WCAG 2.2 Level AA and applicable EN 301 549 web and
documentation requirements. It uses stable layouts, visible focus, 44-pixel
targets, semantic landmarks, clear headings, keyboard operation, reduced
motion, high contrast, progressive disclosure, and no color-only meaning.

Public prose uses a controlled Fasti termbase and deterministic ASD-STE100 Issue
9 checks. Machine checks do not certify compliance. A page remains labeled
“STE-controlled draft” until its human review record changes that state.
