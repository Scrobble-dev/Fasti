# Hostile observation fixtures

These synthetic bodies exercise the provider-neutral webhook boundary used by
/api/v1/integrations/jellyfin/webhook. They are intentionally rejected before
any durable mutation. The JSON files remain syntactically valid so the suite
can distinguish bounded contract violations from parser failures.

Each fixture has a sidecar provenance note. The values are synthetic and do
not contain provider credentials, personal data, or live service responses.

| Fixture | Boundary covered | Expected problem |
| --- | --- | --- |
| oversized-source-event-id.json | source_event_id exceeds the 256-byte contract bound | invalid_observation (422) |
| malformed-observed-at.json | observed_at is not an RFC 3339 instant | invalid_observation (422) |
| nested-title-object.json | strict JSON contract receives an object where a string is required | malformed_json (400) |

The cases were added for Fasti issue #48
(https://github.com/Scrobble-dev/Fasti/issues/48) and are checked through the
authenticated integration route rather than by calling a private parser
directly.
