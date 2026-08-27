# Media-server webhook integrations

Fasti can accept completed playback observations from Plex, Tautulli, Jellyfin, and Emby without giving those systems direct access to Fasti storage.

All production webhook routes use the same application boundary as `POST /api/v1/observations`:

```text
provider webhook
      |
      v
bounded provider decoder
      |
      v
scoped Fasti bearer authentication
      |
      v
canonical consumption occurrence
      |
      v
durable evidence + idempotent receipt
```

A webhook adapter never writes directly to SQLite. A provider title is evidence, not identity. Fasti uses exact provider identifiers when they are present and keeps unresolved identity as a valid state.

## Listener boundary

The normal non-loopback `FASTI_LISTEN` surface remains health-only.

To accept a webhook from another machine, configure the separate integration listener:

```text
FASTI_DATA_ROOT=/var/lib/fasti
FASTI_INTEGRATION_LISTEN=127.0.0.1:8421
```

For a reverse proxy on the same machine, keep this listener on loopback and terminate TLS at the proxy.

If the integration listener itself binds to a non-loopback address, Fasti refuses to start it unless the operator explicitly confirms that a trusted TLS proxy protects the connection:

```text
FASTI_INTEGRATION_LISTEN=192.168.1.20:8421
FASTI_INTEGRATION_TLS_TERMINATED=true
```

`FASTI_INTEGRATION_TLS_TERMINATED=true` is an operator assertion. It does not add TLS. Do not use it unless a reviewed proxy or network boundary supplies authenticated encrypted transport.

The integration listener exposes only:

```text
GET  /api/v1/health
GET  /api/v1/integrations
POST /api/v1/integrations/nuvio/webhook
POST /api/v1/integrations/tautulli/webhook
POST /api/v1/integrations/jellyfin/webhook
POST /api/v1/integrations/emby/webhook
POST /api/v1/integrations/plex/webhook
```

It does not expose node bootstrap, generic record mutation, or the generic observation endpoint.

## Credentials

Create one Fasti API client per adapter or source device from **Connections -> API clients** in the trusted packaged host.

Grant only:

```text
observation_accept
```

Send its one-time credential as:

```http
Authorization: Bearer <credential>
```

Do not put a bearer credential in:

- a webhook URL;
- a query string;
- a log message;
- a screenshot;
- a fixture;
- browser storage.

Revoke a client when the source is retired or a credential may be exposed.

## Shared template contract

Nuvio, Tautulli, and the Jellyfin Webhook plugin can send the provider-neutral template below. The request must use `Content-Type: application/json` and be at most 64 KiB.

```json
{
  "source_event_id": "stable-provider-event-id",
  "observed_at": "2026-08-27T12:00:00Z",
  "occurred_at": "2026-08-27T11:59:58Z",
  "item_type": "episode",
  "title": "Episode title",
  "series_title": "Series title",
  "season_number": 1,
  "episode_number": 4,
  "completed": true,
  "position_seconds": 1440,
  "duration_seconds": 1440,
  "provider_ids": {
    "imdb": "tt1234567"
  },
  "series_provider_ids": {
    "tmdb": "12345"
  },
  "server_id": "server-id",
  "user_id": "provider-user-id",
  "device_id": "player-id"
}
```

Rules:

- `source_event_id` is stable across delivery retries.
- `completed` must be `true` for this Chronicle route.
- Partial progress is rejected. It needs the separate progress capability.
- At most 16 item and series identifiers are accepted in total.
- Identifier values and textual evidence are bounded.
- A duplicate event with the same evidence returns the original receipt.
- Reuse of the same event identity with changed evidence returns `409 idempotency_conflict` and keeps prior state.

## Jellyfin

Use the current Jellyfin Webhook plugin's **Generic** destination. It supports custom request headers and Handlebars templates. Configure the `Authorization` header with the Fasti bearer credential and select playback-stop notifications.

The plugin exposes the fields needed for the Fasti contract, including:

