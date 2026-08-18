"""Read-only Stremio Addon Protocol publication for public Floppy lists."""

from http import HTTPStatus as HTTP  # noqa: N814
from typing import NoReturn
from urllib.parse import parse_qs

from django.contrib.auth.decorators import login_not_required
from django.db.models import Q
from django.http import Http404, JsonResponse
from django.views.decorators.http import require_GET

from app.models import Item, MediaTypes, Sources
from integrations.media_identity import item_external_ids
from lists.models import CustomList, CustomListItem

_CATALOG_PAGE_SIZE = 100
_MAX_SKIP = 1_000_000
_MAX_CONTENT_ID_LENGTH = 500
_STREMIO_TYPES = {
    "movie": (MediaTypes.MOVIE.value,),
    "series": (MediaTypes.TV.value, MediaTypes.ANIME.value),
}
_DIRECT_STREMIO_SOURCES = {
    Sources.IMDB.value,
    Sources.TMDB.value,
    Sources.TVDB.value,
}


def _not_found(message: str) -> NoReturn:
    """Raise one non-enumerating protocol 404 with a stable message."""
    raise Http404(message)


def _public_json(payload, *, status=HTTP.OK):
    """Return one cacheable public protocol response with CORS enabled."""
    response = JsonResponse(payload, status=status)
    response["Access-Control-Allow-Origin"] = "*"
    response["Cache-Control"] = "public, max-age=60, stale-while-revalidate=300"
    return response


def _public_list(list_reference):
    """Return one explicitly public list or raise a non-enumerating 404."""
    custom_list = CustomList.objects.get_public_list(list_reference)
    if custom_list is None:
        _not_found("List not found")
    return custom_list


def _stremio_type(item):
    """Return the Stremio content type for one supported stored Item."""
    if item.media_type == MediaTypes.MOVIE.value:
        return "movie"
    if item.media_type in {MediaTypes.TV.value, MediaTypes.ANIME.value}:
        return "series"
    return None


def _content_id(item):
    """Return one exact Stremio/Nuvio-compatible identifier without guessing."""
    ids = item_external_ids(item)
    imdb_id = ids.get("imdb")
    if imdb_id:
        return imdb_id
    for namespace in ("tmdb", "tvdb"):
        value = ids.get(namespace)
        if value:
            return f"{namespace}:{value}"
    return None


def _has_exact_identity_filter():
    """Return a portable DB predicate for items that can expose an exact ID."""
    return (
        Q(item__source__in=_DIRECT_STREMIO_SOURCES)
        | Q(item__provider_external_ids__has_key="imdb_id")
        | Q(item__provider_external_ids__has_key="tmdb_id")
        | Q(item__provider_external_ids__has_key="tvdb_id")
    )


def _catalog_queryset(custom_list, stremio_type):
    """Return a deterministic, bounded-slice-ready queryset for one catalog type."""
    media_types = _STREMIO_TYPES.get(stremio_type)
    if not media_types:
        return CustomListItem.objects.none()
    return (
        CustomListItem.objects.filter(
            custom_list=custom_list,
            item__media_type__in=media_types,
        )
        .filter(_has_exact_identity_filter())
        .select_related("item")
        .order_by("list_item_id", "id")
    )


def _eligible_count(custom_list, stremio_type):
    """Return all stored items for a supported catalog type, including unresolved rows."""
    media_types = _STREMIO_TYPES.get(stremio_type)
    if not media_types:
        return 0
    return CustomListItem.objects.filter(
        custom_list=custom_list,
        item__media_type__in=media_types,
    ).count()


def _parse_skip(request, extra_args=""):
    """Parse the Stremio `skip` extra with a hard upper bound."""
    raw_skip = request.GET.get("skip")
    if raw_skip is None and extra_args:
        values = parse_qs(extra_args, keep_blank_values=True).get("skip") or []
        raw_skip = values[-1] if values else None
    if raw_skip in (None, ""):
        return 0
    try:
        skip = int(raw_skip)
    except (TypeError, ValueError):
        return None
    if skip < 0 or skip > _MAX_SKIP:
        return None
    return skip


def _meta_preview(item):
    """Serialize only public display fields required by catalog consumers."""
    content_id = _content_id(item)
    content_type = _stremio_type(item)
    if not content_id or not content_type:
        return None

    meta = {
        "id": content_id,
        "type": content_type,
        "name": item.title,
    }
    image = str(item.image or "").strip()
    if image.startswith(("https://", "http://")):
        meta["poster"] = image
    if item.release_datetime:
        meta["releaseInfo"] = str(item.release_datetime.year)
    return meta


