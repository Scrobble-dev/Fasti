# Native source-notice custody qualification

This isolated tool preserves complete selected native source files and their
embedded notices. It does not build, measure or execute cryptography. It adds
no runtime dependency and does not approve a C3 profile or licence choice.

## Frozen input and selection

Use the `LATEST.tar.gz` bundled in `libsodium-sys-stable` 1.24.0, not a newly
downloaded release archive. Its exact size is 2,082,843 bytes and SHA-256 is
`b20a92e7ec25b285eafa349d721a5bb27e3a8ba94c0816630a127883f1d1b3ab`.
This identifies the retained bundled snapshot, not an independently verified
match to a separately published libsodium release.

[selection.json](selection.json) freezes 135 relative source paths, sizes and
hashes. Their combined payload is 2,844,540 bytes. The sorted path list, with
one LF after every path, has SHA-256
`4e54f326616546771cfe0b98afd7e81d222d1947f917682e3d9bfebb491125ee`.

[original-inventory.json](original-inventory.json) retains the original 45
entries byte-for-byte, including its absent final newline. Its SHA-256 is
`b6876ac6f00ecc2900499218104e29ecc0d582f5eeea8719f418867cd7765009`.
Its historical absolute paths are provenance, not file inputs or extraction
destinations. No `/tmp` inventory or extracted source directory is needed.

The selection preserves those 45 entries and adds source, header, assembly and
build context, including Poly1305, Curve25519, AEGIS, optimized ChaCha/Salsa,
shared Ed25519/soft-AES helpers, and `build-aux/install-sh`. Source-only ARM
variants are included as context, not as proof of x86 execution.

## Run offline

Python 3 and a local filesystem with `O_NOFOLLOW` are required. The structural
file checks are exercised on Linux; this is not cross-platform qualification.
No network, compiler, external archive program or native executable is used.

From this directory, run the focused suite:

```sh
python3 -B -m unittest -v test_bundle.py
```

The suite uses explicitly labelled synthetic source bytes for structural
failures. Its success does not establish native source custody. Real-source
generation below checks the pinned archive and all 135 selected members.

Supply the retained archive path explicitly. Use a new artifact filename;
the output's parent directory must already exist:

```sh
python3 -B bundle.py build --archive /absolute/path/to/LATEST.tar.gz --output /absolute/new/path/native-notices.tar
python3 -B bundle.py verify /absolute/new/path/native-notices.tar
```

Generate a second artifact at a different new path and compare their SHA-256
values to check reproducibility. No output is overwritten. If a write or final
verification fails, the output remains as failed evidence; use a new path for
another attempt. A failure never prints a verified result. An interrupted
partial artifact must pass `verify` before it can be used.

## Artifact and verification boundary

The deterministic uncompressed USTAR contains exactly 138 regular members:

- 135 complete source files under `source/libsodium-stable/`;
- `metadata/selection.json`;
- `metadata/original-inventory.json`;
- `metadata/SCOPE.txt`.

Paths are sorted; file modes are 0644; owners and timestamps are zero; owner
names are empty. The complete artifact is bounded to 4 MiB. Input source is
checksum-pinned before tar parsing. Source files are read directly from the
archive; none are extracted or executed.

The verifier uses the checked-in, checksum-frozen selection and original
inventory, not the bundle's self-reported manifest. It checks every expected
member's actual bytes, rejects missing/extra/duplicate/unsafe/nonregular
members, and compares the complete artifact with canonical tar serialization.
That final comparison rejects altered headers, padding, truncated EOF,
concatenated archives and trailing material. Embedded metadata must also match
the trusted package bytes exactly. A changed package selection requires a
new explicit source review; this tool does not accept arbitrary profiles.

## What remains open

Full source preserves embedded notice text. It does not supply referenced
legal texts absent from the source archive, select among source-offered
alternatives, or establish legal clearance. A missing component notice entry
is an evidence gap, not proof of a licence violation. Keep `LICENSE`, `AUTHORS`
and differently worded source notices distinct; do not infer a component's
terms from its filename, symbols or another implementation's notice.

This is not complete distribution attribution, a per-platform linker map,
proof that retained components executed, or qualification of a new binary.
The measured KDF binary and its source/build receipts remain separate. An
unbound retained binary does not become qualified through its hash alone.
Dynamic runtime libraries and other distribution components remain outside
this sodium-source selection. No compiled binary is included.

The [C3 evidence gate](../../docs/plans/fasti-access-c3-native-attribution.md)
retains the independent review and delivery requirements. Production adoption,
integrated resource and secret-cleanup evidence, recovery authorities and
profile approval remain open. Shared M4 files, runtime interfaces, migrations,
archive versions and packaged Tauri authentication are unchanged.
