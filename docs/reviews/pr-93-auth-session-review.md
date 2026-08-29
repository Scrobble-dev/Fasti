# PR 93 authentication and dormant-session review

Status: **PENDING**

This review record begins from local documentation-lane base
`bb11a838dec7f304e074b8be6c1af0b2127c741c`. It does not claim that the final
PR A commit passed a gate. Results from an earlier PR head do not prove a later
commit. Record the exact final SHA, commands, timestamps, and outcomes before
changing this status.

## PR A required behavior

- TrailBase is the selected private human-account service. It runs as a
  separate, pinned, unmodified process.
- PR A contains a dormant Fasti browser-session foundation. It exposes no
  production human sign-in, session-issuance, inventory, or revocation route.
- Production human identity and browser sessions remain **Unavailable until PR
  C1** proves TrailBase proof exchange, stable subject resolution, account
  state, workspace membership, role, administrator continuity, and session
  issuance.
- Direct deterministic domain, application, and store fixtures test the dormant
  foundation. PR A must not create a production or fixture listener.
- A session uses an opaque random secret, stores only its digest, and has an
  exact public `BrowserSessionId` for inventory and revocation.
- Idle and absolute expiry, secret rotation, bounded activity updates,
  revocation, Origin and Host checks, strict CSRF, and session-local authorized
  profile selection are explicit domain rules.
- PR A has no production `BrowserUser`, local password account, development
  account, custom TOTP, simulated passkey, backup-code, or fabricated OIDC path.
- The Account and security surface states that sign-in and session controls are
  unavailable until C1. It does not report simulated success.
- Administrator continuity is a C1 activation gate. PR A must preserve the
  requirement without pretending that a human-account implementation exists.

## Interaction and accessibility acceptance

The final evidence for the owning UI package must cover:

- keyboard-only setup, review, confirmation, cancellation, and safe exit;
- visible focus and reliable focus return;
- descriptive labels, status text, and recovery without color-only meaning;
- narrow viewport, 200 percent zoom, reflow, reduced motion, and target size;
- persistent outcomes for destructive actions;
- one clear primary action in each decision flow;
- Tabler-first composition;
- all ten Nielsen heuristics, applicable Gestalt principles, and relevant
  AskTog and IxDF lenses;
- WCAG 2.2 Level AA and applicable EN 301 549 evidence.

These are acceptance requirements, not recorded pass results.

## Contract disposition

| Surface | PR A disposition |
| --- | --- |
| OpenAPI and generated SDK | No production human-auth or browser-session route. No generated surface is permitted for dormant-only behavior. |
| AsyncAPI | Not applicable. PR A adds no external asynchronous authentication or session event channel. |
| JSON-LD | Not applicable. Authentication secrets and session state are private security state, not public semantic entities. |
| Public CLI | Not applicable. PR A uses direct deterministic fixtures and exposes no public authentication or session command. |

## Exact-head delivery gate

- Rust, TypeScript, browser, contract, security, native, and OCI checks apply
  only where the final PR A diff and repository gate require them.
- Documentation and link checks apply to the exact final commit.
- Each skipped or not-applicable gate must state its reason.
- Temporary review workflows and artifacts must be removed before merge.
- No earlier commit result may be reused as proof for a later commit.
- This record remains pending until the final exact-head evidence is attached.
