# PR 93 authentication and dormant-session review

Status: **VERIFIED AT IMPLEMENTATION HEAD**

This review began from local documentation-lane base
`bb11a838dec7f304e074b8be6c1af0b2127c741c`. Independent session-security and
contract/UI reviewers cleared implementation head
`24fcc39a9276d480cefa18ad29635dd96a7953b2` on 2026-08-29. The source-bound
contract receipt records tree `5b9db5903446499a0768cb5a3ff2f92aaeeecc3b`
with `dirty: false`.

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

The exact-head Playwright run below covers the applicable permanent PR A
surface. Setup interaction remains not applicable until C1 activates the
separate first-run flow.

## Contract disposition

| Surface                   | PR A disposition                                                                                                                                                           |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenAPI and generated SDK | No production human-auth or browser-session HTTP operation, DTO, client method, or security scheme. Governed `later_body` capability and problem metadata remains visible. |
| AsyncAPI                  | Not applicable. PR A adds no external asynchronous authentication or session event channel.                                                                                |
| JSON-LD                   | Not applicable. Authentication secrets and session state are private security state, not public semantic entities.                                                         |
| Public CLI                | Not applicable. PR A uses direct deterministic fixtures and exposes no public authentication or session command.                                                           |

## Exact-head delivery gate

- Rust, TypeScript, browser, contract, security, native, and OCI checks apply
  only where the final PR A diff and repository gate require them.
- Documentation and link checks apply to the exact final commit.
- Each skipped or not-applicable gate must state its reason.
- Temporary review workflows and artifacts must be removed before merge.
- No earlier commit result may be reused as proof for a later commit.

Recorded at `2026-08-29T23:08:29+01:00` for implementation head
`24fcc39a9276d480cefa18ad29635dd96a7953b2`:

| Gate                                | Exact command or evidence                                                                         | Outcome                                                                                                                                                              |
| ----------------------------------- | ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Canonical contract                  | `cargo xtask contract verify --locked`                                                            | Passed 27 gates; receipt bound to the clean exact head and tree above                                                                                                |
| Browser UI                          | `pnpm test:ui`                                                                                    | 82 passed; includes A permanent task map, separate unavailable C first-run flow, Tabler themes, axe, keyboard, reduced motion, forced colors, and 320-1920 px reflow |
| Session and migration               | Included in canonical Rust workspace and HTTP conformance gates, plus focused reviewer inspection | Passed; no code-level blocker                                                                                                                                        |
| Independent session-security review | Read-only audit of the exact head after all earlier findings were fixed                           | Clear; delivery-ledger wording was the only remaining blocker                                                                                                        |
| Independent contract/UI review      | Read-only audit of the exact head after all earlier findings were fixed                           | Clear; Gate 10 A+C remains truthful                                                                                                                                  |
| AsyncAPI                            | PR A adds no authentication/session event channel                                                 | Not applicable                                                                                                                                                       |
| JSON-LD                             | Private authentication/session security state is not a public semantic entity                     | Not applicable                                                                                                                                                       |
| Public CLI                          | PR A exposes no public authentication/session command                                             | Not applicable                                                                                                                                                       |
| Native runtime                      | PR A exposes no production human-auth route or native listener                                    | Not applicable                                                                                                                                                       |
| Release                             | The programme forbids release from this work                                                      | Not applicable                                                                                                                                                       |

The documentation-only evidence commit must rerun the canonical contract gate
before push so its generated receipt binds the delivered source tree.
