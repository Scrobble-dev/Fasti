# FORK: durable playback progress (resume positions) for third-party clients
# doing bidirectional sync (e.g. CrossWatch). Kept separate from
# fork_views_tracking.py for the same reason as fork_views_scrobble.py: this is
# a public integration surface, not a web-UI parity mirror. See issue #429.
#
# Movies/episodes are stored in app.models.PlaybackProgress; podcasts keep
# using Podcast.played_up_to_seconds, which the podcast UI already reads.
import logging
from datetime import UTC, datetime
from http import HTTPStatus as HTTP  # noqa: N814

from django.db import transaction
from django.db.models import DateTimeField, F, Q
from django.db.models.functions import Coalesce
from django.utils import timezone
from drf_spectacular.utils import extend_schema
from rest_framework import views as drf_views
from rest_framework.response import Response

import app.providers.tmdb
from app.models import (
    Item,
    MediaTypes,
    PlaybackProgress,
    Podcast,
    Sources,
    Status,
)
from app.services.tracking_hydration import ensure_item_metadata
from integrations.delivery import get_or_record_receipt
from integrations.media_identity import MAX_EXTERNAL_ID_LENGTH, item_external_ids
from integrations.models import IntegrationToken

from .helpers import paginate_data, parse_limit_offset, try_parse_datetime_input

logger = logging.getLogger(__name__)

MEDIA_TYPES = (
    MediaTypes.MOVIE.value,
    MediaTypes.EPISODE.value,
    MediaTypes.PODCAST.value,
)
_VIDEO_MEDIA_TYPES = (MediaTypes.MOVIE.value, MediaTypes.EPISODE.value)
_SHOW_MEDIA_TYPES = (MediaTypes.TV.value, MediaTypes.ANIME.value)
_EXTERNAL_ID_KEYS = {"tmdb": "tmdb_id", "imdb": "imdb_id", "tvdb": "tvdb_id"}
_PODCAST_COMPLETED_STATUS = 3  # playingStatus from the podcast provider APIs
_MAX_POSITIVE_INTEGER = 2_147_483_647
_EPOCH = datetime(1970, 1, 1, tzinfo=UTC)
_VIDEO_SNAPSHOT_RANK = 1
_PODCAST_SNAPSHOT_RANK = 0


def _coerce_position(value):
    """Return a portable non-negative PositiveIntegerField value."""
    if isinstance(value, bool):
        msg = "must be an integer"
        raise TypeError(msg)
    position = int(value)
    if position < 0 or position > _MAX_POSITIVE_INTEGER:
        msg = f"must be between 0 and {_MAX_POSITIVE_INTEGER}"
        raise ValueError(msg)
    return position


def _coerce_episode_coordinate(value, *, allow_zero):
    """Return a bounded episode coordinate or raise ValueError/TypeError."""
    if isinstance(value, bool):
        msg = "must be an integer"
        raise TypeError(msg)
    number = int(value)
    minimum = 0 if allow_zero else 1
    if number < minimum or number > _MAX_POSITIVE_INTEGER:
        msg = f"must be between {minimum} and {_MAX_POSITIVE_INTEGER}"
        raise ValueError(msg)
    return number


def _normalize_identifier_value(value):
    """Return one bounded provider identifier or None when malformed."""
    if isinstance(value, bool) or not isinstance(value, (str, int)):
        return None
    normalized = str(value).strip()
    if not normalized or len(normalized) > MAX_EXTERNAL_ID_LENGTH:
        return None
    return normalized


def _normalize_identifiers(raw_ids, keys):
    """Normalize known identifiers while rejecting malformed supplied values."""
    if not isinstance(raw_ids, dict):
        return None

    normalized = {}
    for key in keys:
        value = raw_ids.get(key)
        if value is None or value == "":
            continue
        normalized_value = _normalize_identifier_value(value)
        if normalized_value is None:
            return None
        normalized[key] = normalized_value
    return normalized


