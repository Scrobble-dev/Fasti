# Capability Ledger

This ledger prevents a planned route, command, SDK method, screen, or fixture from being mistaken for a working capability. B1 replaces this provisional Markdown ledger with a versioned machine-readable registry and deterministic generators.

Status values are `implemented`, `guarded`, `later body`, and reasoned `N/A`.

| Capability | Body | B0 state | HTTP / OpenAPI | SSE / AsyncAPI | CLI | Schema / JSON-LD / OKF | SDK | UI |
|---|---:|---|---|---|---|---|---|---|
| Process health | B0 | implemented | HTTP implemented; OpenAPI later B1 | N/A: no stream | N/A: HTTP probe | later B1 | health method implemented | N/A through B3 |
| Export workspace | B3 | guarded | later B3 | N/A: offline operation | nonzero guard | later B3 | later B3 | N/A through B3 |
| Restore workspace | B3 | guarded | N/A: stopped-node operation | N/A: stopped-node operation | nonzero guard | later B3 | N/A: local offline activation | N/A through B3 |
| Verify workspace | B3 | guarded | later B3 | N/A: finite operation | nonzero guard | later B3 | later B3 | N/A through B3 |
| Initialize node and enroll first client | B1/B2 | later body | later B1/B2 | later B1/B2 where needed | later B1/B2 | later B1 | later B1 | N/A through B3 |
| Accept observation and replay receipt | B1/B2 | later body | later B1/B2 | later B2 | later B1/B2 | later B1 | later B1 | N/A through B3 |
| Create Record and attach identifier | B2 | later body | later B2 | later B2 where needed | later B2 | later B2 | later B2 | N/A through B3 |
| Inspect, defer, resume, and resolve review | B2 | later body | later B2 | later B2 | later B2 | later B2 | later B2 | N/A through B3 |
| Append correction and inspect chain | B3 | later body | later B3 | later B3 | later B3 | later B3 | later B3 | N/A through B3 |
| Local media interface | B4 | later body | consumes governed capabilities | consumes governed streams | N/A: browser presentation | consumes generated models | consumes generated SDK | later B4 |
| Provider-neutral integration conformance | B6 | later body | later B6 | later B6 where needed | later B6 | later B6 | later B6 | later B6 report view |
| Nuvio one-way observation adapter | B7 | later body | only after B7 readiness | only after B7 readiness | only after B7 readiness | only after B7 readiness | only after B7 readiness | consumes existing actions |
| Supported packages and public release | B8 | later body | artifact-bound | artifact-bound | artifact-bound | signed release set | packaged SDK | packaged surface |

Every row becomes binding only in its named body. Reserved names do not authorize an implementation to invent request shapes or success behavior early.
