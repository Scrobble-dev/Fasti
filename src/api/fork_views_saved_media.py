"""Saved-library membership endpoints for third-party synchronization."""

from http import HTTPStatus as HTTP  # noqa: N814

from django.core import signing
from django.core.signing import BadSignature, SignatureExpired
from django.db import transaction
from django.db.models import Q
from drf_spectacular.utils import extend_schema
from rest_framework import views as drf_views
from rest_framework.response import Response

from app.models import Item, MediaTypes, SavedMediaChange, SavedMediaMembership, Sources
from app.playback_context import get_playback_source_client_id
from app.services.tracking_hydration import ensure_item_metadata
from integrations.delivery import get_or_record_receipt
from integrations.models import IntegrationToken

from .fork_views_playback import resolve_show_tmdb_id
from .helpers import paginate_data, parse_limit_offset

_SUPPORTED_MEDIA_TYPES = (
    MediaTypes.MOVIE.value,
    MediaTypes.TV.value,
    MediaTypes.ANIME.value,
)
_EXTERNAL_ID_FIELDS = {
    "tmdb": "tmdb_id",
    "imdb": "imdb_id",
    "tvdb": "tvdb_id",
    "mal": "mal_id",
    "anilist": "anilist_id",
    "kitsu": "kitsu_id",
}
_DIRECT_SOURCES = {
    "tmdb": Sources.TMDB.value,
    "imdb": Sources.IMDB.value,
    "tvdb": Sources.TVDB.value,
    "mal": Sources.MAL.value,
}
_MAX_EXTERNAL_ID_LENGTH = 500
_CURSOR_VERSION = 1
_CURSOR_RESOURCE = "saved_media"
_CURSOR_SALT = "api.saved-media-changes"
_CURSOR_MAX_AGE_SECONDS = 30 * 24 * 60 * 60
_DEFAULT_CHANGE_LIMIT = 100
_MAX_CHANGE_LIMIT = 100

_ENTRY_SCHEMA = {
    "type": "object",
    "properties": {
        "media_type": {"type": "string", "enum": list(_SUPPORTED_MEDIA_TYPES)},
        "source": {"type": "string"},
        "media_id": {"type": "string"},
        "ids": {"type": "object"},
        "title": {"type": "string"},
        "saved_at": {"type": "string", "format": "date-time"},
        "updated_at": {"type": "string", "format": "date-time"},
    },
}
_WRITE_SCHEMA = {
    "type": "object",
    "required": ["media_type", "ids"],
    "properties": {
        "media_type": {"type": "string", "enum": list(_SUPPORTED_MEDIA_TYPES)},
        "ids": {
            "type": "object",
            "properties": {
                namespace: {
                    "type": "string",
                    "nullable": True,
                    "maxLength": _MAX_EXTERNAL_ID_LENGTH,
                }
                for namespace in _EXTERNAL_ID_FIELDS
            },
        },
    },
}
_CHANGE_SCHEMA = {
    "type": "object",
    "properties": {
        "sequence_id": {"type": "integer"},
        "operation": {"type": "string", "enum": ["add", "remove"]},
        "media_type": {"type": "string"},
        "source": {"type": "string"},
        "media_id": {"type": "string"},
        "ids": {"type": "object"},
        "source_client_id": {"type": "string"},
        "created_at": {"type": "string", "format": "date-time"},
    },
}


def _public_error(code: str, message: str, status: int, *, param: str | None = None):
    """Return a stable public error without provider or database details."""
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


def _item_ids(item: Item) -> dict[str, str]:
    """Return exact external IDs in the public integration vocabulary."""
    result = {}
    stored = item.provider_external_ids or {}
    for namespace, field in _EXTERNAL_ID_FIELDS.items():
        value = stored.get(field)
        if isinstance(value, (str, int)) and not isinstance(value, bool) and str(value):
            result[namespace] = str(value)
    for namespace, source in _DIRECT_SOURCES.items():
        if item.source == source and item.media_id:
            result.setdefault(namespace, str(item.media_id))
    return result


