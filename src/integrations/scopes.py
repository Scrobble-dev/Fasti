"""Supported least-privilege scopes for third-party integration credentials."""

INTEGRATION_SCOPE_DESCRIPTIONS = {
    "scrobble:write": "Send playback start, pause, and stop events.",
    "progress:read": "Read durable playback progress and ordered progress changes.",
    "progress:write": "Write or clear durable playback progress.",
    "watchlist:read": "Read saved-library membership and ordered membership changes.",
    "watchlist:write": "Add or remove saved-library membership.",
    "watched:read": "Read exact current watched state and ordered watched-state changes.",
    "watched:write": (
        "Set or clear exact current watched state without deleting viewing history."
    ),
    "catalog:read": "Read a future explicitly shared catalog surface when available.",
}

INTEGRATION_SCOPES = frozenset(INTEGRATION_SCOPE_DESCRIPTIONS)

NUVIO_RECOMMENDED_SCOPES = (
    "scrobble:write",
    "progress:read",
    "progress:write",
    "watchlist:read",
    "watchlist:write",
    "watched:read",
    "watched:write",
)
