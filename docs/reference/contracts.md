# Contract inventory

Each contract surface has one owner. The documentation build copies exact public
artifacts. It does not merge them into one master schema.

## Raw resources

- [Production OpenAPI 3.1](/openapi.json)
- [Conformance OpenAPI 3.1](/openapi-conformance.json)
- [AsyncAPI transport](/asyncapi/transport.yaml)
- [Capability registry](/capabilities.json)
- [Problem catalogue](/problems.json)
- [JSON-LD context](/jsonld/context.jsonld)
- [JSON-LD vocabulary](/jsonld/vocabulary.jsonld)
- <a href="/okf/index.md">OKF index</a>
- [Documentation manifest](/docs-manifest.json)
- [Release and support state](/release.json)

## Ownership

| Meaning | Owner |
| --- | --- |
| Capability identity and lifecycle | Authored capability registry |
| HTTP operations | Rust and Utoipa plus the registry |
| Events and channels | Authored AsyncAPI plus the registry |
| Payload structure | JSON Schema 2020-12 projections |
| Domain linked data | Authored Fasti JSON-LD |
| Operational knowledge | Authored OKF bundle |
| TypeScript client behavior | Generated SDK |
| Human task explanation | Allowlisted Markdown |
| Public route and navigation | `docs/site.yaml` |

## Verify synchronization

```bash
cargo xtask docs verify --locked
```

The command checks owners, projections, routes, manifests, personas, controlled
language, and deterministic output. A generated page never becomes the source of
an operation or domain rule.

Content state: STE-controlled draft.
