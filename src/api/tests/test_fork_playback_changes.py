# FORK: regression coverage for ordered playback progress changes.
from http import HTTPStatus as HTTP  # noqa: N814
from unittest.mock import patch

from app.models import MediaTypes, PlaybackProgress, PlaybackProgressChange
from integrations.models import IntegrationToken

from .base import FloppyApiTestCase


class PlaybackProgressChangesTests(FloppyApiTestCase):
    """Verify snapshot checkpoints and ordered movie/episode progress changes."""

    def _checkpoint(self, headers=None):
        response = self.call_api(
            "get",
            "api_playback_progress_changes",
            headers=headers or self.auth_headers,
        )
        self.assertEqual(response.status_code, HTTP.OK)
        self.assertTrue(response.data["snapshot_required"])
        return response.data["next_cursor"]

    def _put_movie_progress(self, position, headers=None):
        movie_item = self.items_by_type[MediaTypes.MOVIE.value][0]
        return self.call_api(
            "put",
            "api_playback_progress",
            payload={
                "media_type": "movie",
                "ids": {"tmdb": movie_item.media_id},
                "position_seconds": position,
                "duration_seconds": 3600,
            },
            headers=headers or self.auth_headers,
        )

    def test_checkpoint_then_delta_includes_new_upsert(self):
        """A write after C0 appears in the ordered change feed."""
        cursor = self._checkpoint()
        write = self._put_movie_progress(120)
        self.assertEqual(write.status_code, HTTP.OK)

        response = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": cursor},
            headers=self.auth_headers,
        )

        self.assertEqual(response.status_code, HTTP.OK)
        self.assertFalse(response.data["snapshot_required"])
        self.assertEqual(len(response.data["results"]), 1)
        change = response.data["results"][0]
        self.assertEqual(change["operation"], "upsert")
        self.assertEqual(change["position_seconds"], 120)
        self.assertEqual(change["media_type"], MediaTypes.MOVIE.value)

    def test_explicit_clear_produces_delete_event(self):
        """Deleting a saved video position emits an explicit tombstone."""
        self.assertEqual(self._put_movie_progress(180).status_code, HTTP.OK)
        cursor = self._checkpoint()
        movie_item = self.items_by_type[MediaTypes.MOVIE.value][0]

        delete = self.call_api(
            "delete",
            "api_playback_progress",
            payload={"media_type": "movie", "ids": {"tmdb": movie_item.media_id}},
            headers=self.auth_headers,
        )
        self.assertEqual(delete.status_code, HTTP.NO_CONTENT)

        response = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": cursor},
            headers=self.auth_headers,
        )
        self.assertEqual(response.status_code, HTTP.OK)
        self.assertEqual(len(response.data["results"]), 1)
        tombstone = response.data["results"][0]
        self.assertEqual(tombstone["operation"], "delete")
        self.assertIsNone(tombstone["position_seconds"])
        self.assertFalse(tombstone["completed"])

    def test_pagination_is_monotonic_without_repeats(self):
        """Bounded pages advance through increasing committed sequence IDs."""
        cursor = self._checkpoint()
        for position in (10, 20, 30):
            self.assertEqual(self._put_movie_progress(position).status_code, HTTP.OK)

        first = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": cursor, "limit": 2},
            headers=self.auth_headers,
        )
        self.assertEqual(first.status_code, HTTP.OK)
        self.assertTrue(first.data["has_more"])
        first_ids = [row["sequence_id"] for row in first.data["results"]]
        self.assertEqual(first_ids, sorted(first_ids))
        self.assertEqual(len(set(first_ids)), 2)

        second = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": first.data["next_cursor"], "limit": 2},
            headers=self.auth_headers,
        )
        self.assertEqual(second.status_code, HTTP.OK)
        second_ids = [row["sequence_id"] for row in second.data["results"]]
        self.assertFalse(second.data["has_more"])
        self.assertEqual(len(second_ids), 1)
        self.assertGreater(second_ids[0], first_ids[-1])
        self.assertTrue(set(first_ids).isdisjoint(second_ids))

    def test_tampered_cursor_is_rejected(self):
        """A client cannot edit a signed cursor and continue polling."""
        cursor = self._checkpoint()
        response = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": f"{cursor}x"},
            headers=self.auth_headers,
        )
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "invalid_cursor")

    def test_cursor_is_bound_to_user(self):
        """One user's cursor cannot be consumed by another authenticated user."""
        cursor = self._checkpoint()
        response = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": cursor},
            headers={"HTTP_X_API_KEY": self.user2.token},
        )
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "invalid_cursor")

    def test_expired_cursor_requires_fresh_snapshot(self):
        """Expired incremental state fails explicitly instead of skipping tombstones."""
        cursor = self._checkpoint()
        with patch("api.fork_views_playback_changes._CURSOR_MAX_AGE_SECONDS", -1):
            response = self.call_api(
                "get",
                "api_playback_progress_changes",
                params={"cursor": cursor},
                headers=self.auth_headers,
            )
        self.assertEqual(response.status_code, HTTP.CONFLICT)
        self.assertEqual(response.data["error"]["code"], "cursor_expired")

    def test_progress_read_scope_can_fetch_feed_and_exposes_server_client_id(self):
        """A scoped reader can poll and gets its server-derived client identity."""
        token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio TV",
            scopes=["progress:read", "progress:write"],
        )
        headers = {"HTTP_X_API_KEY": raw_token}
        checkpoint = self.call_api(
            "get",
            "api_playback_progress_changes",
            headers=headers,
        )
        self.assertEqual(checkpoint.status_code, HTTP.OK)
        self.assertEqual(checkpoint.data["client_id"], f"token-{token.pk}")

        write = self._put_movie_progress(240, headers=headers)
        self.assertEqual(write.status_code, HTTP.OK)
        delta = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": checkpoint.data["next_cursor"]},
            headers=headers,
        )
        self.assertEqual(delta.status_code, HTTP.OK)
        self.assertEqual(delta.data["results"][0]["source_client_id"], f"token-{token.pk}")

    def test_missing_progress_read_scope_is_denied(self):
        """A token without progress:read cannot consume the change feed."""
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Scrobble only",
            scopes=["scrobble:write"],
        )
        response = self.call_api(
            "get",
            "api_playback_progress_changes",
            headers={"HTTP_X_API_KEY": raw_token},
        )
        self.assertEqual(response.status_code, HTTP.FORBIDDEN)

    def test_existing_progress_without_change_is_available_from_snapshot(self):
        """The additive migration needs no change-log backfill for current state."""
        item = self.items_by_type[MediaTypes.MOVIE.value][0]
        PlaybackProgress.objects.bulk_create(
            [
                PlaybackProgress(
                    user=self.user1,
                    item=item,
                    position_seconds=321,
                    duration_seconds=3600,
                ),
            ]
        )
        self.assertFalse(PlaybackProgressChange.objects.filter(user=self.user1).exists())

        checkpoint = self._checkpoint()
        snapshot = self.call_api(
            "get",
            "api_playback_progress",
            headers=self.auth_headers,
        )
        self.assertEqual(snapshot.status_code, HTTP.OK)
        self.assertTrue(
            any(row["position_seconds"] == 321 for row in snapshot.data["results"])
        )

        delta = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": checkpoint},
            headers=self.auth_headers,
        )
        self.assertEqual(delta.status_code, HTTP.OK)
        self.assertEqual(delta.data["results"], [])

    def test_snapshot_race_is_closed_by_checkpoint_then_delta(self):
        """A write during snapshot transfer appears after the captured checkpoint."""
        cursor = self._checkpoint()
        self.assertEqual(self._put_movie_progress(456).status_code, HTTP.OK)

        snapshot = self.call_api(
            "get",
            "api_playback_progress",
            headers=self.auth_headers,
        )
        self.assertEqual(snapshot.status_code, HTTP.OK)

        delta = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": cursor},
            headers=self.auth_headers,
        )
        self.assertEqual(delta.status_code, HTTP.OK)
        self.assertEqual(len(delta.data["results"]), 1)
        self.assertEqual(delta.data["results"][0]["position_seconds"], 456)

    def test_limit_is_bounded(self):
        """Clients cannot request an unbounded change-feed page."""
        cursor = self._checkpoint()
        response = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": cursor, "limit": 101},
            headers=self.auth_headers,
        )
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
