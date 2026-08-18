# FORK: security and data-integrity coverage for durable playback writes.
from http import HTTPStatus as HTTP  # noqa: N814
from unittest.mock import patch

from app.models import Item, MediaTypes, PlaybackProgress, Sources
from integrations.media_identity import MAX_EXTERNAL_ID_LENGTH

from .base import FloppyApiTestCase


class PlaybackProgressInputBoundaryTests(FloppyApiTestCase):
    """Reject malformed integration input before it reaches storage or providers."""

    def _put(self, payload):
        return self.call_api(
            "put",
            "api_playback_progress",
            payload=payload,
            headers=self.auth_headers,
        )

    def test_non_object_body_is_rejected(self):
        """A JSON array cannot enter object-oriented validation paths."""
        response = self._put([{"media_type": "movie"}])

        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.json()["detail"], "Request body must be a JSON object.")

    def test_non_object_ids_are_rejected(self):
        """Nested identifier input must be a JSON object."""
        response = self._put(
            {
                "media_type": "movie",
                "ids": ["701"],
                "position_seconds": 10,
            }
        )

        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.json()["detail"], "'ids' must be an object.")

    def test_boolean_and_oversized_seconds_are_rejected(self):
        """Boolean coercion and values above the portable DB range fail closed."""
        for value in (True, 2_147_483_648):
            with self.subTest(value=value):
                response = self._put(
                    {
                        "media_type": "movie",
                        "ids": {"tmdb": "701"},
                        "position_seconds": value,
                    }
                )
                self.assertEqual(response.status_code, HTTP.BAD_REQUEST)

    def test_episode_coordinates_are_bounded_without_rejecting_specials(self):
        """Season zero is valid, but invalid coordinate types and ranges are not."""
        Item.objects.create(
            media_id="1001",
            source=Sources.TMDB.value,
            media_type=MediaTypes.EPISODE.value,
            title="TV Show 1 Special",
            season_number=0,
            episode_number=1,
        )
        valid_special = self._put(
            {
                "media_type": "episode",
                "ids": {"tmdb": "1001"},
                "season_number": 0,
                "episode_number": 1,
                "position_seconds": 10,
            }
        )
        self.assertEqual(valid_special.status_code, HTTP.OK)

        cases = (
            {"season_number": -1, "episode_number": 1},
            {"season_number": True, "episode_number": 1},
            {"season_number": 1, "episode_number": 0},
            {"season_number": 1, "episode_number": False},
        )
        for coordinates in cases:
            with self.subTest(coordinates=coordinates):
                response = self._put(
                    {
                        "media_type": "episode",
                        "ids": {"tmdb": "1001"},
                        **coordinates,
                        "position_seconds": 10,
                    }
                )
                self.assertEqual(response.status_code, HTTP.BAD_REQUEST)

    def test_completed_requires_json_boolean(self):
        """String truthiness cannot silently mark media complete."""
        response = self._put(
            {
                "media_type": "movie",
                "ids": {"tmdb": "701"},
                "position_seconds": 10,
                "completed": "false",
            }
        )

        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)

    def test_oversized_provider_id_stops_before_network_resolution(self):
        """Bound identifier strings before any provider lookup."""
        with patch("api.fork_views_playback.app.providers.tmdb.find") as provider_find:
            response = self._put(
                {
                    "media_type": "movie",
                    "ids": {"imdb": "x" * (MAX_EXTERNAL_ID_LENGTH + 1)},
                    "position_seconds": 10,
                }
            )

        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        provider_find.assert_not_called()


class PlaybackProgressIdentityConsistencyTests(FloppyApiTestCase):
    """Require all supplied exact identifiers to name the same durable target."""

    def _put(self, ids):
        return self.call_api(
            "put",
            "api_playback_progress",
            payload={
                "media_type": "movie",
                "ids": ids,
                "position_seconds": 120,
            },
            headers=self.auth_headers,
        )

    def test_matching_local_aliases_are_accepted_without_provider_lookup(self):
        """A locally verified alias keeps exact progress writes offline-capable."""
        item = self.items_by_type[MediaTypes.MOVIE.value][0]
        item.provider_external_ids = {"imdb_id": "tt0701"}
        item.save(update_fields=["provider_external_ids"])

        with patch("api.fork_views_playback.app.providers.tmdb.find") as provider_find:
            response = self._put({"tmdb": "701", "imdb": "tt0701"})

        self.assertEqual(response.status_code, HTTP.OK)
        provider_find.assert_not_called()
        self.assertTrue(
            PlaybackProgress.objects.filter(user=self.user1, item=item).exists()
        )

    def test_conflicting_local_aliases_cannot_write_progress(self):
        """A valid TMDB id cannot override a conflicting exact IMDb alias."""
        first = self.items_by_type[MediaTypes.MOVIE.value][0]
        second = self.items_by_type[MediaTypes.MOVIE.value][1]
        second.provider_external_ids = {"imdb_id": "tt0702"}
        second.save(update_fields=["provider_external_ids"])

        response = self._put({"tmdb": "701", "imdb": "tt0702"})

        self.assertEqual(response.status_code, HTTP.NOT_FOUND)
        self.assertFalse(
            PlaybackProgress.objects.filter(user=self.user1, item=first).exists()
        )
        self.assertFalse(
            PlaybackProgress.objects.filter(user=self.user1, item=second).exists()
        )

    def test_conflicting_local_aliases_cannot_clear_progress(self):
        """A conflicting alias cannot delete the progress named by another identifier."""
        first = self.items_by_type[MediaTypes.MOVIE.value][0]
        second = self.items_by_type[MediaTypes.MOVIE.value][1]
        second.provider_external_ids = {"imdb_id": "tt0702"}
        second.save(update_fields=["provider_external_ids"])
        progress = PlaybackProgress.objects.create(
            user=self.user1,
            item=first,
            position_seconds=500,
        )

        response = self.call_api(
            "delete",
            "api_playback_progress",
            payload={
                "media_type": "movie",
                "ids": {"tmdb": "701", "imdb": "tt0702"},
            },
            headers=self.auth_headers,
        )

        self.assertEqual(response.status_code, HTTP.NOT_FOUND)
        self.assertTrue(PlaybackProgress.objects.filter(pk=progress.pk).exists())

    def test_direct_tmdb_identity_is_returned_to_client(self):
        """Direct provider identity remains round-trippable in the public payload."""
        response = self._put({"tmdb": "701"})

        self.assertEqual(response.status_code, HTTP.OK)
        self.assertEqual(response.json()["ids"]["tmdb"], "701")

    def test_resolver_exception_text_is_not_returned(self):
        """Unexpected provider errors keep a stable public response."""
        marker = "provider-secret-marker"
        with patch(
            "api.fork_views_playback.resolve_video_item",
            side_effect=RuntimeError(marker),
        ):
            response = self._put({"tmdb": "701"})

        self.assertEqual(response.status_code, HTTP.NOT_FOUND)
        self.assertNotIn(marker, response.content.decode("utf-8"))
