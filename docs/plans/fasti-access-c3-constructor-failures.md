# C3 framing constructor failure regression

Status: FOCUSED_GATES_PASS; PR_DELIVERY_PENDING.

Base: merged dev 48fa62e517cddde268678f8231850f0981176c0d,
tree 828783f4ddd2403b13694b8250602eb122db390a. This is identical to the source
tree used by the completed secret-lifetime preflight. Preserve all previous
qualification branches, artifacts and failed resource runs.

## Scope and ownership

Close one existing C3.0 constructor I/O-failure behavior gap. Do not implement
production encryption, a crypto provider, new recovery authority or memory
locking. No shared M4 schema 17/archive 7, registry, API, SDK, host, Workbench or
persistence files are in scope. Packaged Tauri authentication stays deferred.

One worker owns only qualification/access-c3-framing/src/tests.rs and that
package's README.md. The commander owns this plan, AGENTS.md and the existing
C3 qualification workflow's exact test-count field. An independent reviewer
checks the bounded final diff. No new dependency, test framework or fixture.

## Written implementation gate

1. Trace FrameWriter::new and the existing FaultSink and round-trip helpers.
2. Add one regression covering prefix-write failure before the first byte and
   one byte before the complete prefix. Use the real constructor/provider.
3. Assert BrokenPipe and the original injected error, exact bounded prefix
   output, no returned writer, and release of the sink's Rc ownership. Reuse
   the same borrowed key in a successful existing encrypt/decrypt fixture.
4. Update the full-suite count from 20 to 21 in the package instructions,
   AGENTS.md and existing workflow. Keep two compile-fail doctests and all
   failed/ignored/filtered-count checks unchanged.
5. Run full debug and release suites, formatting and Clippy with the existing
   pinned offline/locked/native-source procedure in a new isolated target.
   Coordinate local build resources with Metadata first. Record actual output
   and identities; cached or prior suites cannot stand in for this diff.
6. Independent correctness and Ponytail review must pass before committing
   the coherent slice. Applicable canonical/PR gates still precede delivery;
   no test-only result represents a merged PR or approved crypto profile.

## Evidence ceiling

The check proves constructor failure behavior and ownership release, not
physical erasure, guarded allocation, provider-local copies, native-error
injection, startup failure or complete secret cleanup. Those remain open C3.0
items. The preserved combined run separately failed on TrailBase's 96 MiB child
limit before KDF launch; this regression does not repair or rerun that fixture.

## Focused verification: 2026-09-08

One 35-line test was added; the other 20 unit tests and two compile-fail checks
remain unchanged. Adapter source and lock hashes remain
16794e3fb1b5cf4288e2b0d0e5a30207fd458a75f18accc02728425d6e6491eb and
7e60010dccc40d0319180b52cb0faa39ec5193df73b25660996d66258edbeab0.
Test source SHA256: c52a346783e7cfc70c446267ed14d96c060768f98ced1fb7fde5934d2dad9dc3.

Actual full debug and release runs each passed 21 unit tests and two doctests,
with zero failed, ignored or filtered cases. Pinned 1.97.1 formatting and strict
Clippy passed. The first formatting check requested wrapping the new assertion;
rustfmt applied that change before either build. Fresh package-local targets
used the bundled static native source with overrides unset, no system-library
fallback. The command group exited 0; Metadata received immediate slot release.
Logs and artifact identities are retained in the local
`.gstack/reviews/c3-constructor-focused-evidence-20260908.json` receipt.

Independent review cleared the exact test source, preserved error/ownership
assertions, documentation and CI counts. Ponytail review found no additional
abstraction or dependency: reuse of FaultSink and existing round-trip helpers
is the smallest meaningful regression. No UI was changed; visual/accessibility
checks are not a result of this headless slice.

Remaining before PR delivery: exact-head canonical verification, dependency
advisories/licence checks, applicable pre-landing reviews and hosted checks.
No PR, merge, crypto-profile approval or complete-cleanup claim is made here.
