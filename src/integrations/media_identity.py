"""Exact media identity helpers shared by external integration contracts."""

from dataclasses import dataclass

from django.db.models import Q

from app.models import Item, Sources

EXTERNAL_ID_FIELDS = {
    "tmdb": "tmdb_id",
    "imdb": "imdb_id",
    "tvdb": "tvdb_id",
    "mal": "mal_id",
    "anilist": "anilist_id",
    "kitsu": "kitsu_id",
}
DIRECT_ID_SOURCES = {
    "tmdb": Sources.TMDB.value,
    "imdb": Sources.IMDB.value,
    "tvdb": Sources.TVDB.value,
    "mal": Sources.MAL.value,
}
MAX_EXTERNAL_ID_LENGTH = 500


@dataclass(frozen=True, slots=True)
class ExternalIdentityError(ValueError):
    """Stable validation detail for untrusted external media identity."""

    code: str
    param: str
    message: str


def _identity_error(code: str, param: str, message: str) -> ExternalIdentityError:
    """Build one stable validation error without repeating constructor details."""
    return ExternalIdentityError(code, param, message)


def normalize_external_ids(raw_ids) -> dict[str, str]:
    """Normalize bounded provider IDs without accepting nested or guessed identity."""
    if not isinstance(raw_ids, dict):
        message = "ids must be an object with at least one supported provider identifier."
        error = _identity_error("missing_media_identity", "ids", message)
        raise error

    normalized = {}
    for namespace in EXTERNAL_ID_FIELDS:
        value = raw_ids.get(namespace)
        if value is None or value == "":
            continue
        if isinstance(value, bool) or not isinstance(value, (str, int)):
            message = f"ids.{namespace} must be a string or integer."
            error = _identity_error(
                "invalid_media_identity",
                f"ids.{namespace}",
                message,
            )
            raise error
        value = str(value).strip()
        if not value:
            message = f"ids.{namespace} must not be empty."
            error = _identity_error(
                "invalid_media_identity",
                f"ids.{namespace}",
                message,
            )
            raise error
        if len(value) > MAX_EXTERNAL_ID_LENGTH:
            message = (
                f"ids.{namespace} must be {MAX_EXTERNAL_ID_LENGTH} characters or fewer."
            )
            error = _identity_error(
                "invalid_media_identity",
                f"ids.{namespace}",
                message,
            )
            raise error
        normalized[namespace] = value

    if not normalized:
        message = "ids must contain at least one supported provider identifier."
        error = _identity_error("missing_media_identity", "ids", message)
        raise error
    return normalized


def item_external_ids(item: Item) -> dict[str, str]:
    """Return the exact public provider aliases stored for one Item."""
    result = {}
    stored = item.provider_external_ids or {}
    for namespace, field in EXTERNAL_ID_FIELDS.items():
        value = stored.get(field)
        if isinstance(value, (str, int)) and not isinstance(value, bool) and str(value):
            result[namespace] = str(value)
    for namespace, source in DIRECT_ID_SOURCES.items():
        if item.source == source and item.media_id:
            result.setdefault(namespace, str(item.media_id))
    return result


def identity_filter(namespace: str, value: str) -> Q:
    """Return the exact persisted predicate for one supported provider alias."""
    field = EXTERNAL_ID_FIELDS[namespace]
    predicate = Q(**{f"provider_external_ids__{field}": value})
    source = DIRECT_ID_SOURCES.get(namespace)
    if source:
        predicate |= Q(source=source, media_id=value)
    return predicate


def find_item_by_exact_ids(media_type: str, ids: dict[str, str]) -> Item | None:
    """Return one Item only when every supplied exact alias resolves to that row."""
    queryset = Item.objects.filter(media_type=media_type)
    for namespace, value in ids.items():
        queryset = queryset.filter(identity_filter(namespace, value))
    return queryset.order_by("id").first()


def item_matches_ids(item: Item, ids: dict[str, str]) -> bool:
    """Return True only when every supplied alias agrees with one Item."""
    actual_ids = item_external_ids(item)
    return all(actual_ids.get(namespace) == value for namespace, value in ids.items())
