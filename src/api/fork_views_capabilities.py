"""Public capability discovery for third-party Floppy clients."""

from drf_spectacular.utils import extend_schema
from rest_framework import permissions
from rest_framework import views as drf_views
from rest_framework.response import Response

from integrations.media_identity import EXTERNAL_ID_FIELDS
from integrations.scopes import (
    INTEGRATION_SCOPE_DESCRIPTIONS,
    NUVIO_RECOMMENDED_SCOPES,
)
from integrations.sync_policy import (
    CHANGE_PAGE_MAX,
    CHANGE_RETENTION_DAYS,
    CURSOR_MAX_AGE_SECONDS,
    CURSOR_VERSION,
    SNAPSHOT_CURSOR_VERSION,
    SNAPSHOT_PAGE_MAX,
)

_CONTRACT_VERSION = "1"


class IntegrationCapabilitiesView(drf_views.APIView):
    """Describe supported integration behavior without exposing user data or secrets."""

    authentication_classes = []
    permission_classes = [permissions.AllowAny]

    @extend_schema(
        description=(
            "Return the versioned third-party integration contract. This endpoint is "
            "public and contains capability metadata only."
        ),
        responses={200: {"type": "object"}},
    )
    def get(self, request):
        """Return the current experimental client-integration capability descriptor."""
        return Response(
            {
                "product": "Floppy",
                "contract_version": _CONTRACT_VERSION,
                "stability": "experimental",
                "authentication": {
                    "methods": ["bearer", "x-api-key"],
                    "scoped_credentials": True,
                    "legacy_account_token_compatible": True,
                    "available_scopes": INTEGRATION_SCOPE_DESCRIPTIONS,
                    "recommended_nuvio_scopes": list(NUVIO_RECOMMENDED_SCOPES),
                },
                "identity": {
                    "namespaces": list(EXTERNAL_ID_FIELDS),
                    "authoritative_title_matching": False,
                    "all_supplied_ids_must_agree": True,
                },
                "resources": {
                    "scrobble": {
                        "write": {
                            "path": "/api/v1/scrobble/",
                            "method": "POST",
                            "scope": "scrobble:write",
                        },
                    },
                    "progress": {
                        "snapshot": {
                            "path": "/api/v1/playback/progress/snapshot/",
                            "method": "GET",
                            "scope": "progress:read",
                            "cursor_version": SNAPSHOT_CURSOR_VERSION,
                            "client_bound_cursor": True,
                            "cursor_ttl_seconds": CURSOR_MAX_AGE_SECONDS,
                            "page_max": SNAPSHOT_PAGE_MAX,
                            "pagination": "stable_keyset",
                            "authoritative_when_complete": True,
                            "media_types": ["movie", "episode"],
                        },
                        "compatibility_snapshot": {
                            "path": "/api/v1/playback/progress/",
                            "method": "GET",
                            "scope": "progress:read",
                            "pagination": "offset",
                            "media_types": ["movie", "episode", "podcast"],
                        },
                        "write": {
                            "path": "/api/v1/playback/progress/",
                            "method": "PUT",
                            "scope": "progress:write",
                        },
                        "delete": {
                            "path": "/api/v1/playback/progress/",
                            "method": "DELETE",
                            "scope": "progress:write",
                        },
                        "changes": {
                            "path": "/api/v1/playback/progress/changes/",
                            "method": "GET",
                            "scope": "progress:read",
                            "cursor_version": CURSOR_VERSION,
                            "client_bound_cursor": True,
                            "cursor_ttl_seconds": CURSOR_MAX_AGE_SECONDS,
                            "change_retention_days": CHANGE_RETENTION_DAYS,
                            "page_max": CHANGE_PAGE_MAX,
                            "media_types": ["movie", "episode"],
                        },
                        "media_types": ["movie", "episode"],
                        "snapshot_only_media_types": ["podcast"],
                    },
                    "saved_media": {
                        "snapshot": {
                            "path": "/api/v1/watchlist/snapshot/",
                            "method": "GET",
                            "scope": "watchlist:read",
                            "cursor_version": SNAPSHOT_CURSOR_VERSION,
                            "client_bound_cursor": True,
                            "cursor_ttl_seconds": CURSOR_MAX_AGE_SECONDS,
                            "page_max": SNAPSHOT_PAGE_MAX,
                            "pagination": "stable_keyset",
                            "authoritative_when_complete": True,
                            "media_types": ["movie", "tv", "anime"],
                        },
                        "compatibility_snapshot": {
                            "path": "/api/v1/watchlist/",
                            "method": "GET",
                            "scope": "watchlist:read",
                            "pagination": "offset",
                            "media_types": ["movie", "tv", "anime"],
                        },
                        "add": {
                            "path": "/api/v1/watchlist/",
                            "method": "PUT",
                            "scope": "watchlist:write",
                        },
                        "remove": {
                            "path": "/api/v1/watchlist/",
                            "method": "DELETE",
                            "scope": "watchlist:write",
                        },
                        "changes": {
                            "path": "/api/v1/watchlist/changes/",
                            "method": "GET",
                            "scope": "watchlist:read",
                            "cursor_version": CURSOR_VERSION,
                            "client_bound_cursor": True,
                            "cursor_ttl_seconds": CURSOR_MAX_AGE_SECONDS,
                            "change_retention_days": CHANGE_RETENTION_DAYS,
                            "page_max": CHANGE_PAGE_MAX,
                        },
                        "media_types": ["movie", "tv", "anime"],
                    },
                    "watched_state": {
                        "snapshot": {
                            "path": "/api/v1/watched/snapshot/",
                            "method": "GET",
                            "scope": "watched:read",
                            "cursor_version": SNAPSHOT_CURSOR_VERSION,
                            "client_bound_cursor": True,
                            "cursor_ttl_seconds": CURSOR_MAX_AGE_SECONDS,
                            "page_max": SNAPSHOT_PAGE_MAX,
                            "pagination": "stable_keyset",
                            "authoritative_when_complete": True,
                            "media_types": ["movie", "episode"],
                        },
                        "compatibility_snapshot": {
                            "path": "/api/v1/watched/",
                            "method": "GET",
                            "scope": "watched:read",
                            "pagination": "offset",
                            "media_type_required": True,
                        },
                        "write": {
                            "path": "/api/v1/watched/",
                            "method": "PUT",
                            "scope": "watched:write",
                            "mutates_viewing_history": False,
                        },
                        "clear": {
                            "path": "/api/v1/watched/",
                            "method": "DELETE",
                            "scope": "watched:write",
                            "semantic": "set_current_state_unwatched",
                            "mutates_viewing_history": False,
                        },
                        "changes": {
                            "path": "/api/v1/watched/changes/",
                            "method": "GET",
                            "scope": "watched:read",
                            "cursor_version": CURSOR_VERSION,
                            "client_bound_cursor": True,
                            "cursor_ttl_seconds": CURSOR_MAX_AGE_SECONDS,
                            "change_retention_days": CHANGE_RETENTION_DAYS,
                            "page_max": CHANGE_PAGE_MAX,
                            "explicit_unwatched_events": True,
                        },
                        "occurrence_write": {
                            "via": "scrobble",
                            "path": "/api/v1/scrobble/",
                            "scope": "scrobble:write",
                        },
                        "media_types": ["movie", "episode"],
                        "watched_at_nullable": True,
                    },
                },
                "sync": {
                    "checkpoint_then_snapshot_then_delta": True,
                    "stable_snapshot_cursor_version": SNAPSHOT_CURSOR_VERSION,
                    "change_cursor_version": CURSOR_VERSION,
                    "cursor_bound_to_authenticated_client": True,
                    "cursor_ttl_seconds": CURSOR_MAX_AGE_SECONDS,
                    "change_retention_days": CHANGE_RETENTION_DAYS,
                    "stable_snapshot_pagination": "keyset_with_fixed_upper_boundary",
                    "delete_requires_explicit_event": True,
                    "cache_miss_is_delete": False,
                    "offline_reconnect_requires_snapshot_on_expired_cursor": True,
                    "viewing_history_is_separate_from_current_watched_state": True,
                },
            }
        )