def _external_ids(item):
    """Map an Item's exact identifiers back to the playback API vocabulary."""
    if item is None:
        return {}
    exact_ids = item_external_ids(item)
    return {key: value for key, value in exact_ids.items() if key in _EXTERNAL_ID_KEYS}


def _local_alias_candidates(media_type, namespace, value):
    """Return locally persisted TMDB ids for one exact external alias."""
    field = _EXTERNAL_ID_KEYS[namespace]
    queryset = Item.objects.filter(
        source=Sources.TMDB.value,
        **{f"provider_external_ids__{field}": value},
    )
    if media_type == MediaTypes.MOVIE.value:
        queryset = queryset.filter(media_type=MediaTypes.MOVIE.value)
    else:
        queryset = queryset.filter(media_type__in=_SHOW_MEDIA_TYPES)

    return {
        str(media_id)
        for media_id in queryset.values_list("media_id", flat=True).distinct()[:2]
    }


def _remote_alias_candidates(media_type, namespace, value):
    """Resolve one exact external alias through TMDB without choosing among conflicts."""
    ext_type = f"{namespace}_id"
    find_response = app.providers.tmdb.find(value, ext_type)
    if not isinstance(find_response, dict):
        return set()

    if media_type == MediaTypes.MOVIE.value:
        return {
            str(result["id"])
            for result in find_response.get("movie_results") or []
            if isinstance(result, dict) and result.get("id") is not None
        }

    candidates = {
        str(result["show_id"])
        for result in find_response.get("tv_episode_results") or []
        if isinstance(result, dict) and result.get("show_id") is not None
    }
    candidates.update(
        str(result["id"])
        for result in find_response.get("tv_results") or []
        if isinstance(result, dict) and result.get("id") is not None
    )
    return candidates


def _resolve_alias_tmdb_id(media_type, namespace, value):
    """Return one exact TMDB id for an alias, preferring the local identity graph."""
    local_candidates = _local_alias_candidates(media_type, namespace, value)
    if len(local_candidates) == 1:
        return next(iter(local_candidates))
    if len(local_candidates) > 1:
        return None

    remote_candidates = _remote_alias_candidates(media_type, namespace, value)
    if len(remote_candidates) == 1:
        return next(iter(remote_candidates))
    return None


def resolve_show_tmdb_id(media_type, ids):
    """Return one TMDB movie/show id only when every supplied exact id agrees."""
    normalized = _normalize_identifiers(ids, _EXTERNAL_ID_KEYS)
    if not normalized:
        return None

    resolved_ids = []
    tmdb_id = normalized.get("tmdb")
    if tmdb_id:
        resolved_ids.append(tmdb_id)

    for namespace in ("imdb", "tvdb"):
        value = normalized.get(namespace)
        if not value:
            continue
        resolved = _resolve_alias_tmdb_id(media_type, namespace, value)
        if resolved is None:
            return None
        resolved_ids.append(resolved)

    if not resolved_ids or len(set(resolved_ids)) != 1:
        return None
    return resolved_ids[0]


def _find_existing_item(media_type, tmdb_id, season_number, episode_number):
    """Cheap DB lookup for an already-known movie/episode Item.

    The same show can have Item rows in more than one library bucket
    (e.g. grouped anime on TV rows), so this can match either bucket;
    ordering by id keeps the choice deterministic. A resume position is
    bucket-agnostic, so picking the oldest row is safe here.
    """
    filters = {
        "source": Sources.TMDB.value,
        "media_id": str(tmdb_id),
        "media_type": media_type,
    }
    if media_type == MediaTypes.EPISODE.value:
        filters["season_number"] = season_number
        filters["episode_number"] = episode_number
    return Item.objects.filter(**filters).order_by("id").first()


