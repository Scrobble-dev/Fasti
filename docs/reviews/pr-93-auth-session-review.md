# PR 93 browser account and session review

Status: in progress

This review applies to pull request 93. It records the safety and interaction checks that must pass on the final commit. It does not replace executable evidence.

## Required behavior

- A browser session is private to one browser user.
- Session secrets and CSRF proof are never returned after creation or written to a URL.
- Session inventory contains only sessions owned by the authenticated user.
- Revoking one listed session cannot revoke a different session.
- Revoking all other sessions keeps the current session active.
- A profile selection changes only the current session.
- A profile switch does not create broader access than the session already has.
- Account changes and deletion require current-password verification.
- The last viable administrator remains available.
- Passkey, authenticator, and OIDC controls do not report success until their server-owned capability exists.

## Interaction and accessibility checks

The final browser evidence must cover:

- keyboard-only sign-in, session review, confirmation, cancellation, and sign-out;
- visible focus and reliable focus return;
- descriptive labels, status text, and error recovery without color-only meaning;
- narrow viewport, 200 percent zoom, reflow, reduced motion, and target size;
- persistent outcomes for destructive actions;
- one clear primary action in each confirmation flow;
- no hidden state that requires the user to remember a transient message.

The review uses WCAG 2.2 Level AA and the applicable EN 301 549 web requirements as evidence targets. It also checks consistency, error prevention, recognition over recall, clear grouping, feedback, reversibility, and user control.

## Contract and delivery checks

- OpenAPI and the generated TypeScript client match the implemented routes.
- AsyncAPI is not changed unless an asynchronous browser-session event is added.
- Rust, TypeScript, browser, contract, security, native, and OCI checks apply to the exact final commit.
- Temporary review workflows and artifacts are removed before merge.
- No earlier commit result is reused as proof for a later commit.
