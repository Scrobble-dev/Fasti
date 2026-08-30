# Provider authoring contracts

Provider manifests and response fixtures are authored files. The schema under
`generated/` is contract-generator output and must not be edited or passed to
the focused checker.

Run the offline first-success check from the repository root:

```bash
cargo xtask integration check contracts/addons/examples/minimal-metadata-source/provider.yaml
```

Use `--output json` for automation. Exit `0` means pass, `2` means authored
manifest, fixture, or compatibility validation failed, and `1` means the local
tool or environment failed. The command does not use a provider credential,
make a network request, or rewrite generated output.

The original manifest, schema, examples, and synthetic fixtures in this
directory are available under `Apache-2.0 OR AGPL-3.0-or-later`. Fasti runtime
implementation remains under the repository licence.
