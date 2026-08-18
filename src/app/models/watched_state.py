"""Current watched-state projection for third-party synchronization."""

from django.conf import settings
from django.db import models, transaction
from django.db.models import Count, Max, UniqueConstraint
from django.db.models.signals import post_delete, post_save, pre_save
from django.dispatch import receiver

from app.models.item import Item
from app.models.media import Movie, MoviePlay
from app.models.tv import Episode
from app.models.utils import MediaTypes, Status
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
_SHOW_MEDIA_TYPES = (MediaTypes.TV.value, MediaTypes.ANIME.value)


class WatchedState(models.Model):
    """Store the user's current watched bit separately from watch history."""

    user = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.CASCADE,
        related_name="watched_states",
    )
    item = models.ForeignKey(
        Item,
        on_delete=models.CASCADE,
        related_name="watched_states",
    )
    watched = models.BooleanField(default=False)
    watched_at = models.DateTimeField(null=True, blank=True)
    source_client_id = models.CharField(max_length=64, blank=True, default="")
    updated_at = models.DateTimeField(auto_now=True)

    class Meta:
        """Model options for current watched state."""

        constraints = [
            UniqueConstraint(
                fields=["user", "item"],
                name="watchedstate_unique_user_item",
            ),
        ]
        indexes = [
            models.Index(fields=["user", "id"], name="watchstate_user_id_idx"),
            models.Index(
                fields=["user", "-updated_at"],
                name="watchstate_user_updated_idx",
            ),
        ]

    def __str__(self):
        """Return a compact current-state label."""
        return f"{self.user_id}:{self.item_id}:{'watched' if self.watched else 'unwatched'}"


class WatchedStateChange(models.Model):
    """Append-only ordered mutation record for current watched state."""

    sequence_id = models.BigAutoField(primary_key=True)
    user = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.CASCADE,
        related_name="watched_state_changes",
    )
    item_id = models.PositiveBigIntegerField()
    watched = models.BooleanField()
    media_type = models.CharField(max_length=32)
    source = models.CharField(max_length=32)
    media_id = models.CharField(max_length=500)
    season_number = models.PositiveIntegerField(null=True, blank=True)
    episode_number = models.PositiveIntegerField(null=True, blank=True)
    ids = models.JSONField(default=dict, blank=True)
    watched_at = models.DateTimeField(null=True, blank=True)
    source_client_id = models.CharField(max_length=64, blank=True, default="")
    created_at = models.DateTimeField(auto_now_add=True)

    class Meta:
        """Model options for the watched-state change feed."""

        ordering = ["sequence_id"]
        indexes = [
            models.Index(fields=["user", "sequence_id"], name="watchchg_user_seq_idx"),
            models.Index(
                fields=["user", "created_at"],
                name="watchchg_user_created_idx",
            ),
            models.Index(fields=["created_at"], name="watchchg_created_idx"),
        ]

    def __str__(self):
        """Return a compact ordered change label."""
        return f"{self.sequence_id}:{self.user_id}:{self.item_id}:{self.watched}"


def _ids_from_item(item: Item | None) -> dict[str, str]:
    """Return stable exact identifiers for one item."""
    if item is None:
        return {}

    ids = {}
    stored = item.provider_external_ids or {}
    for field, namespace in _PROVIDER_ID_FIELDS.items():
        value = stored.get(field)
        if isinstance(value, (str, int)) and str(value):
            ids[namespace] = str(value)

    if item.source in _DIRECT_ID_SOURCES and item.media_id:
        ids.setdefault(item.source, str(item.media_id))
    return ids


def watched_identity(item: Item) -> dict[str, str]:
    """Return the exact client identity, using show-level IDs for episodes."""
    ids = _ids_from_item(item)
    if item.media_type != MediaTypes.EPISODE.value:
        return ids

    show = (
        Item.objects.filter(
            source=item.source,
            media_id=item.media_id,
            media_type__in=_SHOW_MEDIA_TYPES,
        )
        .order_by("id")
        .first()
    )
    show_ids = _ids_from_item(show)
    show_ids.update(ids)
    return show_ids


def _append_change(state: WatchedState) -> WatchedStateChange:
    """Append one ordered current-state mutation with an identity snapshot."""
    item = state.item
    return WatchedStateChange.objects.create(
        user_id=state.user_id,
        item_id=state.item_id,
        watched=state.watched,
        media_type=item.media_type,
        source=item.source,
        media_id=str(item.media_id),
        season_number=item.season_number,
        episode_number=item.episode_number,
        ids=watched_identity(item),
        watched_at=state.watched_at if state.watched else None,
        source_client_id=state.source_client_id,
    )