def resolve_video_item(user, media_type, ids, season_number, episode_number, *, create):
    """Resolve a movie/episode `ids` object to an Item, optionally creating it.

    When created, only the metadata Item is written — never a Movie/Episode
    tracking row — so pushing a resume position never fabricates watch history.
    """
    tmdb_id = resolve_show_tmdb_id(media_type, ids)
    if not tmdb_id:
        return None

    item = _find_existing_item(media_type, tmdb_id, season_number, episode_number)
    if item or not create:
        return item

    return ensure_item_metadata(
        user,
        media_type,
        str(tmdb_id),
        Sources.TMDB.value,
        season_number=season_number if media_type == MediaTypes.EPISODE.value else None,
        episode_number=episode_number
        if media_type == MediaTypes.EPISODE.value
        else None,
    ).item


def _resolve_podcast(user, ids):
    """Resolve a podcast `ids` object to the user's existing Podcast row."""
    episode_uuid = ids.get("episode_uuid")
    if not episode_uuid:
        return None
    return (
        Podcast.objects.filter(user=user, episode__episode_uuid=str(episode_uuid))
        .select_related("item", "episode", "show")
        .order_by("-id")
        .first()
    )


def upsert_playback_progress(
    user,
    item,
    position_seconds,
    duration_seconds=None,
    *,
    completed=False,
    preserve_position=False,
    preserve_duration=False,
    fill_missing_duration=False,
    preserve_completed=False,
):
    """Store playback progress while serializing updates for one item.

    The optional preservation flags are used by webhook events whose payload
    intentionally omits one field. The row lock keeps those field-specific
    updates from becoming a stale read-modify-write race.
    """
    position = 0 if position_seconds is None else position_seconds
    with transaction.atomic():
        progress, created = PlaybackProgress.objects.select_for_update().get_or_create(
            user=user,
            item=item,
            defaults={
                "position_seconds": position,
                "duration_seconds": duration_seconds,
                "completed": completed,
            },
        )
        if created:
            return progress

        update_fields = []
        if not preserve_position:
            progress.position_seconds = position
            update_fields.append("position_seconds")

        if fill_missing_duration:
            if progress.duration_seconds is None and duration_seconds is not None:
                progress.duration_seconds = duration_seconds
                update_fields.append("duration_seconds")
        elif not preserve_duration:
            progress.duration_seconds = duration_seconds
            update_fields.append("duration_seconds")

        if not preserve_completed:
            progress.completed = completed
            update_fields.append("completed")

        if update_fields:
            progress.save(update_fields=[*update_fields, "updated_at"])
    return progress


def _show_item_map(items):
    """Return {(source, media_id): Item} of the shows backing episode items."""
    episode_media_ids = {
        (item.source, item.media_id)
        for item in items
        if item.media_type == MediaTypes.EPISODE.value
    }
    if not episode_media_ids:
        return {}

    shows = Item.objects.filter(
        media_type__in=_SHOW_MEDIA_TYPES,
        media_id__in=[media_id for _, media_id in episode_media_ids],
    ).order_by("id")
    return {(show.source, show.media_id): show for show in shows}


def _serialize_progress(progress, show_map):
    """Serialize a PlaybackProgress row into the API entry shape."""
    item = progress.item
    show = (
        show_map.get((item.source, item.media_id))
        if item.media_type == MediaTypes.EPISODE.value
        else None
    )
    # Episode Items are created without provider links, so their show-level
    # ids (which is what clients match on) come from the show row. Fall back
    # to the episode Item only when no exact show identity is available.
    ids = (
        (_external_ids(show) or _external_ids(item))
        if item.media_type == MediaTypes.EPISODE.value
        else _external_ids(item)
    )

    return {
        "media_type": item.media_type,
        "source": item.source,
        "media_id": item.media_id,
        "season_number": item.season_number,
        "episode_number": item.episode_number,
        "ids": ids,
        "title": item.title,
        "series_title": show.title if show else None,
        "position_seconds": progress.position_seconds,
        "duration_seconds": progress.duration_seconds,
        "completed": progress.completed,
        "updated_at": progress.updated_at,
    }


