# C3 native source-notice custody delivery gate

Status: `PREPARATION_VERIFIED_DELIVERY_GATES_OPEN`.

This bounded slice supports the approved [C3-CRYPTO gate](trailbase-authentication-remediation.md#72-c3-crypto-vault-and-backup-gate).
It preserves reproducible selected source notices. It does not approve a crypto
profile, licence choice, native build, production vault, recovery or public release.
The programme remains active. Packaged Tauri authentication stays deferred.

## Source and ownership

Delivery base: merged `dev` `0a43bd276f3bc4ab2c6ac4c527edb34c75df4859`,
tree `7a244bf97c9f548ecadee01c6ba483a5f455c493`.
Reviewed preparation: `fd5aeb78a2c004943beb29946ead1b57b9cf6a26`,
tree `1660ab51765b34bb211624da05720d4efa0ea39b`, in the separate research
branch. Import only its five package files. Do not merge that research branch,
its earlier plans, parser follow-up or unrelated C2/D0 work.

One package writer owns `qualification/access-c3-native-attribution/`.
The commander owns this plan, the brief AGENTS guidance, the existing
qualification workflow and integration.
Reviewers are additive and read-only. Metadata M4 retains schema 17/archive 7
and shared persistence, policy, registry/generator, API/SDK/host/Workbench
ownership. No next migration is allocated. No change in those surfaces, root
dependencies, runtime, VERSION or release policy belongs in this slice.

## Need and minimum design

The previous retained inventory covered 45 source files. Defined-symbol/source
review found additional Poly1305, Curve25519, AEGIS and optimized ChaCha/Salsa
implementation context, plus build helpers. A notice filename or SPDX label
does not preserve the complete differing permission and disclaimer texts.

Use the Python standard library and existing local file/hash/strict-JSON
patterns. Do not attach this tool to the B1 Git runner machinery or production
archive parser. The five files are a README, frozen selection, unchanged old
inventory, bounded bundler/verifier and one focused structural test suite.
No dependency, service, framework or generic archive abstraction is needed.

Pin the bundled `libsodium-sys-stable` 1.24.0 native source archive to exactly
2,082,843 bytes and SHA-256
`b20a92e7ec25b285eafa349d721a5bb27e3a8ba94c0816630a127883f1d1b3ab`.
This is not an independently verified match to a separately published release.
The frozen selection contains 135 relative paths, 2,844,540 source bytes;
the sorted LF-terminated path list has SHA-256
`4e54f326616546771cfe0b98afd7e81d222d1947f917682e3d9bfebb491125ee`.
Preserve the original inventory bytes and SHA-256
`b6876ac6f00ecc2900499218104e29ecc0d582f5eeea8719f418867cd7765009`.
Its absolute historical paths are provenance, not extraction destinations.

Read selected regular members directly from the checksum-pinned archive;
never extract or execute them. Copy complete source bytes, not regex-extracted
notice fragments. Deterministic uncompressed USTAR contains 135 source files
and three metadata records, bounded to 4 MiB. The verifier checks actual
content against trusted frozen package data, not its own embedded manifest.
Canonical reconstruction rejects changed metadata, padding, truncated EOF,
concatenated archives and trailing material. Refuse output replacement and
retain failed output without a verified result.

## Written delivery sequence

1. Import only the five reviewed package files; adapt the README link to this
   delivery plan. Preserve the existing dev AGENTS guidance and add one entry.
   Add the new path to both existing qualification workflow triggers and a
   read-only Python structural-test job. Do not call this native/source-build
   proof or change the existing Rust qualification/advisory jobs.
2. Verify the executable source, tests and both inventories are byte-identical
   to reviewed preparation. Independently review the final eight-path delta.
3. Run all 14 structural tests, exact-source generation/verification twice,
   deterministic byte comparison, a real tampered-notice rejection, Markdown
   links and whitespace checks. Validate the workflow and execute its exact
   test command; it must reject missing/skipped tests, ordinary failures and
   expected failures. Independent review reproduced an expected-failure bypass
   in the initial guard; explicitly reject `result.expectedFailures` and retain
   its negative sentinel rather than treating an expected failure as a pass.
   Retain failures and exact-source receipts.
4. Commit the coherent slice. Run unchanged `cargo xtask test pr` against the
   exact clean delivery head in addition to the Python-specific checks.
5. Complete applicable ship/documentation review, publish one PR against dev,
   reconcile exact-head hosted checks and review findings, then merge only
   when applicable gates pass. Verify the merged tree and notify metadata.

Tests must reject wrong archive identity; wrong source member hash/length;
missing, extra, duplicate, unsafe, link or nonregular members; duplicate-key or
malformed JSON; oversized and truncated inputs; one-byte notice tampering;
noncanonical framing; existing output and output symlink/FIFO replacement.
Clearly label synthetic structural fixtures. They are not native execution or
cryptographic tests. Add no performance or accessibility claim for this headless
tool; it does not change a UI or service listener.

## Retained preparation evidence

Independent review found no concrete issue and checked all 135 archive members.
The 14 structural tests passed. Two real artifacts were byte-identical:
2,990,080 bytes, SHA-256
`2203fb808a12a5be8b8994de208dd74be2fbec170e8293af4340cf1dc9579f0f`.
A one-byte change in the bundled LICENSE was rejected with CLI exit 1.
The altered artifact remains preserved. These results support preparation,
not a future changed delivery head without new exact-source checks.

Preparation bundler SHA-256:
`f1f65f2aceb17c8a2461d0d12ec79c9b57ffdca1f3554df326fa8180bda4d980`.
Test source SHA-256:
`b51dd4ab81b78b7054843334319096efa577d5a63b6c436b30afb198e7656be3`.
Selection SHA-256:
`dd6774843d132092f39c645706231b86a3d29c75558f40c956ca9f2424274c4f`.
Clean preparation receipt: `2026-09-05T23-59-47-528Z-native-notice-custody-929994-cea2e311.log`.

## Limits and rollback

Complete source preserves embedded texts, not externally referenced legal texts
absent from the pinned archive. Licence alternatives, legal clearance, full
distribution attribution and per-platform linker maps remain open. ARM source
context does not prove measured x86 execution. A source bundle cannot qualify
an unbound binary. Preserve KDF measurement source/build receipts separately.
No compiled binary, new native measurement or production crypto approval is
included. Integrated resources, secret cleanup and recovery authority remain
independent C3 gates. A missing notice entry is not proof of a licence violation.

Rollback affects only this isolated tool and its documentation. Preserve
retained source, failed evidence and generated artifacts; existing production
authentication, metadata, backups and recovery behavior remain unchanged.
