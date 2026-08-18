"""Stable current-state snapshots for third-party synchronization clients."""

from http import HTTPStatus as HTTP  # noqa: N814

from drf_spectacular.utils import OpenApiParameter, extend_schema
from rest_framework import views as drf_views
from rest_framework.response import Response

from app.models import MediaTypes, PlaybackProgress, SavedMediaMembership
from app.playback_context import get_playback_source_client_id
from integrations.snapshot_cursor import (
    SnapshotCursorError,
    SnapshotCursorExpired,
    decode_snapshot_cursor,
    encode_snapshot_cursor,
)
from integrations.sync_policy import (
    CURSOR_MAX_AGE_SECONDS,
    SNAPSHOT_CURSOR_VERSION,
    SNAPSHOT_PAGE_MAX,
)

from .fork_views_playback import _serialize_progress, _show_item_map
from .fork_views_saved_media import _serialize_membership

_PROGRESS_RESOURCE = "playback_progress_snapshot"
_SAVED_MEDIA_RESOURCE = "saved_media_snapshot"
_PROGRESS_MEDIA_TYPES = (MediaTypes.MOVIE.value, MediaTypes.EPISODE.value)
_SAVED_MEDIA_TYPES = (
    MediaTypes.MOVIE.value,
    MediaTypes.TV.value,
    MediaTypes.ANIME.value,
)
_DEFAULT_LIMIT = SNAPSHOT_PAGE_MAX

_RESPONSE_SCHEMA = {
    "type": "object",
    "properties": {
        "version": {"type": "integer", "enum": [SNAPSHOT_CURSOR_VERSION]},
        "results": {"type": "array", "items": {"type": "object"}},
        "next_cursor": {"type": "string", "nullable": True},
        "has_more": {"type": "boolean"},
        "snapshot_complete": {"type": "boolean"},
        "client_id": {"type": "string"},
        "media_types": {"type": "array", "items": {"type": "string"}},
        "cursor_expires_in_seconds": {"type": "integer"},
    },
}


def _error(code: str, message: str, status: int, *, param: str | None = None):
    """Return a stable public snapshot error."""
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
    """Return one bounded stable-snapshot page size."""
    raw_limit = request.query_params.get("limit", _DEFAULT_LIMIT)
    try:
        limit = int(raw_limit)
    except (TypeError, ValueError):
        limit = 0
    if limit < 1 or limit > SNAPSHOT_PAGE_MAX:
        return None, _error(
            "invalid_limit",
            f"limit must be between 1 and {SNAPSHOT_PAGE_MAX}.",
            HTTP.BAD_REQUEST,
            param="limit",
        )
    return limit, None


def _parse_media_types(request, supported):
    """Return a normalized media-type filter for the first snapshot page."""
    raw_value = request.query_params.get("media_type", "")
    if not raw_value:
        return list(supported), None

    requested = sorted(
        {
            part.strip().lower()
            for part in raw_value.split(",")
            if part.strip()
        }
    )
    invalid = [media_type for media_type in requested if media_type not in supported]
    if not requested or invalid:
        return None, _error(
            "unsupported_media_type",
            f"media_type must contain only: {', '.join(supported)}.",
            HTTP.BAD_REQUEST,
            param="media_type",
        )
    return requested, None


def _cursor_state(request, *, resource: str, supported):
    """Return the immutable snapshot state for this authenticated client."""
    client_id = get_playback_source_client_id()
    cursor = request.query_params.get("cursor")
    if not cursor:
        media_types, error = _parse_media_types(request, supported)
        if error:
            return None, None, None, error
        return client_id, None, media_types, None

    try:
        state = decode_snapshot_cursor(
            cursor,
            resource=resource,
            user_id=request.user.pk,
            client_id=client_id,
        )
    except SnapshotCursorExpired:
        return None, None, None, _error(
            "snapshot_cursor_expired",
            "This snapshot cursor expired. Start a fresh snapshot before continuing.",
            HTTP.CONFLICT,
            param="cursor",
        )
    except SnapshotCursorError:
        return None, None, None, _error(
            "invalid_snapshot_cursor",
            "The snapshot cursor is invalid for this user, integration client, or resource.",
            HTTP.BAD_REQUEST,
            param="cursor",
        )

    media_types = state.filters.get("media_types")
    if (
        not isinstance(media_types, list)
        or not media_types
        or any(media_type not in supported for media_type in media_types)
        or media_types != sorted(set(media_types))
    ):
        return None, None, None, _error(
            "invalid_snapshot_cursor",
            "The snapshot cursor contains invalid filters.",
            HTTP.BAD_REQUEST,
            param="cursor",
        )

    if request.query_params.get("media_type"):
        requested, error = _parse_media_types(request, supported)
        if error:
            return None, None, None, error
        if requested != media_types:
            return None, None, None, _error(
                "snapshot_filter_conflict",
                "media_type cannot change while a snapshot is in progress.",
                HTTP.BAD_REQUEST,
                param="media_type",
            )

    return client_id, state, media_types, None