- `ServerId`;
- `NotificationType`;
- `UserId`;
- `DeviceId`;
- `ItemId` and `ItemType`;
- `Provider_imdb`, `Provider_tmdb`, and `Provider_tvdb`;
- `PlaybackPositionTicks`;
- `RunTimeTicks`;
- `PlayedToCompletion` on playback stop.

The template must emit `completed: true` only when `PlayedToCompletion` is true. Do not convert every playback stop into history.

Fasti stores the original bounded JSON request as evidence and then normalizes it through the shared occurrence service.

## Tautulli

Use Tautulli's webhook notification agent. It supports custom headers and JSON bodies. Configure a watched/completed notification and send the shared Fasti template with a stable event identity assembled from Tautulli/Plex session and media identifiers.

Prefer Tautulli for Plex deployments that need direct Fasti authentication. Tautulli can send a custom `Authorization` header; Plex's native webhook configuration is URL-oriented and does not provide a Fasti-specific secret header.

## Plex

Plex native webhooks POST a `multipart/form-data` body containing a JSON part named `payload` and can include an optional JPEG part. Fasti:

- limits the complete multipart request to 512 KiB;
- extracts only the `payload` part;
- limits JSON payload evidence to 64 KiB;
- ignores the optional image;
- accepts only `media.scrobble` as a completed occurrence;
- keeps Plex rating keys and supported GUIDs as identity evidence;
- never treats `media.stop` alone as completion.

Plex does not supply a Fasti bearer header. Therefore the native Plex route is secure only behind a trusted proxy that injects the scoped Fasti `Authorization` header after authenticating and restricting the Plex source.

If you do not operate such a proxy, use Tautulli instead.

Fasti intentionally does not support a secret in the Plex webhook URL.

## Emby

The Emby adapter accepts native JSON events. It accepts only:

```text
playback.stop
item.markplayed
```

A playback-stop event must also contain explicit completion evidence such as `PlayedToCompletion` or `Item.UserData.Played`.

Fasti reads the item ID, provider IDs, series ID, runtime, playback position, item type, and timestamp when present. Emby time values in ticks are converted to seconds at the adapter boundary.

Unsupported or ambiguous events return a typed error and create no occurrence.

## Runtime status

`GET /api/v1/integrations` is a safe, non-secret runtime capability surface. The Connections interface reads it instead of maintaining a parallel browser status table.

Possible states are:

```text
available
setup_required
active
degraded
disabled
unsupported
error
```

Endpoint readiness is not the same as an active source. `setup_required` means the adapter exists but the external system still needs configuration.

## Troubleshooting

| Result | Meaning | Action |
|---|---|---|
| `400` | Malformed JSON | Fix the sender's request encoding. |
| `401` | Missing, invalid, or revoked Fasti credential | Create or rotate the scoped API client. |
| `403` | Credential does not have `observation_accept` | Use the intended scoped client. Do not widen unrelated credentials. |
| `409` | Same provider event identity arrived with different evidence | Keep the prior event. Fix the sender's event identity policy. |
| `413` | Body exceeded a documented bound | Reduce the webhook payload. Do not send artwork or raw library objects. |
| `415` | Wrong media type | Use the provider-specific content type. |
| `422` | Event is incomplete, ambiguous, or unsupported | Send an explicit completed occurrence; use the progress capability for partial state. |
| `500` | Durable state failed an integrity check | Retry is not safe to automate; investigate server-side storage integrity. |
| `503` | Local storage is unavailable | Retry once storage recovers. |
| `507` | Bounded evidence or observation capacity is exhausted | Wait for capacity to free up before retrying. |

## Security checklist

Before enabling a remote webhook:

1. Use one revocable scoped client per source.
2. Use TLS for non-loopback traffic.
3. Do not log or embed the bearer secret in a URL.
4. Do not forward authorization headers across hosts.
5. Restrict reverse-proxy source networks where practical.
6. Keep body and proxy limits at or below Fasti's documented limits.
7. Test credential revocation.
8. Test duplicate delivery and changed-evidence conflict behavior.
9. Confirm that bootstrap and generic mutation routes are absent from the integration listener.
10. Keep playback independent from Fasti availability.
