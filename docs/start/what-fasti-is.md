# What Fasti is

Fasti is a local system of record for media activity. It receives observations,
preserves evidence, reconciles identity, records occurrences, and exposes
governed history.

## The boundary

Fasti records. Players play.

Fasti does not decode, transcode, stream, or play media. A player, reader,
tracker, import tool, or integration can report an observation. Fasti then keeps
the report and its evidence inside the governed local record.

## Stable local identity

A Fasti Record has a stable local identity. A metadata provider does not own
that identity. IMDb, TMDB, TVDB, MusicBrainz, ISBN, and other identifiers are
typed claims attached to the Record.

Changing a preferred metadata source must not change the Fasti Record ID.

## Unresolved data

An unresolved item is valid data. Fasti can keep partial or conflicting evidence
without forcing an unsafe automatic match. A later interpretation can supersede
an earlier interpretation. It does not rewrite the original observation or
evidence.

## Current support state

Fasti is an engineering baseline. It is not a supported public release. Some
durable local routes are implemented. Other behavior is available only in a
fixture, staged behind internal ports, or planned for a later body.

Read [Current status](/start/current-status/) before an installation or
integration decision.

## Source and review evidence

- [Fasti constitution](https://github.com/Scrobble-dev/Fasti/blob/dev/docs/constitution.md)
- [Architecture overview](https://github.com/Scrobble-dev/Fasti/blob/dev/docs/architecture/overview.md)
- [Glossary](/reference/glossary/)

Content state: STE-controlled draft. Machine and human review records are
published with the documentation manifest.
