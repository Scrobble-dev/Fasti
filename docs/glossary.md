# Fasti Glossary

These terms have one meaning across the domain, API, CLI, SDK, contracts, documentation, and future UI.

| Term | Meaning |
|---|---|
| Record | Stable local identity for a media entity. A Record does not belong to a metadata provider. |
| External identifier | Typed provider or catalogue claim attached to a Record at a declared grain. |
| Grain | The identity level being described, such as work, series, season, episode, release, edition, recording, track, or chapter. |
| Observation | Immutable report that something was seen or reported by a source. It may be unresolved. |
| Occurrence | Domain fact that a consumption-related event happened for an actor at a time. An Observation does not automatically become an Occurrence. |
| Evidence | Opaque or structured source material preserved with provenance and a digest. |
| Assertion | Directional, typed claim relating identities or ranges, with provenance and lifecycle state. |
| Interpretation | Current append-only reading of observations and evidence. It can be revised without rewriting the originals. |
| Review item | Durable unit of ambiguous or conflicted work that can be inspected, deferred, resumed, and resolved. |
| Operation | Durable application attempt with an ID, state, authorization context, and result or typed problem. |
| Receipt | Replayable proof of the result and durability boundary of an operation. |
| Provider | External source of metadata, identity claims, observations, or evidence. It is never Fasti's canonical identity owner. |
| Adapter | Infrastructure or delivery translation at a boundary. Adapters consume application ports and cannot own domain policy. |
| Capability | Versioned user or integrator action whose lifecycle, permissions, surfaces, examples, and errors are governed together. |
| Profile | Isolated local data and authorization boundary within a Fasti node. |
| Correction | New audited interpretation that supersedes an earlier interpretation without changing the original observation or evidence. |
| Tombstone | Append-only statement that data is withdrawn from active interpretation while its audit meaning remains governed. |
| Player | Separate software that presents media. Fasti can receive its observations but never becomes the player. |
