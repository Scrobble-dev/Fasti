"""Canonical authorization contract for third-party integration credentials."""

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

# Compatibility default for callers that do not provide an explicit scope list.
# New user-facing token creation should continue to request explicit scopes.
DEFAULT_INTEGRATION_SCOPES = (
    "scrobble:write",
    "progress:read",
    "progress:write",
    "watchlist:read",
    "watchlist:write",
    "catalog:read",
)

NUVIO_RECOMMENDED_SCOPES = (
    "scrobble:write",
    "progress:read",
    "progress:write",
    "watchlist:read",
    "watchlist:write",
    "watched:read",
    "watched:write",
)

# One fail-closed route-to-scope map is shared by the current PAT-style token
# authenticator and future standards-based authorization adapters. Unknown
# endpoints remain denied to scoped integration credentials.
INTEGRATION_SCOPE_RULES = {
    "api_scrobble": {
        "POST": "scrobble:write",
    },
    "api_playback_progress": {
        "GET": "progress:read",
        "HEAD": "progress:read",
        "PUT": "progress:write",
        "DELETE": "progress:write",
    },
    "api_playback_progress_snapshot": {
        "GET": "progress:read",
        "HEAD": "progress:read",
    },
    "api_playback_progress_changes": {
        "GET": "progress:read",
        "HEAD": "progress:read",
    },
    "api_watchlist": {
        "GET": "watchlist:read",
        "HEAD": "watchlist:read",
        "PUT": "watchlist:write",
        "DELETE": "watchlist:write",
    },
    "api_watchlist_snapshot": {
        "GET": "watchlist:read",
        "HEAD": "watchlist:read",
    },
    "api_watchlist_changes": {
        "GET": "watchlist:read",
        "HEAD": "watchlist:read",
    },
    "api_watched_state": {
        "GET": "watched:read",
        "HEAD": "watched:read",
        "PUT": "watched:write",
        "DELETE": "watched:write",
    },
    "api_watched_state_snapshot": {
        "GET": "watched:read",
        "HEAD": "watched:read",
    },
    "api_watched_state_changes": {
        "GET": "watched:read",
        "HEAD": "watched:read",
    },
}


def required_scope_for_route(url_name: str | None, method: str | None) -> tuple[bool, str | None]:
    """Return whether a route is declared and its required integration scope."""
    if not url_name:
        return False, None

    method_scopes = INTEGRATION_SCOPE_RULES.get(url_name)
    if method_scopes is None:
        return False, None

    normalized_method = (method or "").upper()
    if normalized_method == "OPTIONS":
        return True, None

    scope = method_scopes.get(normalized_method)
    return scope is not None, scope


def normalize_requested_scopes(raw_scopes) -> tuple[list[str] | None, str | None]:
    """Normalize an explicit scope list and return the first unsupported scope."""
    if not isinstance(raw_scopes, list) or not raw_scopes:
        return None, None
    if any(not isinstance(scope, str) for scope in raw_scopes):
        return None, None

    scopes = list(dict.fromkeys(raw_scopes))
    unsupported = sorted(set(scopes) - INTEGRATION_SCOPES)
    return scopes, unsupported[0] if unsupported else None
