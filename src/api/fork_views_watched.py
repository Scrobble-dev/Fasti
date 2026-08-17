"""Exact watched-state snapshot for third-party integration clients."""

from http import HTTPStatus as HTTP  # noqa: N814

from django.db.models import F, Max, Q
from drf_spectacular.utils import OpenApiParameter, extend_schema
from rest_framework import views as drf_views
from rest_framework.response import Response

from app.models import Episode, Item, MediaTypes, Movie, Status
from integrations.media_identity import item_external_ids

from .helpers import make_page_url, parse_limit_offset

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


def _error(code: str, message: str, *, param: str):
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
        status=HTTP.BAD_REQUEST,
    )


def _pagination(request, total: int, limit: int, offset: int) -> dict:
    """Build the standard Floppy limit/offset envelope without loading all rows."""
    next_url = make_page_url(request, limit, offset + limit) if offset + limit < total else None
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


def _serialize_item(item: Item, watched_at) -> dict:
    """Return one portable watched-state row from an exact Item identity."""
    return {
        "media_type": item.media_type,
        "library_media_type": item.library_media_type or None,
        "source": item.source,
        "media_id": str(item.media_id),
        "ids": item_external_ids(item),
        "season_number": item.season_number,
        "episode_number": item.episode_number,
        "watched_at": watched_at,
    }


def _movie_snapshot(user, limit: int, offset: int):
    """Return one current watched-state row per movie without collapsing history."""
    queryset = (
        Movie.objects.filter(user=user)
        .filter(
            Q(status=Status.COMPLETED.value)
            | Q(end_date__isnull=False)
            | Q(plays__isnull=False)
        )
        .select_related("item")
        .distinct()
        .order_by(F("end_date").desc(nulls_last=True), "-created_at", "-id")
    )
    total = queryset.count()
    page = list(queryset[offset : offset + limit])
    return total, [_serialize_item(movie.item, movie.end_date) for movie in page]


def _episode_snapshot(user, limit: int, offset: int):
    """Collapse repeated episode occurrences to one exact current watched-state row."""
    grouped = (
        Episode.objects.filter(
            related_season__user=user,
            status=Status.COMPLETED.value,
            item__isnull=False,
        )
        .values("item_id")
        .annotate(
            watched_at=Max("end_date"),
            observed_at=Max("created_at"),
        )
        .order_by(
            F("watched_at").desc(nulls_last=True),
            "-observed_at",
            "-item_id",
        )
    )
    total = grouped.count()
    page = list(grouped[offset : offset + limit])
    items = Item.objects.in_bulk(row["item_id"] for row in page)
    results = []
    for row in page:
        item = items.get(row["item_id"])
        if item is not None:
            results.append(_serialize_item(item, row["watched_at"]))
    return total, results


class WatchedStateView(drf_views.APIView):
    """Return exact current watched state without mutating playback history."""

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
        """Return one bounded watched-state snapshot for the authenticated user."""
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

        if media_type == MediaTypes.MOVIE.value:
            total, results = _movie_snapshot(request.user, limit, offset)
        else:
            total, results = _episode_snapshot(request.user, limit, offset)

        return Response(
            {
                "pagination": _pagination(request, total, limit, offset),
                "results": results,
            },
            status=HTTP.OK,
        )