def set_watched_state(
    *,
    user_id: int,
    item: Item,
    watched: bool,
    watched_at=None,
    source_client_id: str | None = None,
) -> tuple[WatchedState, bool]:
    """Set current watched state without creating or deleting history occurrences."""
    source_client_id = (
        get_playback_source_client_id()
        if source_client_id is None
        else source_client_id
    )
    if not watched:
        watched_at = None

    with transaction.atomic():
        state, created = WatchedState.objects.select_for_update().get_or_create(
            user_id=user_id,
            item=item,
            defaults={
                "watched": watched,
                "watched_at": watched_at,
                "source_client_id": source_client_id,
            },
        )
        if created:
            _append_change(state)
            return state, True

        effective_watched_at = watched_at
        if watched and effective_watched_at is None and state.watched:
            effective_watched_at = state.watched_at

        changed = (
            state.watched != watched
            or state.watched_at != effective_watched_at
        )
        if not changed:
            return state, False

        state.watched = watched
        state.watched_at = effective_watched_at
        state.source_client_id = source_client_id
        state.save(
            update_fields=[
                "watched",
                "watched_at",
                "source_client_id",
                "updated_at",
            ]
        )
        _append_change(state)
        return state, True


def sync_movie_watched_state(movie: Movie) -> tuple[WatchedState, bool]:
    """Project current movie state from explicit movie play occurrences."""
    stats = movie.plays.aggregate(
        occurrence_count=Count("id"),
        watched_at=Max("end_date"),
    )
    return set_watched_state(
        user_id=movie.user_id,
        item=movie.item,
        watched=bool(stats["occurrence_count"]),
        watched_at=stats["watched_at"],
    )


def sync_episode_watched_state(episode: Episode) -> tuple[WatchedState, bool]:
    """Project one episode's current state from exact completed occurrences."""
    stats = Episode.objects.filter(
        related_season__user_id=episode.related_season.user_id,
        item_id=episode.item_id,
        status=Status.COMPLETED.value,
    ).aggregate(
        occurrence_count=Count("id"),
        watched_at=Max("end_date"),
    )
    return set_watched_state(
        user_id=episode.related_season.user_id,
        item=episode.item,
        watched=bool(stats["occurrence_count"]),
        watched_at=stats["watched_at"],
    )


def sync_bulk_episode_watched_states(episodes) -> None:
    """Project exact Episode rows created by a native bulk history operation."""
    seen = set()
    for episode in episodes:
        if not episode.item_id or not episode.related_season_id:
            continue
        key = (episode.related_season.user_id, episode.item_id)
        if key in seen:
            continue
        seen.add(key)
        sync_episode_watched_state(episode)


@receiver(
    pre_save,
    sender=Movie,
    dispatch_uid="watched_state_capture_movie_completion",
)
def _capture_movie_completion(sender, instance, **_kwargs):
    """Remember whether a direct movie save is becoming completed."""
    instance._watched_state_became_completed = (
        instance.status == Status.COMPLETED.value
        and (instance._state.adding or instance.tracker.has_changed("status"))
    )


@receiver(
    post_save,
    sender=Movie,
    dispatch_uid="watched_state_project_direct_movie_completion",
)
def _project_direct_movie_completion(sender, instance, **_kwargs):
    """Mirror an explicit movie completion without creating a history occurrence."""
    if getattr(instance, "_watched_state_became_completed", False):
        set_watched_state(
            user_id=instance.user_id,
            item=instance.item,
            watched=True,
            watched_at=instance.end_date,
        )


@receiver(
    post_save,
    sender=MoviePlay,
    dispatch_uid="watched_state_project_movie_play_save",
)
def _project_movie_play_save(sender, instance, **_kwargs):
    """Recalculate current movie state after a history occurrence is saved."""
    sync_movie_watched_state(instance.movie)


@receiver(
    post_delete,
    sender=MoviePlay,
    dispatch_uid="watched_state_project_movie_play_delete",
)
def _project_movie_play_delete(sender, instance, **_kwargs):
    """Recalculate current movie state after a history occurrence is removed."""
    sync_movie_watched_state(instance.movie)


@receiver(
    post_save,
    sender=Episode,
    dispatch_uid="watched_state_project_episode_save",
)
def _project_episode_save(sender, instance, **_kwargs):
    """Recalculate one episode bit after an exact history occurrence is saved."""
    sync_episode_watched_state(instance)


@receiver(
    post_delete,
    sender=Episode,
    dispatch_uid="watched_state_project_episode_delete",
)
def _project_episode_delete(sender, instance, **_kwargs):
    """Recalculate one episode bit after an exact history occurrence is removed."""
    sync_episode_watched_state(instance)