def _parse_content_id(content_id):
    """Return (namespace, value) for one exact Stremio/Nuvio identifier."""
    value = str(content_id or "").strip()
    if not value or len(value) > _MAX_CONTENT_ID_LENGTH:
        return None
    if value.lower().startswith("tt") and ":" not in value:
        return "imdb", value
    namespace, separator, identifier = value.partition(":")
    if separator and namespace.lower() in {"imdb", "tmdb", "tvdb"} and identifier:
        return namespace.lower(), identifier.split(":", 1)[0]
    return None


def _matching_list_item(custom_list, stremio_type, content_id):
    """Resolve an exact catalog ID inside the selected public list only."""
    parsed = _parse_content_id(content_id)
    media_types = _STREMIO_TYPES.get(stremio_type)
    if parsed is None or not media_types:
        return None
    namespace, value = parsed
    provider_field = {
        "imdb": "imdb_id",
        "tmdb": "tmdb_id",
        "tvdb": "tvdb_id",
    }[namespace]
    source = {
        "imdb": Sources.IMDB.value,
        "tmdb": Sources.TMDB.value,
        "tvdb": Sources.TVDB.value,
    }[namespace]
    item_ids = Item.objects.filter(media_type__in=media_types).filter(
        Q(**{f"provider_external_ids__{provider_field}": value})
        | Q(source=source, media_id=value)
    ).values("id")
    return (
        CustomListItem.objects.filter(custom_list=custom_list, item_id__in=item_ids)
        .select_related("item")
        .order_by("list_item_id", "id")
        .first()
    )


@login_not_required
@require_GET
def manifest(request, list_reference):
    """Return a minimal read-only addon manifest for one public list."""
    custom_list = _public_list(list_reference)
    available_types = []
    catalogs = []
    for stremio_type in _STREMIO_TYPES:
        if _eligible_count(custom_list, stremio_type):
            available_types.append(stremio_type)
            catalogs.append(
                {
                    "type": stremio_type,
                    "id": f"floppy-list-{custom_list.pk}",
                    "name": custom_list.name,
                    "extra": [{"name": "skip", "isRequired": False}],
                }
            )

    if not available_types:
        # A valid addon still needs one resource/type. Keeping the manifest empty
        # would make installation look successful but unusable, so fail clearly.
        _not_found("List has no supported catalog items")

    return _public_json(
        {
            "id": f"dev.floppy.list.{custom_list.pk}",
            "version": "1.0.0",
            "name": f"Floppy: {custom_list.name}",
            "description": custom_list.description or "Read-only public list from Floppy",
            "types": available_types,
            "resources": ["catalog", "meta"],
            "catalogs": catalogs,
        }
    )


@login_not_required
@require_GET
def catalog(request, list_reference, media_type, catalog_id, extra_args=""):
    """Return one deterministic page from a public Floppy list."""
    custom_list = _public_list(list_reference)
    if catalog_id != f"floppy-list-{custom_list.pk}" or media_type not in _STREMIO_TYPES:
        _not_found("Catalog not found")

    skip = _parse_skip(request, extra_args)
    if skip is None:
        return _public_json({"error": "Invalid skip."}, status=HTTP.BAD_REQUEST)

    queryset = _catalog_queryset(custom_list, media_type)
    total_resolved = queryset.count()
    eligible = _eligible_count(custom_list, media_type)
    rows = list(queryset[skip : skip + _CATALOG_PAGE_SIZE])
    metas = []
    for row in rows:
        meta = _meta_preview(row.item)
        if meta is not None:
            metas.append(meta)

    return _public_json(
        {
            "metas": metas,
            "floppy": {
                "skip": skip,
                "limit": _CATALOG_PAGE_SIZE,
                "total_resolved": total_resolved,
                "skipped_unresolved": max(eligible - total_resolved, 0),
            },
        }
    )


@login_not_required
@require_GET
def meta(request, list_reference, media_type, content_id):
    """Return stored metadata for one exact item in the selected public list."""
    custom_list = _public_list(list_reference)
    row = _matching_list_item(custom_list, media_type, content_id)
    if row is None:
        _not_found("Metadata not found")
    payload = _meta_preview(row.item)
    if payload is None:
        _not_found("Metadata not found")
    return _public_json({"meta": payload})
