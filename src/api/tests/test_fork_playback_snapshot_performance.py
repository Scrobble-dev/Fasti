# FORK: performance regression coverage for durable playback snapshots.
from http import HTTPStatus as HTTP  # noqa: N814
from unittest.mock import patch

from django.utils import timezone

from api import fork_views_playback
from app.models import (
    Item,
    MediaTypes,
    PlaybackProgress,
    Podcast,
    PodcastEpisode,
    PodcastShow,
    Sources,
    Status,
)

from .base import FloppyApiTestCase


class PlaybackSnapshotPerformanceTests(FloppyApiTestCase):
    """Keep snapshot serialization proportional to the requested page window."""

    def _get(self, params):
        return self.call_api(
            "get",
            "api_playback_progress",
            params=params,
            headers=self.auth_headers,
        )

    def _seed_movie_positions(self):
        rows = []
        for index, item in enumerate(self.items_by_type[MediaTypes.MOVIE.value], start=1):
            rows.append(
                PlaybackProgress.objects.create(
                    user=self.user1,
                    item=item,
                    position_seconds=index * 100,
                )
            )
        return rows

    def _create_podcast(self, suffix, *, completed):
        show = PodcastShow.objects.create(
            podcast_uuid=f"snapshot-show-{suffix}",
            title=f"Snapshot Show {suffix}",
        )
        episode = PodcastEpisode.objects.create(
            show=show,
            episode_uuid=f"snapshot-episode-{suffix}",
            title=f"Snapshot Episode {suffix}",
            duration=1800,
        )
        item = Item.objects.create(
            media_id=f"snapshot-episode-{suffix}",
            source=Sources.POCKETCASTS.value,
            media_type=MediaTypes.PODCAST.value,
            title=f"Snapshot Episode {suffix}",
        )
        return Podcast.objects.create(
            user=self.user1,
            item=item,
            show=show,
            episode=episode,
            status=Status.COMPLETED.value if completed else Status.IN_PROGRESS.value,
            last_seen_status=3 if completed else 2,
            played_up_to_seconds=900,
            position_updated_at=timezone.now(),
        )

    def test_first_video_page_serializes_only_one_row(self):
        """A one-row page does not serialize every matching video position."""
        self._seed_movie_positions()

        with patch.object(
            fork_views_playback,
            "_serialize_progress",
            wraps=fork_views_playback._serialize_progress,
        ) as serializer:
            response = self._get({"media_type": "movie", "limit": 1})

        self.assertEqual(response.status_code, HTTP.OK)
        self.assertEqual(response.data["pagination"]["total"], 3)
        self.assertEqual(len(response.data["results"]), 1)
        self.assertEqual(serializer.call_count, 1)

    def test_second_video_page_serializes_only_offset_plus_limit_window(self):
        """Offset compatibility does not force full-library serialization."""
        self._seed_movie_positions()

        with patch.object(
            fork_views_playback,
            "_serialize_progress",
            wraps=fork_views_playback._serialize_progress,
        ) as serializer:
            response = self._get(
                {"media_type": "movie", "limit": 1, "offset": 1}
            )

        self.assertEqual(response.status_code, HTTP.OK)
        self.assertEqual(response.data["pagination"]["total"], 3)
        self.assertEqual(len(response.data["results"]), 1)
        self.assertEqual(serializer.call_count, 2)

    def test_podcast_completion_filter_runs_before_serialization(self):
        """A filtered podcast snapshot serializes only rows that can enter the page."""
        self._create_podcast("complete", completed=True)
        self._create_podcast("progress", completed=False)

        with patch.object(
            fork_views_playback,
            "_serialize_podcast",
            wraps=fork_views_playback._serialize_podcast,
        ) as serializer:
            response = self._get(
                {"media_type": "podcast", "completed": "true", "limit": 1}
            )

        self.assertEqual(response.status_code, HTTP.OK)
        self.assertEqual(response.data["pagination"]["total"], 1)
        self.assertEqual(len(response.data["results"]), 1)
        self.assertTrue(response.data["results"][0]["completed"])
        self.assertEqual(serializer.call_count, 1)