def _parse_identity(data):
    """Return normalized exact media identity or a stable public error."""
    media_type = data.get("media_type") if isinstance(data, dict) else None
    if media_type not in _SUPPORTED_MEDIA_TYPES:
        return None, None, _public_error(
            "unsupported_media_type",
            f"media_type must be one of: {', '.join(_SUPPORTED_MEDIA_TYPES)}.",
            HTTP.BAD_REQUEST,
            param="media_type",
        )

    raw_ids = data.get("ids") if isinstance(data, dict) else None
    if not isinstance(raw_ids, dict):
        return None, None, _public_error(
            "missing_media_identity",
            "ids must be an object with at least one supported provider identifier.",
            HTTP.BAD_REQUEST,
            param="ids",
        )

    normalized = {}
    for namespace in _EXTERNAL_ID_FIELDS:
        value = raw_ids.get(namespace)
        if value is None or value == "":
            continue
        if isinstance(value, bool) or not isinstance(value, (str, int)):
            return None, None, _public_error(
                "invalid_media_identity",
                f"ids.{namespace} must be a string or integer.",
                HTTP.BAD_REQUEST,
                param=f"ids.{namespace}",
            )
        value = str(value).strip()
        if not value:
            return None, None, _public_error(
                "invalid_media_identity",
                f"ids.{namespace} must not be empty.",
                HTTP.BAD_REQUEST,
                param=f"ids.{namespace}",
            )
        if len(value) > _MAX_EXTERNAL_ID_LENGTH:
            return None, None, _public_error(
                "invalid_media_identity",
                f"ids.{namespace} must be {_MAX_EXTERNAL_ID_LENGTH} characters or fewer.",
                HTTP.BAD_REQUEST,
                param=f"ids.{namespace}",
            )
        normalized[namespace] = value

    if not normalized:
        return None, None, _public_error(
            "missing_media_identity",
            "ids must contain at least one supported provider identifier.",
            HTTP.BAD_REQUEST,
            param="ids",
        )
    return media_type, normalized, None


def _identity_filter(namespace: str, value: str) -> Q:
    """Return the exact alias predicate for one provider identifier."""
    field = _EXTERNAL_ID_FIELDS[namespace]
    predicate = Q(**{f"provider_external_ids__{field}": value})
    source = _DIRECT_SOURCES.get(namespace)
    if source:
        predicate |= Q(source=source, media_id=value)
    return predicate


def _item_matches_ids(item: Item, ids: dict[str, str]) -> bool:
    """Return True only when every supplied identifier matches one Item."""
    actual_ids = _item_ids(item)
    return all(actual_ids.get(namespace) == value for namespace, value in ids.items())


def _find_existing_item(media_type: str, ids: dict) -> Item | None:
    """Resolve one Item only when all supplied exact aliases agree."""
    queryset = Item.objects.filter(media_type=media_type)
    for namespace, value in ids.items():
        queryset = queryset.filter(_identity_filter(namespace, value))
    return queryset.order_by("id").first()


def _resolve_saved_item(user, media_type: str, ids: dict, *, create: bool) -> Item | None:
    """Resolve one exact saved-media identity, creating metadata only when safe."""
    existing = _find_existing_item(media_type, ids)
    if existing is not None or not create:
        return existing

    if media_type == MediaTypes.ANIME.value:
        mal_id = ids.get("mal")
        if not mal_id:
            return None
        hydrated = ensure_item_metadata(
            user,
            MediaTypes.ANIME.value,
            mal_id,
            Sources.MAL.value,
        ).item
        return hydrated if _item_matches_ids(hydrated, ids) else None

    tmdb_id = resolve_show_tmdb_id(media_type, ids)
    if not tmdb_id:
        return None
    hydrated = ensure_item_metadata(
        user,
        media_type,
        str(tmdb_id),
        Sources.TMDB.value,
    ).item
    return hydrated if _item_matches_ids(hydrated, ids) else None


def _serialize_membership(membership: SavedMediaMembership) -> dict:
    """Serialize one saved-library membership."""
    item = membership.item
    return {
        "media_type": item.media_type,
        "source": item.source,
        "media_id": str(item.media_id),
        "ids": _item_ids(item),
        "title": item.title,
        "saved_at": membership.created_at,
        "updated_at": membership.updated_at,
    }


