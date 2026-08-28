# B4 UI quality harness

This document records the current B4 quality boundary. It does not describe a
public release or promise later infrastructure.

## Current surface

- `/` renders the local media Workbench.
- `/status` renders the separate service diagnostic.
- The browser reads generated health and Record contracts. The trusted desktop
  host validates health with the shared Rust contract before returning typed
  status over IPC.
- The Record list is a summary of at most 500 Records. The operation has no
  cursor.
- The browser owns no domain persistence or mutation queue.
- The browser stores only non-secret display preferences and the selected
  client service URL. Credentials remain in tab memory.
- Trusted-host settings and unsupported domain mutations remain disabled in the
  browser.

## Gate layers

```text
Svelte presentation and host adapters
  -> formatting, Svelte diagnostics, build, and UI policy checks
  -> deterministic Playwright journeys with Axe
  -> rendered design, contrast, reflow, target-size, and keyboard review
  -> packaged assistive-technology and release evidence when B4 closes
```

Playwright uses at most two workers. This keeps cold runs deterministic on
constrained hosts. Tests use isolated local ports and do not reuse another
worktree's server.

## Current state matrix

The automated matrix covers only states that the current host can produce:

| Dimension          | Covered states                                                                                                               |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| Routes             | Workbench root, browser history, and browser or packaged service diagnostic before and after setup                           |
| Session            | Signed out, valid memory-only bearer, rejected bearer                                                                        |
| Records            | Empty, bounded summary list, unavailable service, native failure, successful retry, and repeated failed retry               |
| Provider discovery | Authoritative provider status, explicit selection, search race, unconfigured state, and credential removal confirmation    |
| Review resolution  | Empty, open review, one mutation in flight, and resolved state                                                               |
| Settings           | Browser client URL, saved display state, trusted-host network and credential controls, and browser-disabled host controls   |
| Service status     | Healthy, unavailable, contract-invalid, duplicate retry prevention, setup-inspection failure, and focus recovery            |
| Viewports          | 320, 375, 768, 1024, 1440, and 1920 CSS pixels                                                                               |
| Themes             | Light, Dark, and distinct Night; every exposed accent, base, font, and radius persists and changes its advertised output     |
| Accessibility      | Workbench keyboard and modal containment, stable focus contrast, Axe in open and closed overlays, text enlargement, text spacing, reduced motion, forced colors, and shared 44 CSS pixel product targets |

Run the fast structural and focused interaction checks before the complete gate:

```bash
pnpm lint:ui
pnpm test:ui -- tests/e2e/workbench-regressions.spec.ts tests/e2e/control-target-regression.spec.ts
```

`pnpm lint:ui` normally returns in about one second. It names the shell or theme file and the required Tabler primitive. It also rejects accent-owned focus rings and finite component radii that bypass Tabler's scale. The focused browser command proves responsive geometry, Settings canvas ownership, theme truth and continuity, enlarged-text reflow, keyboard containment, focus return, Axe, and 44px targets. The full `pnpm test:ui` suite and `PKG_CONFIG=/usr/bin/pkg-config cargo xtask test pr` remain required before a pull request.

Do not add optimistic stories for a capability that has no real host command.
Add its states when its bounded context, contract or IPC adapter, typed recovery,
and end-to-end evidence land together.

Browser navigation aborts its active fetch. Tauri IPC has no `AbortSignal`
channel, so the shared status controller prevents concurrent retry commands and
ignores stale results. A native check that already started remains bounded by
the desktop host's 15-second timeout.

## Non-negotiable boundaries

- Headless daemon and contract gates remain independent of browser packaging.
- `fastid` and SQLite own domain persistence.
- The UI does not invent retry or mutation queues.
- UI code contains no analytics, session replay, external font, or CDN
  dependency.
- Unsupported controls stay visible and disabled when that preserves approved
  information architecture.
- Motion is functional, transform-based, and removed for reduced-motion users.
- Local tests do not depend on a cloud screenshot or performance service.

## Evidence limits

Automated Axe and browser checks do not prove WCAG or EN 301 549 conformity.
B4 closure still requires packaged keyboard and screen-reader evidence plus
testing with representative disabled and neurodivergent users. B8b requires a
separate release-bound design and accessibility receipt.