def _podcast_updated_at(podcast):
    """Best-effort mutation time for a podcast position.

    Rows last written before position_updated_at existed fall back to
    progressed_at. Query annotations use the same precedence for filtering
    and ordering so paginated responses stay consistent.
    """
    return (
        getattr(podcast, "_effective_updated_at", None)
        or podcast.position_updated_at
        or podcast.progressed_at
    )


def _serialize_podcast(podcast):
    """Serialize a Podcast row into the API entry shape."""
    item = podcast.item
    episode = podcast.episode

    duration_seconds = None
    if episode and episode.duration:
        duration_seconds = episode.duration
    elif item and item.runtime_minutes:
        duration_seconds = item.runtime_minutes * 60

    return {
        "media_type": MediaTypes.PODCAST.value,
        "source": item.source if item else None,
        "media_id": item.media_id if item else None,
        "season_number": episode.season_number if episode else None,
        "episode_number": episode.episode_number if episode else None,
        "ids": {"episode_uuid": episode.episode_uuid} if episode else {},
        "title": item.title if item else None,
        "series_title": podcast.show.title if podcast.show else None,
        "position_seconds": podcast.played_up_to_seconds,
        "duration_seconds": duration_seconds,
        "completed": (
            podcast.last_seen_status == _PODCAST_COMPLETED_STATUS
            or podcast.status == Status.COMPLETED.value
        ),
        "updated_at": _podcast_updated_at(podcast),
    }


def _parse_list_filters(request):
    """Return (filters, error_response) for the GET query params."""
    raw_media_types = request.GET.get("media_type") or ""
    media_types = [part for part in raw_media_types.split(",") if part]
    invalid = [part for part in media_types if part not in MEDIA_TYPES]
    if invalid:
        return None, Response(
            {"detail": f"'media_type' must be one of {MEDIA_TYPES}."},
            status=HTTP.BAD_REQUEST,
        )

    updated_since = None
    raw_updated_since = request.GET.get("updated_since")
    if raw_updated_since:
        try:
            updated_since = try_parse_datetime_input(raw_updated_since)
        except (TypeError, ValueError):
            return None, Response(
                {"detail": "Invalid updated_since format."},
                status=HTTP.BAD_REQUEST,
            )

    completed = None
    raw_completed = request.GET.get("completed")
    if raw_completed is not None and raw_completed != "":
        if raw_completed.lower() not in ("true", "false"):
            return None, Response(
                {"detail": "'completed' must be 'true' or 'false'."},
                status=HTTP.BAD_REQUEST,
            )
        completed = raw_completed.lower() == "true"

    return {
        "media_types": media_types or list(MEDIA_TYPES),
        "updated_since": updated_since,
        "completed": completed,
    }, None


def _identifier_error(data, media_type):
    """Return a 400 Response if the media identifiers are unusable, else None."""
    raw_ids = data.get("ids")
    if not isinstance(raw_ids, dict):
        return Response(
            {"detail": "'ids' must be an object."},
            status=HTTP.BAD_REQUEST,
        )

    if media_type == MediaTypes.PODCAST.value:
        normalized = _normalize_identifiers(raw_ids, ("episode_uuid",))
        if normalized is None or not normalized.get("episode_uuid"):
            return Response(
                {
                    "detail": (
                        "'ids.episode_uuid' must be a non-empty string or integer "
                        f"of at most {MAX_EXTERNAL_ID_LENGTH} characters."
                    )
                },
                status=HTTP.BAD_REQUEST,
            )
        return None

    normalized = _normalize_identifiers(raw_ids, _EXTERNAL_ID_KEYS)
    if normalized is None:
        return Response(
            {
                "detail": (
                    "Movie and episode identifiers must be strings or integers "
                    f"of at most {MAX_EXTERNAL_ID_LENGTH} characters."
                )
            },
            status=HTTP.BAD_REQUEST,
        )
    if not normalized:
        return Response(
            {"detail": "'ids' must include at least one of tmdb/imdb/tvdb."},
            status=HTTP.BAD_REQUEST,
        )

    if media_type == MediaTypes.EPISODE.value:
        if data.get("season_number") is None or data.get("episode_number") is None:
            return Response(
                {
                    "detail": (
                        "'season_number' and 'episode_number' are required "
                        "for media_type 'episode'."
                    ),
                },
                status=HTTP.BAD_REQUEST,
            )
        try:
            _coerce_episode_coordinate(data.get("season_number"), allow_zero=True)
            _coerce_episode_coordinate(data.get("episode_number"), allow_zero=False)
        except (TypeError, ValueError):
            return Response(
                {
                    "detail": (
                        "'season_number' must be an integer >= 0 and "
                        "'episode_number' must be an integer >= 1."
                    )
                },
                status=HTTP.BAD_REQUEST,
            )

    return None