def _snapshot_response(
    *,
    request,
    resource: str,
    client_id: str,
    media_types,
    upper_id: int,
    rows,
    limit: int,
    serialize,
):
    """Return one keyset page and a client-bound continuation cursor."""
    has_more = len(rows) > limit
    page = rows[:limit]
    next_cursor = None
    if has_more and page:
        next_cursor = encode_snapshot_cursor(
            resource=resource,
            user_id=request.user.pk,
            client_id=client_id,
            upper_id=upper_id,
            after_id=page[-1].pk,
            filters={"media_types": media_types},
        )

    return Response(
        {
            "version": SNAPSHOT_CURSOR_VERSION,
            "results": serialize(page),
            "next_cursor": next_cursor,
            "has_more": has_more,
            "snapshot_complete": not has_more,
            "client_id": client_id,
            "media_types": media_types,
            "cursor_expires_in_seconds": CURSOR_MAX_AGE_SECONDS,
        },
        status=HTTP.OK,
    )


class PlaybackProgressSnapshotView(drf_views.APIView):
    """Return a stable movie/episode progress snapshot using bounded keyset pages."""

    @extend_schema(
        description=(
            "Return a stable current-state snapshot for movie and episode resume positions. "
            "Call without cursor for the first page, then follow next_cursor until "
            "snapshot_complete is true. The first page fixes an upper row boundary, so "
            "concurrent inserts and deletes cannot shift later pages. Use the change-feed "
            "checkpoint before this snapshot and apply the delta afterward."
        ),
        parameters=[
            OpenApiParameter(
                name="cursor",
                type=str,
                location=OpenApiParameter.QUERY,
                required=False,
                description="Opaque client-bound continuation cursor from the prior page.",
            ),
            OpenApiParameter(
                name="limit",
                type=int,
                location=OpenApiParameter.QUERY,
                required=False,
                description=f"Page size from 1 to {SNAPSHOT_PAGE_MAX}.",
            ),
            OpenApiParameter(
                name="media_type",
                type=str,
                location=OpenApiParameter.QUERY,
                required=False,
                description="Optional comma-separated subset: movie,episode.",
            ),
        ],
        responses={200: _RESPONSE_SCHEMA, 400: {"type": "object"}, 409: {"type": "object"}},
    )
    def get(self, request):
        """Return one stable page of current movie/episode progress."""
        limit, error = _parse_limit(request)
        if error:
            return error
        client_id, state, media_types, error = _cursor_state(
            request,
            resource=_PROGRESS_RESOURCE,
            supported=_PROGRESS_MEDIA_TYPES,
        )
        if error:
            return error

        queryset = PlaybackProgress.objects.filter(
            user=request.user,
            item__media_type__in=media_types,
        )
        if state is None:
            upper_id = (
                queryset.order_by("-pk").values_list("pk", flat=True).first() or 0
            )
            after_id = 0
        else:
            upper_id = state.upper_id
            after_id = state.after_id

        rows = list(
            queryset.filter(pk__gt=after_id, pk__lte=upper_id)
            .select_related("item")
            .order_by("pk")[: limit + 1]
        )

        def _serialize(rows_to_serialize):
            show_map = _show_item_map([row.item for row in rows_to_serialize])
            return [
                _serialize_progress(row, show_map) for row in rows_to_serialize
            ]

        return _snapshot_response(
            request=request,
            resource=_PROGRESS_RESOURCE,
            client_id=client_id,
            media_types=media_types,
            upper_id=upper_id,
            rows=rows,
            limit=limit,
            serialize=_serialize,
        )


class SavedMediaSnapshotView(drf_views.APIView):
    """Return a stable saved-library snapshot using bounded keyset pages."""

    @extend_schema(
        description=(
            "Return a stable current saved-library snapshot. Call without cursor for the "
            "first page, then follow next_cursor until snapshot_complete is true. The "
            "first page fixes an upper membership boundary, so concurrent inserts and "
            "deletes cannot shift later pages. Use the change-feed checkpoint before this "
            "snapshot and apply the delta afterward."
        ),
        parameters=[
            OpenApiParameter(
                name="cursor",
                type=str,
                location=OpenApiParameter.QUERY,
                required=False,
                description="Opaque client-bound continuation cursor from the prior page.",
            ),
            OpenApiParameter(
                name="limit",
                type=int,
                location=OpenApiParameter.QUERY,
                required=False,
                description=f"Page size from 1 to {SNAPSHOT_PAGE_MAX}.",
            ),
            OpenApiParameter(
                name="media_type",
                type=str,
                location=OpenApiParameter.QUERY,
                required=False,
                description="Optional comma-separated subset: movie,tv,anime.",
            ),
        ],
        responses={200: _RESPONSE_SCHEMA, 400: {"type": "object"}, 409: {"type": "object"}},
    )
    def get(self, request):
        """Return one stable page of explicit saved-library membership."""
        limit, error = _parse_limit(request)
        if error:
            return error
        client_id, state, media_types, error = _cursor_state(
            request,
            resource=_SAVED_MEDIA_RESOURCE,
            supported=_SAVED_MEDIA_TYPES,
        )
        if error:
            return error

        queryset = SavedMediaMembership.objects.filter(
            user=request.user,
            item__media_type__in=media_types,
        )
        if state is None:
            upper_id = (
                queryset.order_by("-pk").values_list("pk", flat=True).first() or 0
            )
            after_id = 0
        else:
            upper_id = state.upper_id
            after_id = state.after_id

        rows = list(
            queryset.filter(pk__gt=after_id, pk__lte=upper_id)
            .select_related("item")
            .order_by("pk")[: limit + 1]
        )
        return _snapshot_response(
            request=request,
            resource=_SAVED_MEDIA_RESOURCE,
            client_id=client_id,
            media_types=media_types,
            upper_id=upper_id,
            rows=rows,
            limit=limit,
            serialize=lambda page: [_serialize_membership(row) for row in page],
        )
