"""Exact current watched state for third-party integration clients."""

import logging
from http import HTTPStatus as HTTP  # noqa: N814

from drf_spectacular.utils import OpenApiParameter, extend_schema
from rest_framework import views as drf_views
from rest_framework.response import Response

from app.models import MediaTypes, WatchedState
from app.models.watched_state import set_watched_state, watched_identity
from integrations.delivery import get_or_record_receipt
from integrations.models import IntegrationToken

from .fork_views_playback import (
    _EXTERNAL_ID_KEYS,
    _coerce_episode_coordinate,
    _normalize_identifiers,
    resolve_video_item,
)
from .helpers import make_page_url, parse_limit_offset, try_parse_datetime_input

logger = logging.getLogger(__name__)

_SUPPORTED_WATCHED_TYPES = (MediaTypes.MOVIE.value, MediaTypes.EPISODE.value)

_WATCHED_ROW_SCHEMA = {
    "type": "object",
    "properties": {
        "media_type": {"type": "string", "enum": list(_SUPPORTED_WATCHED_TYPES)},
        "library_media_type": {"type": "string", "nullable": True},
        "source": {"type": "string"},
        "media_id": {"type": "string"},
        "ids": {"type": "object"},
        "season_number": {"type": "integer", "nullable": True},
        "episode_number": {"type": "integer", "nullable": True},
        "watched_at": {"type": "string", "format": "date-time", "nullable": True},
    },
}
_WRITE_RESPONSE_SCHEMA = {
    **_WATCHED_ROW_SCHEMA,
    "properties": {
        **_WATCHED_ROW_SCHEMA["properties"],
        "watched": {"type": "boolean"},
        "updated_at": {"type": "string", "format": "date-time"},
    },
}
_WRITE_SCHEMA = {
    "type": "object",
    "required": ["media_type", "ids", "watched"],
    "properties": {
        "media_type": {"type": "string", "enum": list(_SUPPORTED_WATCHED_TYPES)},
        "ids": {
            "type": "object",
            "properties": {
                "tmdb": {"type": "string", "nullable": True},
                "imdb": {"type": "string", "nullable": True},
                "tvdb": {"type": "string", "nullable": True},
            },
        },
        "season_number": {
            "type": "integer",
            "minimum": 0,
            "nullable": True,
            "description": "Required for media_type episode.",
        },
        "episode_number": {
            "type": "integer",
            "minimum": 1,
            "nullable": True,
            "description": "Required for media_type episode.",
        },
        "watched": {"type": "boolean"},
        "watched_at": {
            "type": "string",
            "format": "date-time",
            "nullable": True,
            "description": "Optional trusted occurrence time. Null means unknown.",
        },
        "client_event_id": {
            "type": "string",
            "nullable": True,
            "description": "Optional retry-safe event identifier.",
        },
    },
}
_DELETE_SCHEMA = {
    "type": "object",
    "required": ["media_type", "ids"],
    "properties": {
        key: value
        for key, value in _WRITE_SCHEMA["properties"].items()
        if key not in {"watched", "watched_at"}
    },
}


def _error(code: str, message: str, *, param: str, status=HTTP.BAD_REQUEST):
    """Return a stable watched-state request error."""
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


def _pagination(request, total: int, limit: int, offset: int) -> dict:
    """Build the standard Floppy limit/offset envelope without loading all rows."""
    next_url = (
        make_page_url(request, limit, offset + limit)
        if offset + limit < total
        else None
    )
    previous_url = (
        make_page_url(request, limit, max(0, offset - limit)) if offset > 0 else None
    )
    return {
        "total": total,
        "limit": limit,
        "offset": offset,
        "next": next_url,
        "previous": previous_url,
    }


def serialize_watched_state(state: WatchedState, *, include_state=False) -> dict:
    """Return one portable exact current-state row."""
    item = state.item
    payload = {
        "media_type": item.media_type,
        "library_media_type": item.library_media_type or None,
        "source": item.source,
        "media_id": str(item.media_id),
        "ids": watched_identity(item),
        "season_number": item.season_number,
        "episode_number": item.episode_number,
        "watched_at": state.watched_at if state.watched else None,
    }
    if include_state:
        payload["watched"] = state.watched
        payload["updated_at"] = state.updated_at
    return payload


