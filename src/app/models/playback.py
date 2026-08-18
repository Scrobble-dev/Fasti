from django.conf import settings
from django.db import models
from django.db.models import UniqueConstraint
from django.db.models.signals import post_save, pre_delete
from django.dispatch import receiver

from app.models.item import Item
from app.playback_context import get_playback_source_client_id

_PROVIDER_ID_FIELDS = {
    "tmdb_id": "tmdb",
    "imdb_id": "imdb",
    "tvdb_id": "tvdb",
    "mal_id": "mal",
    "anilist_id": "anilist",
    "kitsu_id": "kitsu",
}
_DIRECT_ID_SOURCES = frozenset(_PROVIDER_ID_FIELDS.values())


class PlaybackProgress(models.Model):
    """Durable second-level resume position for a movie or episode.

    ``Media.progress`` can't carry this: its unit varies per media type
    (episodes for TV, 1 for movies, pages for books, minutes for podcasts),
    and ``live_playback`` only keeps a cache entry for the user's *current*
    item. This is the durable, per-item store third-party clients read and
    write for bidirectional resume sync.

    Keyed on Item rather than Movie/Episode because Episode is not a Media
    subclass — Item is the only uniform key across both. Podcast positions
    stay on ``Podcast.played_up_to_seconds``, which the podcast UI reads.
    """

    user = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.CASCADE,
        related_name="playback_progress",
    )
    item = models.ForeignKey(
        Item,
        on_delete=models.CASCADE,
        related_name="playback_progress",
    )
    position_seconds = models.PositiveIntegerField()
    duration_seconds = models.PositiveIntegerField(null=True, blank=True)
    completed = models.BooleanField(default=False)
    updated_at = models.DateTimeField(auto_now=True)

    class Meta:
        """Meta options for PlaybackProgress."""

        ordering = ["-updated_at"]
        verbose_name_plural = "Playback progress"
        constraints = [
            UniqueConstraint(
                fields=["user", "item"],
                name="playbackprogress_unique_user_item",
            ),
        ]
        indexes = [
            models.Index(fields=["user", "-updated_at"]),
        ]

    def __str__(self):
        """Return a readable position label."""
        return f"{self.user_id}:{self.item_id}@{self.position_seconds}s"


class PlaybackProgressChange(models.Model):
    """Append-only ordered change record for movie and episode resume state."""

    class Operation(models.TextChoices):
        """Supported durable progress change operations."""

        UPSERT = "upsert", "Upsert"
        DELETE = "delete", "Delete"

    sequence_id = models.BigAutoField(primary_key=True)
    user = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.CASCADE,
        related_name="playback_progress_changes",
    )
    item_id = models.PositiveBigIntegerField()
    operation = models.CharField(max_length=6, choices=Operation.choices)
    media_type = models.CharField(max_length=32)
    source = models.CharField(max_length=32)
    media_id = models.CharField(max_length=500)
    season_number = models.PositiveIntegerField(null=True, blank=True)
    episode_number = models.PositiveIntegerField(null=True, blank=True)
    ids = models.JSONField(default=dict, blank=True)
    position_seconds = models.PositiveIntegerField(null=True, blank=True)
    duration_seconds = models.PositiveIntegerField(null=True, blank=True)
    completed = models.BooleanField(default=False)
    source_client_id = models.CharField(max_length=64, blank=True, default="")
    created_at = models.DateTimeField(auto_now_add=True)

    class Meta:
        """Meta options for the ordered playback progress change feed."""

        ordering = ["sequence_id"]
        indexes = [
            models.Index(
                fields=["user", "sequence_id"],
                name="playchg_user_seq_idx",
            ),
            models.Index(
                fields=["user", "created_at"],
                name="playchg_user_created_idx",
            ),
            models.Index(fields=["created_at"], name="playchg_created_idx"),
        ]

    def __str__(self):
        """Return a compact ordered change label."""
        return f"{self.sequence_id}:{self.user_id}:{self.operation}:{self.item_id}"


def _item_identity(item: Item) -> dict[str, str]:
    """Return stable external IDs that can survive later Item deletion."""
    ids = {}
    stored = item.provider_external_ids or {}
    for field, namespace in _PROVIDER_ID_FIELDS.items():
        value = stored.get(field)
        if isinstance(value, (str, int)) and str(value):
            ids[namespace] = str(value)

    if item.source in _DIRECT_ID_SOURCES and item.media_id:
        ids.setdefault(item.source, str(item.media_id))
    return ids


def _record_playback_change(progress: PlaybackProgress, operation: str) -> None:
    """Append one ordered identity snapshot for a progress mutation."""
    item = (
        Item.objects.filter(pk=progress.item_id)
        .only(
            "media_type",
            "source",
            "media_id",
            "season_number",
            "episode_number",
            "provider_external_ids",
        )
        .first()
    )
    if item is None:
        return

    is_delete = operation == PlaybackProgressChange.Operation.DELETE
    PlaybackProgressChange.objects.create(
        user_id=progress.user_id,
        item_id=progress.item_id,
        operation=operation,
        media_type=item.media_type,
        source=item.source,
        media_id=str(item.media_id),
        season_number=item.season_number,
        episode_number=item.episode_number,
        ids=_item_identity(item),
        position_seconds=None if is_delete else progress.position_seconds,
        duration_seconds=None if is_delete else progress.duration_seconds,
        completed=False if is_delete else progress.completed,
        source_client_id=get_playback_source_client_id(),
    )


@receiver(
    post_save,
    sender=PlaybackProgress,
    dispatch_uid="playback_progress_append_upsert_change",
)
def _append_progress_upsert(sender, instance, **_kwargs):
    """Record every current-state save as an ordered progress upsert."""
    _record_playback_change(instance, PlaybackProgressChange.Operation.UPSERT)


@receiver(
    pre_delete,
    sender=PlaybackProgress,
    dispatch_uid="playback_progress_append_delete_change",
)
def _append_progress_delete(sender, instance, origin, **_kwargs):
    """Record explicit progress clears without manufacturing parent-delete events."""
    explicit_progress_delete = isinstance(origin, PlaybackProgress) or (
        getattr(origin, "model", None) is PlaybackProgress
    )
    if not explicit_progress_delete:
        return
    _record_playback_change(instance, PlaybackProgressChange.Operation.DELETE)