def _seconds_error(data, *, require_position):
    """Return a 400 Response if the seconds fields are unusable, else None."""
    raw_position = data.get("position_seconds")
    if raw_position is None and require_position and "position_seconds" not in data:
        return Response(
            {
                "detail": (
                    "'position_seconds' is required (send null to clear "
                    "the saved position)."
                ),
            },
            status=HTTP.BAD_REQUEST,
        )

    for field in ("position_seconds", "duration_seconds"):
        value = data.get(field)
        if value is None:
            continue
        try:
            _coerce_position(value)
        except (TypeError, ValueError):
            return Response(
                {
                    "detail": (
                        f"'{field}' must be an integer between 0 and "
                        f"{_MAX_POSITIVE_INTEGER}."
                    )
                },
                status=HTTP.BAD_REQUEST,
            )

    return None


def _completed_error(data):
    """Return a 400 Response when completed is not a JSON boolean."""
    completed = data.get("completed")
    if completed is not None and not isinstance(completed, bool):
        return Response(
            {"detail": "'completed' must be true or false."},
            status=HTTP.BAD_REQUEST,
        )
    return None


def _write_request_error(data, *, require_position):
    """Return a 400 Response if the write payload is malformed, else None."""
    if not isinstance(data, dict):
        return Response(
            {"detail": "Request body must be a JSON object."},
            status=HTTP.BAD_REQUEST,
        )

    media_type = data.get("media_type")
    if media_type not in MEDIA_TYPES:
        return Response(
            {"detail": f"'media_type' must be one of {MEDIA_TYPES}."},
            status=HTTP.BAD_REQUEST,
        )

    return (
        _identifier_error(data, media_type)
        or _seconds_error(data, require_position=require_position)
        or _completed_error(data)
    )


_ENTRY_SCHEMA = {
    "type": "object",
    "properties": {
        "media_type": {"type": "string", "enum": list(MEDIA_TYPES)},
        "source": {"type": "string", "nullable": True},
        "media_id": {"type": "string", "nullable": True},
        "season_number": {"type": "integer", "nullable": True},
        "episode_number": {"type": "integer", "nullable": True},
        "ids": {"type": "object"},
        "title": {"type": "string", "nullable": True},
        "series_title": {"type": "string", "nullable": True},
        "position_seconds": {"type": "integer", "nullable": True},
        "duration_seconds": {"type": "integer", "nullable": True},
        "completed": {"type": "boolean"},
        "updated_at": {"type": "string", "format": "date-time", "nullable": True},
    },
}

