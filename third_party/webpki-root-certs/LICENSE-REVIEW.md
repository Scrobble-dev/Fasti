# webpki-root-certs 1.0.9: bounded dependency licence record

Recorded: 2026-09-05. Scope: the existing locked package, not a general
licence approval, legal opinion, or new supported platform.

## Exact evidence

- Package: `webpki-root-certs 1.0.9` from crates.io.
- Package SHA-256: `b96554aa2acc8ccdb7e1c9a58a7a68dd5d13bccc69cd124cb09406db612a1c9b`.
- Cached package provenance: upstream commit
  `0a553dbc8b3f18ea05c4f881cffa3f2d005d0d30`, path `webpki-root-certs`.
- Retained [`LICENSE`](LICENSE) SHA-256:
  `e271993808fec50ab29350b39539cdec611a9103f827e0aa26d61da70e2d33f8`.
- The retained text matches the package's `LICENSE` and the
  [tagged upstream agreement](https://raw.githubusercontent.com/rustls/webpki-roots/v/1.0.9/LICENSE-CCADB).
- The [upstream repository](https://github.com/rustls/webpki-roots/tree/v/1.0.9)
  identifies the data as derived from CCADB under CDLA-Permissive-2.0.

The package and checksum already exist in merged `dev`
`df09101028a988a92f4546313c5eed6dd20d238a`. C2 does not add or upgrade them.

## Decision and obligations

The [official agreement](https://cdla.dev/permissive-2-0/) permits use,
modification and sharing under its terms. Section 2.1 requires the agreement
text to be available with shared data. The repository retains that text and
links it from `NOTICE`; it does not relicense the data as Fasti code.

Use cargo-deny's existing
[package-specific exception](https://embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html#exceptions)
for exactly `webpki-root-certs =1.0.9`. Do not add CDLA-Permissive-2.0 to the
general allowlist. A package/version change requires renewed review.

`rustls-platform-verifier 0.7.0` declares this dependency for wasm32, while its
non-Apple, non-Android Unix path uses `rustls-native-certs`. The all-target
scanner combines target dependency edges. The checked Linux dependency graph
contains no `webpki-root-certs`; this source evidence is not a binary-content
audit or a claim that Fasti supports WebAssembly.

Before distributing an artifact containing this data, its owner must include
the retained agreement with that artifact and verify recipient access. Reuse
the artifact's packaging/notice path; an SPDX identifier or SBOM alone is
not the agreement text. Existing binary-only OCI and checksum staging are
not a general dependency-notice bundle. No packaging compliance or packaged
Tauri authentication is claimed here.

## Verification

Run `cargo deny --all-features check licenses sources` with the unchanged
lockfile. Check the package and retained text against the hashes above.
The exception must match only this exact crate version. No advisory,
unknown-source, confidence, or global licence check is weakened.
