# B4 UI quality harness

This document records the current B4 quality boundary. It does not describe a
public release or promise later infrastructure.

## Current surface

- `/` renders the local media Workbench.
- `/status` renders the separate service diagnostic.
- The browser reads generated health and Record contracts.
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

| Dimension     | Covered states                                                                                                               |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Routes        | Workbench root and service diagnostic                                                                                        |
| Session       | Signed out, valid memory-only bearer, rejected bearer                                                                        |
| Records       | Empty, bounded summary list, unavailable service                                                                             |
| Settings      | Browser client URL, saved display state, trusted-host controls disabled                                                      |
| Viewports     | 320, 768, and 1440 CSS pixels                                                                                                |
| Themes        | Light and Dark                                                                                                               |
| Accessibility | Keyboard, focus, Axe, contrast, reflow, text spacing, reduced motion, forced colors, and shared 44 CSS pixel product targets |

Do not add optimistic stories for a capability that has no real host command.
Add its states when its bounded context, contract or IPC adapter, typed recovery,
and end-to-end evidence land together.

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
