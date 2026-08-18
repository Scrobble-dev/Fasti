"""Durable saved-library membership for third-party synchronization."""

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


class SavedMediaMembership(models.Model):
    """Record that a user explicitly saved one media item for later."""

    user = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.CASCADE,
        related_name="saved_media_memberships",
    )
    item = models.ForeignKey(
        Item,
        on_delete=models.CASCADE,
        related_name="saved_media_memberships",
    )
    created_at = models.DateTimeField(auto_now_add=True)
    updated_at = models.DateTimeField(auto_now=True)

    class Meta:
        """Model options for saved-media membership."""

        ordering = ["-updated_at", "-id"]
        constraints = [
            UniqueConstraint(
                fields=["user", "item"],
                name="savedmedia_unique_user_item",
            ),
        ]
        indexes = [
            models.Index(fields=["user", "-updated_at"], name="savedmedia_user_updated_idx"),
        ]

    def __str__(self):
        """Return a compact saved-media label."""
        return f"{self.user_id}:{self.item_id}"


class SavedMediaChange(models.Model):
    """Append-only ordered change record for saved-library membership."""

    class Operation(models.TextChoices):
        """Supported saved-library change operations."""

        ADD = "add", "Add"
        REMOVE = "remove", "Remove"

    sequence_id = models.BigAutoField(primary_key=True)
    user = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.CASCADE,
        related_name="saved_media_changes",
    )
    item_id = models.PositiveBigIntegerField()
    operation = models.CharField(max_length=6, choices=Operation.choices)
    media_type = models.CharField(max_length=32)
    source = models.CharField(max_length=32)
    media_id = models.CharField(max_length=500)
    ids = models.JSONField(default=dict, blank=True)
    source_client_id = models.CharField(max_length=64, blank=True, default="")
    created_at = models.DateTimeField(auto_now_add=True)

    class Meta:
        """Model options for the saved-library change feed."""

        ordering = ["sequence_id"]
        indexes = [
            models.Index(fields=["user", "sequence_id"], name="savedchg_user_seq_idx"),
            models.Index(fields=["user", "created_at"], name="savedchg_user_created_idx"),
            models.Index(fields=["created_at"], name="savedchg_created_idx"),
        ]

    def __str__(self):
        """Return a compact saved-media change label."""
        return f"{self.sequence_id}:{self.user_id}:{self.operation}:{self.item_id}"


def _item_identity(item: Item) -> dict[str, str]:
    """Return stable external IDs that survive a later Item deletion."""
    ids = {}
    stored = item.provider_external_ids or {}
    for field, namespace in _PROVIDER_ID_FIELDS.items():
        value = stored.get(field)
        if isinstance(value, (str, int)) and str(value):
            ids[namespace] = str(value)

    if item.source in _DIRECT_ID_SOURCES and item.media_id:
        ids.setdefault(item.source, str(item.media_id))
    return ids


def _record_saved_media_change(membership: SavedMediaMembership, operation: str) -> None:
    """Append one identity snapshot for an explicit membership mutation."""
    item = (
        Item.objects.filter(pk=membership.item_id)
        .only("media_type", "source", "media_id", "provider_external_ids")
        .first()
    )
    if item is None:
        return

    SavedMediaChange.objects.create(
        user_id=membership.user_id,
        item_id=membership.item_id,
        operation=operation,
        media_type=item.media_type,
        source=item.source,
        media_id=str(item.media_id),
        ids=_item_identity(item),
        source_client_id=get_playback_source_client_id(),
    )


@receiver(
    post_save,
    sender=SavedMediaMembership,
    dispatch_uid="saved_media_append_add_change",
)
def _append_saved_media_add(sender, instance, created, **_kwargs):
    """Record one add event when membership is first created."""
    if created:
        _record_saved_media_change(instance, SavedMediaChange.Operation.ADD)


@receiver(
    pre_delete,
    sender=SavedMediaMembership,
    dispatch_uid="saved_media_append_remove_change",
)
def _append_saved_media_remove(sender, instance, origin, **_kwargs):
    """Record explicit removals without manufacturing parent-delete events."""
    explicit_membership_delete = isinstance(origin, SavedMediaMembership) or (
        getattr(origin, "model", None) is SavedMediaMembership
    )
    if explicit_membership_delete:
        _record_saved_media_change(instance, SavedMediaChange.Operation.REMOVE)