def _with_idempotency(request, execute_fn):
    """Execute one write through the durable receipt gateway when requested."""
    client_event_id = request.headers.get("Idempotency-Key")
    if not client_event_id and isinstance(request.data, dict):
        client_event_id = request.data.get("client_event_id")
    if not client_event_id:
        return execute_fn()

    token = request.auth if isinstance(request.auth, IntegrationToken) else None
    response, _ = get_or_record_receipt(
        user=request.user,
        client_event_id=str(client_event_id),
        payload=request.data,
        execute_fn=execute_fn,
        token=token,
    )
    return response


def _cursor_error(code: str, message: str, status: int) -> Response:
    """Return a stable saved-media cursor error."""
    return _public_error(code, message, status, param="cursor")


def _encode_cursor(user_id: int, sequence_id: int) -> str:
    """Sign one user-bound saved-media cursor."""
    return signing.dumps(
        {
            "v": _CURSOR_VERSION,
            "resource": _CURSOR_RESOURCE,
            "user": user_id,
            "sequence": sequence_id,
        },
        salt=_CURSOR_SALT,
        compress=True,
    )


def _decode_cursor(cursor: str, user_id: int) -> tuple[int | None, Response | None]:
    """Return the validated saved-media sequence encoded in a cursor."""
    try:
        payload = signing.loads(
            cursor,
            salt=_CURSOR_SALT,
            max_age=_CURSOR_MAX_AGE_SECONDS,
        )
    except SignatureExpired:
        return None, _cursor_error(
            "cursor_expired",
            "This cursor expired. Fetch a fresh saved-media snapshot before continuing.",
            HTTP.CONFLICT,
        )
    except BadSignature:
        return None, _cursor_error(
            "invalid_cursor",
            "The saved-media cursor is invalid.",
            HTTP.BAD_REQUEST,
        )

    if not isinstance(payload, dict) or (
        payload.get("v") != _CURSOR_VERSION
        or payload.get("resource") != _CURSOR_RESOURCE
        or payload.get("user") != user_id
    ):
        return None, _cursor_error(
            "invalid_cursor",
            "The saved-media cursor is not valid for this user or resource.",
            HTTP.BAD_REQUEST,
        )

    sequence_id = payload.get("sequence")
    if not isinstance(sequence_id, int) or sequence_id < 0:
        return None, _cursor_error(
            "invalid_cursor",
            "The saved-media cursor contains an invalid sequence.",
            HTTP.BAD_REQUEST,
        )
    return sequence_id, None


def _parse_change_limit(request) -> tuple[int | None, Response | None]:
    """Return a bounded page size for saved-media changes."""
    raw_limit = request.query_params.get("limit", _DEFAULT_CHANGE_LIMIT)
    try:
        limit = int(raw_limit)
    except (TypeError, ValueError):
        limit = 0
    if limit < 1 or limit > _MAX_CHANGE_LIMIT:
        return None, _public_error(
            "invalid_limit",
            f"limit must be between 1 and {_MAX_CHANGE_LIMIT}.",
            HTTP.BAD_REQUEST,
            param="limit",
        )
    return limit, None


def _serialize_change(change: SavedMediaChange) -> dict:
    """Serialize one saved-library membership change."""
    return {
        "sequence_id": change.sequence_id,
        "operation": change.operation,
        "media_type": change.media_type,
        "source": change.source,
        "media_id": change.media_id,
        "ids": change.ids,
        "source_client_id": change.source_client_id,
        "created_at": change.created_at,
    }