_WRITE_SCHEMA = {
    "type": "object",
    "required": ["media_type", "ids"],
    "properties": {
        "media_type": {"type": "string", "enum": list(MEDIA_TYPES)},
        "ids": {
            "type": "object",
            "description": (
                "Movies/episodes: at least one of tmdb/imdb/tvdb. "
                "Podcasts: episode_uuid (Pocket Casts UUID or RSS GUID)."
            ),
            "properties": {
                "tmdb": {
                    "type": "string",
                    "nullable": True,
                    "maxLength": MAX_EXTERNAL_ID_LENGTH,
                },
                "imdb": {
                    "type": "string",
                    "nullable": True,
                    "maxLength": MAX_EXTERNAL_ID_LENGTH,
                },
                "tvdb": {
                    "type": "string",
                    "nullable": True,
                    "maxLength": MAX_EXTERNAL_ID_LENGTH,
                },
                "episode_uuid": {
                    "type": "string",
                    "nullable": True,
                    "maxLength": MAX_EXTERNAL_ID_LENGTH,
                },
            },
        },
        "season_number": {
            "type": "integer",
            "minimum": 0,
            "maximum": _MAX_POSITIVE_INTEGER,
            "nullable": True,
            "description": "Required for media_type 'episode'.",
        },
        "episode_number": {
            "type": "integer",
            "minimum": 1,
            "maximum": _MAX_POSITIVE_INTEGER,
            "nullable": True,
            "description": "Required for media_type 'episode'.",
        },
        "position_seconds": {
            "type": "integer",
            "minimum": 0,
            "maximum": _MAX_POSITIVE_INTEGER,
            "nullable": True,
            "description": "Absolute resume position. null clears it.",
        },
        "duration_seconds": {
            "type": "integer",
            "minimum": 0,
            "maximum": _MAX_POSITIVE_INTEGER,
            "nullable": True,
        },
        "completed": {"type": "boolean", "nullable": True},
    },
}

_DETAIL_SCHEMA = {"type": "object", "properties": {"detail": {"type": "string"}}}


