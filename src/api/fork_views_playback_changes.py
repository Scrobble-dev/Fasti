"""Ordered playback progress changes for incremental third-party synchronization."""

from http import HTTPStatus as HTTP  # noqa: N814

from django.core import signing
from django.core.signing import BadSignature, SignatureExpired
from drf_spectacular.utils import extend_schema
from rest_framework import views as drf_views
from rest_framework.response import Response

from app.models import PlaybackProgressChange
from app.playback_context import get_playback_source_client_id

_CURSOR_VERSION = 2
_CURSOR_RESOURCE = "playback_progress"
_CURSOR_SALT = "api.playback-progress-changes"
_CURSOR_MAX_AGE_SECONDS = 30 * 24 * 60 * 60
_DEFAULT_LIMIT = 100
_MAX_LIMIT = 100

_CHANGE_SCHEMA = {
    "type": "object",
    "properties": {
        "sequence_id": {"type": "integer"},
        "operation": {"type": "string", "enum": ["upsert", "delete"]},
        "item_id": {"type": "integer"},
        "media_type": {"type": "string"},
        "source": {"type": "string"},
        "media_id": {"type": "string"},
        "season_number": {"type": "integer", "nullable": True},
        "episode_number": {"type": "integer", "nullable": True},
        "ids": {"type": "object"},
        "position_seconds": {"type": "integer", "nullable": True},
        "duration_seconds": {"type": "integer", "nullable": True},
        "completed": {"type": "boolean"},
        "source_client_id": {"type": "string"},
        "created_at": {"type": "string", "format": "date-time"},
    },
}

_RESPONSE_SCHEMA = {
    "type": "object",
    "properties": {
        "version": {"type": "integer", "enum": [_CURSOR_VERSION]},
        "results": {"type": "array", "items": _CHANGE_SCHEMA},
        "next_cursor": {"type": "string"},
        "has_more": {"type": "boolean"},
        "snapshot_required": {"type": "boolean"},
        "client_id": {"type": "string"},
        "cursor_expires_in_seconds": {"type": "integer"},
    },
}


def _cursor_error(code: str, message: str, status: int) -> Response:
    """Return a stable public cursor error."""
    return Response(
        {
            "error": {
                "type": "invalid_request_error",
                "code": code,
                "message": message,
                "param": "cursor",
            },
        },
        status=status,
    )


def _encode_cursor(user_id: int, sequence_id: int, client_id: str) -> str:
    """Sign one user-, resource-, and integration-client-bound progress cursor."""
    return signing.dumps(
        {
            "v": _CURSOR_VERSION,
            "resource": _CURSOR_RESOURCE,
            "user": user_id,
            "client": client_id,
            "sequence": sequence_id,
        },
        salt=_CURSOR_SALT,
        compress=True,
    )


def _decode_cursor(
    cursor: str,
    user_id: int,
    client_id: str,
) -> tuple[int | None, Response | None]:
    """Return the sequence from a cursor bound to this user and integration client."""
    try:
        payload = signing.loads(
            cursor,
            salt=_CURSOR_SALT,
            max_age=_CURSOR_MAX_AGE_SECONDS,
        )
    except SignatureExpired:
        return None, _cursor_error(
            "cursor_expired",
            "This cursor expired. Fetch a fresh progress snapshot before continuing.",
            HTTP.CONFLICT,
        )
    except BadSignature:
        return None, _cursor_error(
            "invalid_cursor",
            "The playback progress cursor is invalid.",
            HTTP.BAD_REQUEST,
        )

    if not isinstance(payload, dict):
        return None, _cursor_error(
            "invalid_cursor",
            "The playback progress cursor is invalid.",
            HTTP.BAD_REQUEST,
        )

    if (
        payload.get("v") != _CURSOR_VERSION
        or payload.get("resource") != _CURSOR_RESOURCE
        or payload.get("user") != user_id
        or payload.get("client") != client_id
    ):
        return None, _cursor_error(
            "invalid_cursor",
            (
                "The playback progress cursor is not valid for this user, "
                "integration client, or resource."
            ),
            HTTP.BAD_REQUEST,
        )

    sequence_id = payload.get("sequence")
    if not isinstance(sequence_id, int) or sequence_id < 0:
        return None, _cursor_error(
            "invalid_cursor",
            "The playback progress cursor contains an invalid sequence.",
            HTTP.BAD_REQUEST,
        )
    return sequence_id, None