class SavedMediaView(drf_views.APIView):
    """List and explicitly mutate saved-library membership."""

    @extend_schema(
        description=(
            "List explicit saved-library membership. This resource is separate from "
            "watched state, history, playback progress, Floppy lists, and Nuvio Collections."
        ),
        responses={200: {"type": "object"}},
    )
    def get(self, request):
        """Return a bounded snapshot of the user's saved media."""
        limit, offset, error = parse_limit_offset(request)
        if error:
            return error
        rows = SavedMediaMembership.objects.filter(user=request.user).select_related("item")
        entries = [_serialize_membership(row) for row in rows]
        return Response(paginate_data(request, entries, limit, offset), status=HTTP.OK)

    @extend_schema(
        description=(
            "Save one exact movie, TV show, or anime item. The operation does not "
            "change watched state, progress, history, tracking status, or list membership."
        ),
        request={"application/json": _WRITE_SCHEMA},
        responses={200: _ENTRY_SCHEMA, 201: _ENTRY_SCHEMA, 400: {"type": "object"}},
    )
    def put(self, request):
        """Add one explicit saved-media membership."""

        def _execute():
            media_type, ids, error = _parse_identity(request.data)
            if error:
                return error
            try:
                item = _resolve_saved_item(request.user, media_type, ids, create=True)
            except Exception:
                item = None
            if item is None:
                return _public_error(
                    "unresolved_media",
                    "No exact supported media identity could be resolved.",
                    HTTP.NOT_FOUND,
                    param="ids",
                )

            with transaction.atomic():
                membership, created = SavedMediaMembership.objects.get_or_create(
                    user=request.user,
                    item=item,
                )
            membership.item = item
            return Response(
                _serialize_membership(membership),
                status=HTTP.CREATED if created else HTTP.OK,
            )

        return _with_idempotency(request, _execute)

    @extend_schema(
        description=(
            "Remove one explicit saved-library membership. This never deletes or "
            "downgrades the user's tracked, watched, progress, history, or list state."
        ),
        request={"application/json": _WRITE_SCHEMA},
        responses={204: None, 400: {"type": "object"}, 404: {"type": "object"}},
    )
    def delete(self, request):
        """Remove one explicit saved-media membership."""

        def _execute():
            media_type, ids, error = _parse_identity(request.data)
            if error:
                return error
            try:
                item = _resolve_saved_item(request.user, media_type, ids, create=False)
            except Exception:
                item = None
            if item is None:
                return _public_error(
                    "unresolved_media",
                    "No exact supported media identity could be resolved.",
                    HTTP.NOT_FOUND,
                    param="ids",
                )

            with transaction.atomic():
                membership = SavedMediaMembership.objects.filter(
                    user=request.user,
                    item=item,
                ).first()
                if membership is None:
                    return Response(status=HTTP.NO_CONTENT)
                membership.delete()
            return Response(status=HTTP.NO_CONTENT)

        return _with_idempotency(request, _execute)


class SavedMediaChangesView(drf_views.APIView):
    """Return an initial checkpoint or ordered saved-library changes."""

    @extend_schema(
        description=(
            "Create a snapshot checkpoint when cursor is omitted, or list explicit "
            "saved-media add/remove events after a signed cursor. Fetch /api/v1/watchlist "
            "after the initial checkpoint, then apply changes from the returned cursor."
        ),
        responses={200: {"type": "object"}, 400: {"type": "object"}, 409: {"type": "object"}},
    )
    def get(self, request):
        """Return a checkpoint or one bounded ordered page of saved-media changes."""
        limit, error = _parse_change_limit(request)
        if error:
            return error

        cursor = request.query_params.get("cursor")
        if not cursor:
            high_water = (
                SavedMediaChange.objects.filter(user=request.user)
                .order_by("-sequence_id")
                .values_list("sequence_id", flat=True)
                .first()
                or 0
            )
            return Response(
                {
                    "version": _CURSOR_VERSION,
                    "results": [],
                    "next_cursor": _encode_cursor(request.user.pk, high_water),
                    "has_more": False,
                    "snapshot_required": True,
                    "client_id": get_playback_source_client_id(),
                    "cursor_expires_in_seconds": _CURSOR_MAX_AGE_SECONDS,
                },
                status=HTTP.OK,
            )

        sequence_id, error = _decode_cursor(cursor, request.user.pk)
        if error:
            return error

        rows = list(
            SavedMediaChange.objects.filter(
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
                "next_cursor": _encode_cursor(request.user.pk, next_sequence),
                "has_more": has_more,
                "snapshot_required": False,
                "client_id": get_playback_source_client_id(),
                "cursor_expires_in_seconds": _CURSOR_MAX_AGE_SECONDS,
            },
            status=HTTP.OK,
        )
