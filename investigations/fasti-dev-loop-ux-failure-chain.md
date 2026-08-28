---
type: concept
title: >-
  Investigation: fasti dev-loop UX failure chain (port fallback + podman build
  hint)
ingested_via: put_page
ingested_at: '2026-08-27T18:57:18.168Z'
source_kind: put_page
tags:
  - dev-loop
  - fasti
  - investigation
  - podman
---

## Symptom
User ran `fasti`, `fasti --podman`, then `fasti open` and hit three failures in a row:
1. `FASTI_PORT 8420 is already in use` (native)
2. Same error under `--podman`
3. `fasti open` -> "Container image fasti:b0 is not available. Build it with: podman build --tag fasti:b0 ." -> user ran the suggested command minus the trailing `.` -> "Error: no context directory and no Containerfile specified"

## Root causes
1&2. Not a code bug. `~/.zshrc`'s `fasti()` function was fixed 2026-08-27 to
   `export FASTI_PORT_FALLBACK="${FASTI_PORT_FALLBACK:-auto}"` before calling
   `scripts/dev.sh`, and `scripts/dev.sh`'s `_start_container` already
   correctly implements the auto-fallback (publishes to an ephemeral port
   when the preferred one is taken). No stale override found in any rc file.
   The pasted transcript is almost certainly from a shell session opened
   before the fix landed -- it needs `source ~/.zshrc` or a new terminal.

3. Real bug, fixed in `scripts/dev.sh:402` (`_require_container_image`).
   The build hint ended in a bare ` .` (the build context arg) which is
   visually indistinguishable from a sentence-ending period, so users drop
   it. It was also cwd-dependent (a bare `.` only resolves correctly when
   run from the repo root). Fixed by printing `$PROJECT_ROOT` (absolute
   path) instead of `.`. No test asserted the old message text.

## Fix
Branch `claude/fasti-build-hint-fix` (based on origin/dev), one-line change,
`./scripts/dev.sh --self-test` passes.
