# Fasti branch topology change — 2026-08-24

**Status:** Dated snapshot. Records one change to branch and pull-request topology.
**Supersedes for branch facts only:** the branch and PR sections of the two earlier 2026-08-24 handoffs in this directory.
**Does not change:** product boundary, architecture, security invariants, milestone status, or evidence requirements.

> Read [`README.md`](README.md) in this directory for handoff precedence. Live source and exact-head evidence outrank every file here.

---

## 1. What changed

Work was spread across five branches with overlapping content, and two handoff documents shared one filename with divergent bodies. More seriously, the GitHub Actions workflows trigger only on `push` and `pull_request` to `dev` and `release`. The branch holding the entire B0–B3 implementation was neither, so pull requests targeting it reported **zero checks**.

The repository now uses the two-branch model its workflows already assume:

```text
feature branch → dev → release
```

`dev` was fast-forwarded from `8fc8173d` to `a676ca60`. That is 128 commits, a clean fast-forward. No history was rewritten and no commit was lost.

## 2. Current topology

| Ref                                                        | Role                                                                                |
| ---------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `release`                                                  | Stable target. Still the pre-truth-reset scaffold until #20 lands.                  |
| `dev`                                                      | Integration branch. Branch from here. CI runs on push and on pull requests into it. |
| `vendor/floppy-pr-791`                                     | Retained historical merge point. Do not delete.                                     |
| `vendor/floppy/plan/nuvio-integration-reviewed-2026-08-15` | Retained historical merge point. Do not delete.                                     |

### Pull requests

- **#20** — `dev` → `release`. The current implementation and review surface. Draft.
- **#14** — closed, superseded by #20. Its review history and discussion remain readable and are still worth reading.
- **#17** — closed, superseded by #18.
- **#18**, **#19** — merged.

## 3. Deleted branches

Each was verified fully merged with `git merge-base --is-ancestor` before deletion, and tagged first. Every tag is on the remote and dereferences to the original commit.

| Tag                                               | Commit     | Was                               |
| ------------------------------------------------- | ---------- | --------------------------------- |
| `archive/security-b1-b2-foundation-20260822`      | `d07d826d` | branch of the same name           |
| `archive/vscode-fasti-b0-truth-reset`             | `da4bf084` | branch of the same name           |
| `archive/security-test-noop-do-not-use`           | `f5603635` | branch of the same name           |
| `archive/pr17-pre-reconcile`                      | `e5db2bc2` | PR #17 head before reconciliation |
| `archive/security-b1-evidence-hardening-20260822` | `a676ca60` | the former build branch           |

Recover any of them with `git branch <name> <tag>`.

## 4. Rule for the next harness

Branch from `dev`, not from `release` and not from any archived branch name that appears in an earlier handoff.

Earlier dated handoffs in this directory instruct you to continue on `security/b1-evidence-hardening-20260822` and to treat `security/b1-b2-foundation-20260822` as a synchronized compatibility ref. **Both instructions are obsolete.** Those branches no longer exist. Their content is in `dev`. Those documents are otherwise still accurate and were deliberately left unedited, because a dated snapshot records what was true when it was written.

## 5. What did not change

- Milestone status. B0 complete; B1 software scope complete with physical Pi 5 and J4125 evidence open; B2 implemented behind ports for review and not publicly activated; B3 correction and workspace verification in, export/restore/equality not started.
- The evidence rule. A body is complete only when its declared evidence passes at exact head.
- The product boundary. Fasti records. Players play.
- The `release` branch contents, which remain the pre-truth-reset scaffold with red CI on its own tree until #20 lands.

## 6. Note on `release` CI

`release` fails `cargo fmt --check` and `cargo clippy -D warnings` inside its own pre-truth-reset crate set, has no `pnpm-lock.yaml`, has no `tsconfig.json` in any package, and fails `prettier --check` on 19 files. Every affected file is deleted by #20, so fixing them in place produces throwaway work and merge conflicts.

The crate names involved are deliberately not repeated here. `scripts/check-repository-truth.sh` treats them as forbidden product claims in active documentation, which is the correct guard: those surfaces do not exist in `dev`.

That red is also not a merge blocker. `release` branch protection requires one approving review and declares **no required status checks**. A pull request there showing `mergeable_state: "blocked"` is waiting on review, not on CI.
