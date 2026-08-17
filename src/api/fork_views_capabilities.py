"""Public capability discovery for third-party Floppy clients."""

from django.conf import settings
from drf_spectacular.utils import extend_schema
from rest_framework import permissions
from rest_framework import views as drf_views
from rest_framework.response import Response

from integrations.media_identity import EXTERNAL_ID_FIELDS

_CONTRACT_VERSION = "1"
_CURSOR_TTL_SECONDS = 30 * 24 * 60 * 60
_CHANGE_PAGE_MAX = 100


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
                "server_version": settings.VERSION,
                "contract_version": _CONTRACT_VERSION,
                "stability": "experimental",
                "authentication": {
                    "methods": ["bearer", "x-api-key"],
                    "scoped_credentials": True,
                    "legacy_account_token_compatible": True,
                },
                "identity": {
                    "namespaces": list(EXTERNAL_ID_FIELDS),
                    "authoritative_title_matching": False,
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
                            "path": "/api/v1/playback/progress/",
                            "method": "GET",
                            "scope": "progress:read",
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
                            "cursor_ttl_seconds": _CURSOR_TTL_SECONDS,
                            "page_max": _CHANGE_PAGE_MAX,
                        },
                        "media_types": ["movie", "episode", "podcast"],
                    },
                    "saved_media": {
                        "snapshot": {
                            "path": "/api/v1/watchlist/",
                            "method": "GET",
                            "scope": "watchlist:read",
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
                            "cursor_ttl_seconds": _CURSOR_TTL_SECONDS,
                            "page_max": _CHANGE_PAGE_MAX,
                        },
                        "media_types": ["movie", "tv", "anime"],
                    },
                    "watched_state": {
                        "snapshot": {
                            "path": "/api/v1/watched/",
                            "method": "GET",
                            "scope": "watched:read",
                            "media_type_required": True,
                        },
                        "completion_write": {
                            "via": "scrobble",
                            "path": "/api/v1/scrobble/",
                            "scope": "scrobble:write",
                        },
                        "delete": None,
                        "delete_reason": (
                            "A generic watched-state delete is not exposed because Floppy "
                            "preserves viewing occurrences and does not delete unrelated history."
                        ),
                        "media_types": ["movie", "episode"],
                        "watched_at_nullable": True,
                    },
                },
                "sync": {
                    "checkpoint_then_snapshot_then_delta": True,
                    "delete_requires_explicit_event": True,
                    "cache_miss_is_delete": False,
                },
            }
        )
