# Dependency advisory disposition — 2026-08-24

**Scope:** Open dependency and code-scanning alerts on `dev`.
**Method:** Reachability and upgrade-path analysis against the exact lockfiles, not advisory metadata alone.

---

## 1. `GHSA-wrw7-89jp-8q8g` / `RUSTSEC-2024-0429` — glib

**Status:** Accepted, tracked. Not fixable at this baseline. Not present in the product.

| Field          | Value                                                                                |
| -------------- | ------------------------------------------------------------------------------------ |
| Package        | `glib 0.18.5`                                                                        |
| Advisory class | **Unsoundness** (RustSec informational), surfaced by Dependabot as GHSA medium       |
| Title          | Unsoundness in `Iterator` and `DoubleEndedIterator` impls for `glib::VariantStrIter` |
| Affected range | `>= 0.15.0, < 0.20.0`                                                                |
| First patched  | `0.20.0`                                                                             |
| Manifest       | `benchmarks/b1/tauri-shell/src-tauri/Cargo.lock`                                     |

### Why it is not in the product

`glib` appears **zero** times in the workspace `Cargo.lock`. It exists only in the isolated Tauri benchmark fixture, which declares its own workspace and is deliberately excluded from the product dependency graph.

The fixture is a 16-line empty shell that writes a ready-file so a governed Linux desktop run can measure resident memory. Its policy declares `benchmark_only: true` and `product_surface: false`. It contains no direct reference to `glib` or to `VariantStrIter`.

### Why it cannot be upgraded

`tauri 2.11.5` is the latest published release. `cargo add tauri@'>=2.12'` reports that no such version exists in the registry index.

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

No `0.18.x` patch exists; the advisory's first fixed version is `0.20.0`. Removing `glib` therefore means removing `gtk`, which means the fixture is no longer a Tauri Linux shell and can no longer produce the desktop memory receipt that B1 requires.

### Why the repository gate passes

`cargo audit` scans this lockfile explicitly in `.github/workflows/security.yml`:

```bash
cargo audit --file benchmarks/b1/tauri-shell/src-tauri/Cargo.lock
```

It reports the entry as `Warning: unsound` and exits successfully. Unsoundness advisories are informational: they record an API that can be misused to cause undefined behavior, not a reachable exploit. `.github/dependabot.yml` has no `cargo` entry for the fixture directory, so Dependabot will not raise a pull request for it either.

### Review trigger

Re-evaluate when **any** of these becomes true:

- a Tauri release adopts `gtk 0.20+` / `glib 0.20+`;
- the advisory is reclassified from unsoundness to a reachable vulnerability;
- the fixture stops being benchmark-only, or any `glib` API is called directly;
- `glib` appears in the product workspace `Cargo.lock`.

Until then this must not be described as a product vulnerability, and it must not be used to claim the fixture is unsafe to run.

### Unrelated warnings in the same scan

`cargo audit` also reports `unic-ucd-ident 0.9.0` and `unic-ucd-version 0.9.0` as **unmaintained** (`RUSTSEC-2025-0100`, `RUSTSEC-2025-0098`). Same disposition: fixture-only, informational, no upgrade path at this Tauri version.

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
