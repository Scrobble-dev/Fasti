"""Ordered exact watched-state changes for third-party synchronization."""

from http import HTTPStatus as HTTP  # noqa: N814

from drf_spectacular.utils import OpenApiParameter, extend_schema
from rest_framework import views as drf_views
from rest_framework.response import Response

from app.models import WatchedStateChange
from app.playback_context import get_playback_source_client_id
from integrations.change_cursor import (
    ChangeCursorError,
    ChangeCursorExpired,
    decode_change_cursor,
    encode_change_cursor,
)
from integrations.sync_policy import (
    CHANGE_PAGE_MAX,
    CURSOR_MAX_AGE_SECONDS,
    CURSOR_VERSION,
)

_CURSOR_RESOURCE = "watched_state"
_DEFAULT_LIMIT = CHANGE_PAGE_MAX

_CHANGE_SCHEMA = {
    "type": "object",
    "properties": {
        "sequence_id": {"type": "integer"},
        "watched": {"type": "boolean"},
        "item_id": {"type": "integer"},
        "media_type": {"type": "string", "enum": ["movie", "episode"]},
        "source": {"type": "string"},
        "media_id": {"type": "string"},
        "ids": {"type": "object"},
        "season_number": {"type": "integer", "nullable": True},
        "episode_number": {"type": "integer", "nullable": True},
        "watched_at": {"type": "string", "format": "date-time", "nullable": True},
        "source_client_id": {"type": "string"},
        "created_at": {"type": "string", "format": "date-time"},
    },
}

_RESPONSE_SCHEMA = {
    "type": "object",
    "properties": {
        "version": {"type": "integer", "enum": [CURSOR_VERSION]},
        "results": {"type": "array", "items": _CHANGE_SCHEMA},
        "next_cursor": {"type": "string"},
        "has_more": {"type": "boolean"},
        "snapshot_required": {"type": "boolean"},
        "client_id": {"type": "string"},
        "cursor_expires_in_seconds": {"type": "integer"},
    },
}


def _error(code: str, message: str, status: int, *, param: str = "cursor") -> Response:
    """Return one stable public watched-change error."""
    return Response(
        {
            "error": {
                "type": "invalid_request_error",
                "code": code,
                "message": message,
                "param": param,
            },
        },
        status=status,
    )


def _parse_limit(request):
    """Return a bounded watched-state change page size."""
    raw_limit = request.query_params.get("limit", _DEFAULT_LIMIT)
    try:
        limit = int(raw_limit)
    except (TypeError, ValueError):
        limit = 0
    if limit < 1 or limit > CHANGE_PAGE_MAX:
        return None, _error(
            "invalid_limit",
            f"limit must be between 1 and {CHANGE_PAGE_MAX}.",
            HTTP.BAD_REQUEST,
            param="limit",
        )
    return limit, None


def _serialize_change(change: WatchedStateChange) -> dict:
    """Serialize one exact current-state mutation."""
    return {
        "sequence_id": change.sequence_id,
        "watched": change.watched,
        "item_id": change.item_id,
        "media_type": change.media_type,
        "source": change.source,
        "media_id": change.media_id,
        "ids": change.ids,
        "season_number": change.season_number,
        "episode_number": change.episode_number,
        "watched_at": change.watched_at if change.watched else None,
        "source_client_id": change.source_client_id,
        "created_at": change.created_at,
    }


class WatchedStateChangesView(drf_views.APIView):
    """Return a checkpoint or ordered exact watched-state changes."""

    @extend_schema(
        description=(
            "Create a watched-state snapshot checkpoint when cursor is omitted, or list "
            "ordered exact movie/episode watched-state changes after a client-bound cursor. "
            "After the checkpoint, fetch /api/v1/watched/snapshot/, then apply this feed "
            "from the returned cursor. Both watched=true and watched=false are explicit "
            "events. Viewing-history occurrences are not changed by this feed."
        ),
        parameters=[
            OpenApiParameter(
                name="cursor",
                type=str,
                location=OpenApiParameter.QUERY,
                required=False,
                description="Opaque cursor returned by the previous response.",
            ),
            OpenApiParameter(
                name="limit",
                type=int,
                location=OpenApiParameter.QUERY,
                required=False,
                description=f"Page size from 1 to {CHANGE_PAGE_MAX}.",
            ),
        ],
        responses={
            200: _RESPONSE_SCHEMA,
            400: {"type": "object"},
            409: {"type": "object"},
        },
    )
    def get(self, request):
        """Return an initial checkpoint or one bounded ordered delta page."""
        limit, error = _parse_limit(request)
        if error:
            return error

        client_id = get_playback_source_client_id()
        cursor = request.query_params.get("cursor")
        if not cursor:
            high_water = (
                WatchedStateChange.objects.filter(user=request.user)
                .order_by("-sequence_id")
                .values_list("sequence_id", flat=True)
                .first()
                or 0
            )
            return Response(
                {
                    "version": CURSOR_VERSION,
                    "results": [],
                    "next_cursor": encode_change_cursor(
                        resource=_CURSOR_RESOURCE,
                        user_id=request.user.pk,
                        client_id=client_id,
                        sequence_id=high_water,
                    ),
                    "has_more": False,
                    "snapshot_required": True,
                    "client_id": client_id,
                    "cursor_expires_in_seconds": CURSOR_MAX_AGE_SECONDS,
                },
                status=HTTP.OK,
            )

        try:
            cursor_state = decode_change_cursor(
                cursor,
                resource=_CURSOR_RESOURCE,
                user_id=request.user.pk,
                client_id=client_id,
            )
        except ChangeCursorExpired:
            return _error(
                "cursor_expired",
                "This cursor expired. Start a fresh watched-state snapshot before continuing.",
                HTTP.CONFLICT,
            )
        except ChangeCursorError:
            return _error(
                "invalid_cursor",
                "The watched-state cursor is invalid for this user, integration client, or resource.",
                HTTP.BAD_REQUEST,
            )

        rows = list(
            WatchedStateChange.objects.filter(
                user=request.user,
                sequence_id__gt=cursor_state.sequence_id,
            ).order_by("sequence_id")[: limit + 1]
        )
        has_more = len(rows) > limit
        page = rows[:limit]
        next_sequence = (
            page[-1].sequence_id if page else cursor_state.sequence_id
        )

        return Response(
            {
                "version": CURSOR_VERSION,
                "results": [_serialize_change(change) for change in page],
                "next_cursor": encode_change_cursor(
                    resource=_CURSOR_RESOURCE,
                    user_id=request.user.pk,
                    client_id=client_id,
                    sequence_id=next_sequence,
                ),
                "has_more": has_more,
                "snapshot_required": False,
                "client_id": client_id,
                "cursor_expires_in_seconds": CURSOR_MAX_AGE_SECONDS,
            },
            status=HTTP.OK,
        )