# /api/v1/playback/progress/
class PlaybackProgressView(drf_views.APIView):
    """Durable resume positions for movies, episodes and podcasts.

    Complements /api/v1/scrobble/, which can push playback progress in but
    can't read saved positions back out. Positions here are written by API
    clients, by scrobble 'stop', and by media-server webhooks reporting a
    position (Plex pause/resume/stop/scrobble, Jellyfin pause/stop,
    Emby stop).
    """

    @extend_schema(
        description=(
            "List saved resume positions, newest first. Supports "
            "?updated_since= for delta sync, ?media_type= (comma separated) "
            "and ?completed=true|false, plus the standard limit/offset."
        ),
        responses={
            200: {
                "type": "object",
                "properties": {
                    "pagination": {"type": "object"},
                    "results": {"type": "array", "items": _ENTRY_SCHEMA},
                },
            },
            400: _DETAIL_SCHEMA,
        },
    )
    def get(self, request):
        """Return the user's saved resume positions without serializing the full library."""
        limit, offset, error = parse_limit_offset(request)
        if error:
            return error

        filters, error = _parse_list_filters(request)
        if error:
            return error

        video_types = [
            media_type
            for media_type in filters["media_types"]
            if media_type in _VIDEO_MEDIA_TYPES
        ]
        video_queryset = (
            self._video_queryset(request.user, video_types, filters)
            if video_types
            else None
        )
        podcast_queryset = (
            self._podcast_queryset(request.user, filters)
            if MediaTypes.PODCAST.value in filters["media_types"]
            else None
        )

        video_total = video_queryset.count() if video_queryset is not None else 0
        podcast_total = podcast_queryset.count() if podcast_queryset is not None else 0
        total = video_total + podcast_total
        if offset >= total:
            return Response(
                paginate_data(request, [], limit, offset, total=total),
                status=HTTP.OK,
            )

        candidate_limit = min(offset + limit, total)
        candidates = []
        if video_queryset is not None:
            video_rows = list(
                video_queryset.order_by("-updated_at", "-id")[:candidate_limit]
            )
            show_map = _show_item_map([row.item for row in video_rows])
            candidates.extend(
                (
                    row.updated_at or _EPOCH,
                    _VIDEO_SNAPSHOT_RANK,
                    row.id,
                    _serialize_progress(row, show_map),
                )
                for row in video_rows
            )

        if podcast_queryset is not None:
            podcast_rows = list(
                podcast_queryset.order_by(
                    F("_effective_updated_at").desc(nulls_last=True),
                    "-id",
                )[:candidate_limit]
            )
            candidates.extend(
                (
                    _podcast_updated_at(podcast) or _EPOCH,
                    _PODCAST_SNAPSHOT_RANK,
                    podcast.id,
                    _serialize_podcast(podcast),
                )
                for podcast in podcast_rows
            )

        candidates.sort(key=lambda candidate: candidate[:3], reverse=True)
        entries = [candidate[3] for candidate in candidates]
        return Response(
            paginate_data(request, entries, limit, offset, total=total),
            status=HTTP.OK,
        )

    def _video_queryset(self, user, media_types, filters):
        """Return the filtered video progress queryset without evaluating it."""
        queryset = PlaybackProgress.objects.filter(
            user=user,
            item__media_type__in=media_types,
        ).select_related("item")
        if filters["updated_since"]:
            queryset = queryset.filter(updated_at__gte=filters["updated_since"])
        if filters["completed"] is not None:
            queryset = queryset.filter(completed=filters["completed"])
        return queryset

    def _podcast_queryset(self, user, filters):
        """Return filtered podcast positions with database-side freshness semantics."""
        queryset = (
            Podcast.objects.filter(
                user=user,
                played_up_to_seconds__gt=0,
            )
            .select_related("item", "episode", "show")
            .annotate(
                _effective_updated_at=Coalesce(
                    "position_updated_at",
                    "progressed_at",
                    output_field=DateTimeField(),
                )
            )
        )

        if filters["updated_since"]:
            queryset = queryset.filter(
                _effective_updated_at__gte=filters["updated_since"]
            )

        if filters["completed"] is not None:
            completed_query = Q(last_seen_status=_PODCAST_COMPLETED_STATUS) | Q(
                status=Status.COMPLETED.value
            )
            queryset = (
                queryset.filter(completed_query)
                if filters["completed"]
                else queryset.exclude(completed_query)
            )
        return queryset

    @extend_schema(
        description=(
            "Set an absolute resume position. Sending "
            '"position_seconds": null clears it, for clients that cannot '
            "send a DELETE body. Movies/episodes create the metadata item "
            "if it isn't known yet, without creating a watch record; "
            "podcasts require an already-tracked episode."
        ),
        request={"application/json": _WRITE_SCHEMA},
        responses={200: _ENTRY_SCHEMA, 400: _DETAIL_SCHEMA, 404: _DETAIL_SCHEMA},
    )
    def put(self, request):
        """Set or clear a resume position."""
        client_event_id = request.headers.get("Idempotency-Key")
        if not client_event_id and isinstance(request.data, dict):
            client_event_id = request.data.get("client_event_id")

        token = request.auth if isinstance(request.auth, IntegrationToken) else None

        def _execute():
            error = _write_request_error(request.data, require_position=True)
            if error:
                return error

            if request.data.get("position_seconds") is None:
                return self._clear(request)

            media_type = request.data.get("media_type")
            if media_type == MediaTypes.PODCAST.value:
                return self._put_podcast(request)
            return self._put_video(request, media_type)

        if client_event_id:
            response, _ = get_or_record_receipt(
                user=request.user,
                client_event_id=str(client_event_id),
                payload=request.data,
                execute_fn=_execute,
                token=token,
            )
            return response

        return _execute()

    def _put_video(self, request, media_type):
        """Upsert a movie/episode position, creating the Item if needed."""
        data = request.data
        ids = _normalize_identifiers(data.get("ids"), _EXTERNAL_ID_KEYS) or {}
        season_number = data.get("season_number")
        episode_number = data.get("episode_number")
        if media_type == MediaTypes.EPISODE.value:
            season_number = _coerce_episode_coordinate(season_number, allow_zero=True)
            episode_number = _coerce_episode_coordinate(episode_number, allow_zero=False)
        try:
            item = resolve_video_item(
                request.user,
                media_type,
                ids,
                season_number,
                episode_number,
                create=True,
            )
        except Exception:
            logger.warning("Could not resolve playback media", exc_info=True)
            return Response(
                {"detail": "Could not resolve media."},
                status=HTTP.NOT_FOUND,
            )
        if item is None:
            return Response(
                {"detail": "Could not resolve media."},
                status=HTTP.NOT_FOUND,
            )

        duration = data.get("duration_seconds")
        progress = upsert_playback_progress(
            request.user,
            item,
            _coerce_position(data.get("position_seconds")),
            _coerce_position(duration) if duration is not None else None,
            completed=data.get("completed") is True,
        )
        progress.item = item
        show_map = _show_item_map([item])
        return Response(_serialize_progress(progress, show_map), status=HTTP.OK)

    def _put_podcast(self, request):
        """Write a podcast position onto the existing Podcast row."""
        ids = _normalize_identifiers(request.data.get("ids"), ("episode_uuid",)) or {}
        podcast = _resolve_podcast(request.user, ids)
        if podcast is None:
            return Response(
                {"detail": "No tracked podcast episode matches that episode_uuid."},
                status=HTTP.NOT_FOUND,
            )

        podcast.played_up_to_seconds = _coerce_position(
            request.data.get("position_seconds"),
        )
        podcast.position_updated_at = timezone.now()
        update_fields = ["played_up_to_seconds", "position_updated_at"]
        if request.data.get("completed") is True:
            podcast.status = Status.COMPLETED.value
            podcast.last_seen_status = _PODCAST_COMPLETED_STATUS
            update_fields += ["status", "last_seen_status"]
        podcast.save(update_fields=update_fields)

        return Response(_serialize_podcast(podcast), status=HTTP.OK)

    @extend_schema(
        description="Clear a saved resume position.",
        request={"application/json": _WRITE_SCHEMA},
        responses={204: None, 400: _DETAIL_SCHEMA, 404: _DETAIL_SCHEMA},
    )
    def delete(self, request):
        """Clear a saved resume position."""
        client_event_id = request.headers.get("Idempotency-Key")
        if not client_event_id and isinstance(request.data, dict):
            client_event_id = request.data.get("client_event_id")

        token = request.auth if isinstance(request.auth, IntegrationToken) else None

        def _execute():
            error = _write_request_error(request.data, require_position=False)
            if error:
                return error
            return self._clear(request)

        if client_event_id:
            response, _ = get_or_record_receipt(
                user=request.user,
                client_event_id=str(client_event_id),
                payload=request.data,
                execute_fn=_execute,
                token=token,
            )
            return response

        return _execute()

    def _clear(self, request):
        """Delete the stored position for the identified media."""
        data = request.data
        media_type = data.get("media_type")

        if media_type == MediaTypes.PODCAST.value:
            ids = _normalize_identifiers(data.get("ids"), ("episode_uuid",)) or {}
            podcast = _resolve_podcast(request.user, ids)
            if podcast is None:
                return Response(
                    {"detail": "No tracked podcast episode matches that episode_uuid."},
                    status=HTTP.NOT_FOUND,
                )
            podcast.played_up_to_seconds = None
            podcast.position_updated_at = timezone.now()
            podcast.save(
                update_fields=["played_up_to_seconds", "position_updated_at"],
            )
            return Response(status=HTTP.NO_CONTENT)

        ids = _normalize_identifiers(data.get("ids"), _EXTERNAL_ID_KEYS) or {}
        season_number = data.get("season_number")
        episode_number = data.get("episode_number")
        if media_type == MediaTypes.EPISODE.value:
            season_number = _coerce_episode_coordinate(season_number, allow_zero=True)
            episode_number = _coerce_episode_coordinate(episode_number, allow_zero=False)
        try:
            item = resolve_video_item(
                request.user,
                media_type,
                ids,
                season_number,
                episode_number,
                create=False,
            )
        except Exception:
            logger.warning("Could not resolve playback media for clear", exc_info=True)
            return Response(
                {"detail": "Could not resolve media."},
                status=HTTP.NOT_FOUND,
            )
        if item is None:
            return Response(
                {"detail": "Could not resolve media."},
                status=HTTP.NOT_FOUND,
            )

        PlaybackProgress.objects.filter(user=request.user, item=item).delete()
        return Response(status=HTTP.NO_CONTENT)
