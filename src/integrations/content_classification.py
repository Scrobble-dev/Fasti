"""Source-neutral content classification shared by external media adapters."""

from dataclasses import dataclass
from enum import StrEnum

from app.models import MediaTypes


class ExternalContentClass(StrEnum):
    """Classifier vocabulary shared with Stremio/Nuvio protocol surfaces."""

    MOVIE = "movie"
    SERIES = "series"
    CHANNEL = "channel"
    TV = "tv"
    UNKNOWN = "unknown"


@dataclass(frozen=True, slots=True)
class ClassifiedContentType:
    """Keep a known classifier and the raw provider value together."""

    classification: ExternalContentClass
    raw_type: str

    @property
    def api_type(self) -> str:
        """Return the canonical type or the original non-empty unknown value."""
        if self.classification is ExternalContentClass.UNKNOWN:
            return self.raw_type or ExternalContentClass.MOVIE.value
        return self.classification.value


def classify_external_content_type(raw_type: str | None) -> ClassifiedContentType:
    """Classify one Nuvio/Stremio type without guessing unknown vocabulary."""
    normalized = str(raw_type or "").strip().lower()
    try:
        classification = ExternalContentClass(normalized)
    except ValueError:
        classification = ExternalContentClass.UNKNOWN
    return ClassifiedContentType(
        classification=classification,
        raw_type=normalized,
    )


def floppy_catalog_content_type(media_type: str | None) -> ExternalContentClass | None:
    """Return the protocol catalog class for a top-level Floppy media type."""
    if media_type == MediaTypes.MOVIE.value:
        return ExternalContentClass.MOVIE
    if media_type in {MediaTypes.TV.value, MediaTypes.ANIME.value}:
        return ExternalContentClass.SERIES
    return None


def floppy_catalog_media_types(
    classification: ExternalContentClass | str,
) -> tuple[str, ...]:
    """Return the Floppy media buckets represented by one catalog class."""
    try:
        classification = ExternalContentClass(classification)
    except ValueError:
        return ()
    if classification is ExternalContentClass.MOVIE:
        return (MediaTypes.MOVIE.value,)
    if classification is ExternalContentClass.SERIES:
        return (MediaTypes.TV.value, MediaTypes.ANIME.value)
    return ()