def _parse_limit(request) -> tuple[int | None, Response | None]:
    """Return a bounded page limit for the ordered change feed."""
    raw_limit = request.query_params.get("limit", _DEFAULT_LIMIT)
    try:
        limit = int(raw_limit)
    except (TypeError, ValueError):
        limit = 0
    if limit < 1 or limit > _MAX_LIMIT:
        return None, Response(
            {"detail": f"'limit' must be between 1 and {_MAX_LIMIT}."},
            status=HTTP.BAD_REQUEST,
        )
    return limit, None


def _serialize_change(change: PlaybackProgressChange) -> dict:
    """Serialize one ordered progress mutation for external clients."""
    return {
        "sequence_id": change.sequence_id,
        "operation": change.operation,
        "item_id": change.item_id,
        "media_type": change.media_type,
        "source": change.source,
        "media_id": change.media_id,
        "season_number": change.season_number,
        "episode_number": change.episode_number,
        "ids": change.ids,
        "position_seconds": change.position_seconds,
        "duration_seconds": change.duration_seconds,
        "completed": change.completed,
        "source_client_id": change.source_client_id,
        "created_at": change.created_at,
    }


class PlaybackProgressChangesView(drf_views.APIView):
    """Return an initial checkpoint or ordered movie/episode progress changes."""

    @extend_schema(
        description=(
            "Create a snapshot checkpoint when cursor is omitted, or list ordered "
            "movie/episode playback progress changes after a signed client-bound cursor. "
            "Clients must fetch /api/v1/playback/progress after the initial checkpoint, "
            "then apply this feed from the returned cursor. Delete events are explicit. "
            "A cursor can only be resumed by the same authenticated integration client."
        ),
        responses={
            200: _RESPONSE_SCHEMA,
            400: {"type": "object"},
            409: {"type": "object"},
        },
    )
    def get(self, request):
        """Return a checkpoint or one bounded ordered page of progress changes."""
        limit, error = _parse_limit(request)
        if error:
            return error

        client_id = get_playback_source_client_id()
        cursor = request.query_params.get("cursor")
        if not cursor:
            high_water = (
                PlaybackProgressChange.objects.filter(user=request.user)
                .order_by("-sequence_id")
                .values_list("sequence_id", flat=True)
                .first()
                or 0
            )
            return Response(
                {
                    "version": _CURSOR_VERSION,
                    "results": [],
                    "next_cursor": _encode_cursor(
                        request.user.pk,
                        high_water,
                        client_id,
                    ),
                    "has_more": False,
                    "snapshot_required": True,
                    "client_id": client_id,
                    "cursor_expires_in_seconds": _CURSOR_MAX_AGE_SECONDS,
                },
                status=HTTP.OK,
            )

        sequence_id, error = _decode_cursor(
            cursor,
            request.user.pk,
            client_id,
        )
        if error:
            return error

        rows = list(
            PlaybackProgressChange.objects.filter(
                user=request.user,
                sequence_id__gt=sequence_id,
            ).order_by("sequence_id")[: limit + 1]
        )
        has_more = len(rows) > limit
        page = rows[:limit]
        next_sequence = page[-1].sequence_id if page else sequence_id

        return Response(
            {
                "version": _CURSOR_VERSION,
                "results": [_serialize_change(change) for change in page],
                "next_cursor": _encode_cursor(
                    request.user.pk,
                    next_sequence,
                    client_id,
                ),
                "has_more": has_more,
                "snapshot_required": False,
                "client_id": client_id,
                "cursor_expires_in_seconds": _CURSOR_MAX_AGE_SECONDS,
            },
            status=HTTP.OK,
        )