def _parse_identity(data, *, require_watched):
    """Validate one exact watched-state write without resolving provider metadata."""
    if not isinstance(data, dict):
        return None, _error(
            "invalid_request_body",
            "Request body must be a JSON object.",
            param="body",
        )

    media_type = data.get("media_type")
    if media_type not in _SUPPORTED_WATCHED_TYPES:
        return None, _error(
            "unsupported_media_type",
            f"media_type must be one of: {', '.join(_SUPPORTED_WATCHED_TYPES)}.",
            param="media_type",
        )

    ids = _normalize_identifiers(data.get("ids"), _EXTERNAL_ID_KEYS)
    if ids is None or not ids:
        return None, _error(
            "invalid_ids",
            "ids must contain at least one valid tmdb, imdb, or tvdb identifier.",
            param="ids",
        )

    season_number = data.get("season_number")
    episode_number = data.get("episode_number")
    if media_type == MediaTypes.EPISODE.value:
        if season_number is None or episode_number is None:
            return None, _error(
                "missing_episode_coordinates",
                "season_number and episode_number are required for an episode.",
                param="season_number",
            )
        try:
            season_number = _coerce_episode_coordinate(
                season_number,
                allow_zero=True,
            )
            episode_number = _coerce_episode_coordinate(
                episode_number,
                allow_zero=False,
            )
        except (TypeError, ValueError):
            return None, _error(
                "invalid_episode_coordinates",
                "season_number must be at least 0 and episode_number at least 1.",
                param="season_number",
            )
    else:
        season_number = None
        episode_number = None

    watched = data.get("watched")
    if require_watched and not isinstance(watched, bool):
        return None, _error(
            "invalid_watched_state",
            "watched must be true or false.",
            param="watched",
        )

    watched_at = None
    if "watched_at" in data and data.get("watched_at") is not None:
        if watched is False:
            return None, _error(
                "invalid_watched_at",
                "watched_at must be null when watched is false.",
                param="watched_at",
            )
        try:
            watched_at = try_parse_datetime_input(data.get("watched_at"))
        except (TypeError, ValueError):
            return None, _error(
                "invalid_watched_at",
                "watched_at must be a valid date-time.",
                param="watched_at",
            )

    return {
        "media_type": media_type,
        "ids": ids,
        "season_number": season_number,
        "episode_number": episode_number,
        "watched": watched,
        "watched_at": watched_at,
    }, None


def _resolve_item(request, parsed):
    """Resolve or hydrate one exact movie/episode Item without creating history."""
    try:
        return resolve_video_item(
            request.user,
            parsed["media_type"],
            parsed["ids"],
            parsed["season_number"],
            parsed["episode_number"],
            create=True,
        )
    except Exception:
        logger.warning("Could not resolve watched-state media", exc_info=True)
        return None


def _with_idempotency(request, execute_fn):
    """Execute a watched-state mutation through the durable delivery receipt gateway."""
    client_event_id = request.headers.get("Idempotency-Key")
    if not client_event_id and isinstance(request.data, dict):
        client_event_id = request.data.get("client_event_id")
    if not client_event_id:
        return execute_fn()

    token = request.auth if isinstance(request.auth, IntegrationToken) else None
    receipt_payload = {
        "resource": "watched_state",
        "method": request.method.upper(),
        "body": request.data,
    }
    response, _ = get_or_record_receipt(
        user=request.user,
        client_event_id=str(client_event_id),
        payload=receipt_payload,
        execute_fn=execute_fn,
        token=token,
    )
    return response


