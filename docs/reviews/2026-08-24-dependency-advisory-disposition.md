# Dependency advisory disposition — 2026-08-24

**Scope:** Open dependency and code-scanning alerts on `dev`.
**Method:** Reachability and upgrade-path analysis against the exact lockfiles, not advisory metadata alone.
**Scope correction:** 2026-09-05, source `21fb4a9e24b4724341302bf208399a6dda2283a6`.

---

## 1. `GHSA-wrw7-89jp-8q8g` / `RUSTSEC-2024-0429` — glib

**Status:** Existing tracked exception. Present in the Desktop product dependency
graph as well as the isolated benchmark. This advisory is not resolved.

| Field          | Value                                                                                |
| -------------- | ------------------------------------------------------------------------------------ |
| Package        | `glib 0.18.5`                                                                        |
| Advisory class | **Unsoundness** (RustSec informational), surfaced by Dependabot as GHSA medium       |
| Title          | Unsoundness in `Iterator` and `DoubleEndedIterator` impls for `glib::VariantStrIter` |
| Affected range | `>= 0.15.0, < 0.20.0`                                                                |
| First patched  | `0.20.0`                                                                             |
| Lockfiles      | `apps/desktop/src-tauri/Cargo.lock`; `benchmarks/b1/tauri-shell/src-tauri/Cargo.lock` |

### Current scope and evidence limit

The root workspace lockfile does not contain `glib`, but it is not the complete
product inventory. Both the separately locked Desktop and benchmark graphs
contain `glib 0.18.5`. The earlier benchmark-only conclusion was incorrect.

The benchmark policy declares `benchmark_only: true` and `product_surface: false`;
those properties do not apply to Desktop. A source search under both first-party
Rust source trees found no direct `glib::` or `VariantStrIter` references. This
does not prove transitive Tauri/GTK paths cannot reach the affected iterator.
No new reachable exploit was established by this bounded review.

The [RustSec advisory](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)
classifies the defect as unsoundness and lists versions from 0.20.0 as patched.
An informational classification is not a guarantee that an affected API is safe.

### Historical resolver evidence and upgrade constraint

The 2026-08-24 investigation used `tauri 2.11.5` and could not resolve a
`>=2.12` release from its registry snapshot. This is historical evidence, not a
claim about the latest available Tauri release today.

The dependency is structural to the Tauri Linux stack:

```text
glib 0.18.5
├── atk 0.18.2   → gtk 0.18.2 → muda, tao, wry, webkit2gtk, tauri-runtime{,-wry}, tauri
└── cairo-rs 0.18.5 → gdk 0.18.2 → gtk, webkit2gtk
```

A direct bump is refused by the resolver:

```text
error: failed to select a version for the requirement `glib = "^0.18"`
candidate versions found which didn't match: 0.20.0
required by package `gtk v0.18.2`
    ... which satisfies dependency `gtk = "^0.18"` of package `tauri v2.11.5`
```

The recorded GTK `^0.18` requirement cannot accept `glib 0.20.0` by a direct
version substitution. A supported stack upgrade or reviewed backport needs its
own compatibility and runtime evidence. Deleting GTK or the Desktop surface is
not an acceptable remediation. No dependency change was made in this correction.

### Why the repository gate passes

The current `.github/workflows/security.yml` scans the root, benchmark and
Desktop lockfiles separately. Its Desktop command explicitly retains the existing
exception:

```bash
cargo audit --file apps/desktop/src-tauri/Cargo.lock --ignore RUSTSEC-2024-0429
```

That exception is owned by the Desktop maintainer and leaves other advisories
fatal. A passing gate does not resolve this advisory. Open Dependabot alerts
[#1](https://github.com/Scrobble-dev/Fasti/security/dependabot/1) and
[#3](https://github.com/Scrobble-dev/Fasti/security/dependabot/3) were still present
at the 2026-09-05 read-only check. No alert was dismissed and no workflow or ignore
configuration was changed.

### Review trigger

Re-evaluate when **any** of these becomes true:

- a supported Tauri/GTK dependency path provides patched `glib`;
- a compatible backport becomes available for review;
- the advisory is reclassified from unsoundness to a reachable vulnerability;
- first-party code begins using the affected iterator, or transitive reachability is established;
- the next Desktop dependency review occurs.

Keep the affected Desktop dependency and the accepted exception visible. Neither
absence of direct calls nor a green audit with an ignore proves unreachability.

### Unrelated warnings in the same scan

The historical scan also reported `unic-ucd-ident 0.9.0` and
`unic-ucd-version 0.9.0` as unmaintained (`RUSTSEC-2025-0100`,
`RUSTSEC-2025-0098`). Both packages are in the current Desktop and benchmark locks,
not only the fixture. Their current upgrade paths were not reassessed by this
scope correction.

---

## 2. `js/incomplete-url-substring-sanitization` — resolved

**Status:** Fixed, not suppressed.

`scripts/validate-authored-contracts.mjs` asserted a vocabulary term with `JSON.stringify(expanded).includes("https://fasti.scrobble.dev/ns/v1/Observation")`.

The security framing did not apply: the input is a local governed contract file, not attacker-controlled, and this is not a sanitization path. The alert nonetheless identified a real defect. A substring search accepts any longer IRI sharing the prefix, and accepts the term appearing only as a value.

Renaming the published term to `fasti:ObservationBatch` breaks the contract, and the old assertion passed anyway. The check now matches `@id` exactly and asserts the node is declared `rdfs:Class`. Verified fail-closed against both mutations.

No suppression comment and no rule configuration change was added.

---

## Rule

An alert is resolved by a fix, by proven unreachability, or by a recorded acceptance carrying evidence and a review trigger. It is never resolved by silence.