class WatchedStateView(drf_views.APIView):
    """Read or mutate exact current watched state without rewriting history."""

    @extend_schema(
        description=(
            "List current watched state for movies or exact episodes. Repeated plays stay "
            "in Floppy history and collapse to one current-state row here. A null watched_at "
            "means Floppy knows the item is watched but has no trustworthy watch timestamp."
        ),
        parameters=[
            OpenApiParameter(
                name="media_type",
                type=str,
                location=OpenApiParameter.QUERY,
                required=True,
                enum=list(_SUPPORTED_WATCHED_TYPES),
                description="Return movie or exact episode watched state.",
            ),
            OpenApiParameter(
                name="limit",
                type=int,
                location=OpenApiParameter.QUERY,
                required=False,
            ),
            OpenApiParameter(
                name="offset",
                type=int,
                location=OpenApiParameter.QUERY,
                required=False,
            ),
        ],
        responses={200: {"type": "object"}, 400: {"type": "object"}},
    )
    def get(self, request):
        """Return one bounded compatibility snapshot of watched=true current state."""
        media_type = str(request.query_params.get("media_type", "")).strip().lower()
        if media_type not in _SUPPORTED_WATCHED_TYPES:
            return _error(
                "unsupported_media_type",
                f"media_type must be one of: {', '.join(_SUPPORTED_WATCHED_TYPES)}.",
                param="media_type",
            )

        limit, offset, error = parse_limit_offset(request)
        if error:
            return error

        queryset = (
            WatchedState.objects.filter(
                user=request.user,
                watched=True,
                item__media_type=media_type,
            )
            .select_related("item")
            .order_by("-updated_at", "-id")
        )
        total = queryset.count()
        results = [
            serialize_watched_state(state)
            for state in queryset[offset : offset + limit]
        ]
        return Response(
            {
                "pagination": _pagination(request, total, limit, offset),
                "results": results,
            },
            status=HTTP.OK,
        )

    @extend_schema(
        description=(
            "Set exact current watched state for a movie or episode. This changes only "
            "the synchronization state. It does not create, delete, or rewrite MoviePlay "
            "or Episode viewing-history occurrences. Use scrobble for a viewing occurrence."
        ),
        request={"application/json": _WRITE_SCHEMA},
        responses={200: _WRITE_RESPONSE_SCHEMA, 400: {"type": "object"}, 404: {"type": "object"}},
    )
    def put(self, request):
        """Set watched=true or watched=false without mutating viewing history."""

        def _execute():
            parsed, error = _parse_identity(request.data, require_watched=True)
            if error:
                return error
            item = _resolve_item(request, parsed)
            if item is None:
                return _error(
                    "unresolved_media",
                    "No exact supported media identity could be resolved.",
                    param="ids",
                    status=HTTP.NOT_FOUND,
                )

            state, _changed = set_watched_state(
                user_id=request.user.pk,
                item=item,
                watched=parsed["watched"],
                watched_at=parsed["watched_at"],
            )
            state.item = item
            return Response(
                serialize_watched_state(state, include_state=True),
                status=HTTP.OK,
            )

        return _with_idempotency(request, _execute)

    @extend_schema(
        description=(
            "Clear exact current watched state for a movie or episode. This is equivalent "
            "to watched=false. Viewing history is preserved and no sibling episode state "
            "is inferred or changed."
        ),
        request={"application/json": _DELETE_SCHEMA},
        responses={200: _WRITE_RESPONSE_SCHEMA, 400: {"type": "object"}, 404: {"type": "object"}},
    )
    def delete(self, request):
        """Set watched=false while preserving every viewing-history occurrence."""

        def _execute():
            parsed, error = _parse_identity(request.data, require_watched=False)
            if error:
                return error
            item = _resolve_item(request, parsed)
            if item is None:
                return _error(
                    "unresolved_media",
                    "No exact supported media identity could be resolved.",
                    param="ids",
                    status=HTTP.NOT_FOUND,
                )

            state, _changed = set_watched_state(
                user_id=request.user.pk,
                item=item,
                watched=False,
            )
            state.item = item
            return Response(
                serialize_watched_state(state, include_state=True),
                status=HTTP.OK,
            )

        return _with_idempotency(request, _execute)
