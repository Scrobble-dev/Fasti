# FORK: regression coverage for saved-library membership synchronization.
from http import HTTPStatus as HTTP  # noqa: N814

from app.models import (
    MediaTypes,
    Movie,
    MoviePlay,
    PlaybackProgress,
    SavedMediaChange,
    SavedMediaMembership,
)
from integrations.models import IntegrationToken

from .base import FloppyApiTestCase


class SavedMediaSyncTests(FloppyApiTestCase):
    """Verify explicit saved-library membership and ordered changes."""

    def _movie_payload(self):
        item = self.items_by_type[MediaTypes.MOVIE.value][0]
        return item, {"media_type": "movie", "ids": {"tmdb": item.media_id}}

    def _checkpoint(self, headers=None):
        response = self.call_api(
            "get",
            "api_watchlist_changes",
            headers=headers or self.auth_headers,
        )
        self.assertEqual(response.status_code, HTTP.OK)
        self.assertTrue(response.data["snapshot_required"])
        return response.data["next_cursor"]

    def test_add_is_separate_from_progress_history_and_tracking_status(self):
        """Saving media does not manufacture playback or rewrite tracked state."""
        item, payload = self._movie_payload()
        tracked = Movie.objects.filter(user=self.user1, item=item).first()
        status_before = tracked.status if tracked is not None else None
        plays_before = MoviePlay.objects.filter(user=self.user1, item=item).count()
        progress_before = PlaybackProgress.objects.filter(user=self.user1, item=item).count()

        response = self.call_api(
            "put",
            "api_watchlist",
            payload=payload,
            headers=self.auth_headers,
        )

        self.assertEqual(response.status_code, HTTP.CREATED)
        self.assertTrue(
            SavedMediaMembership.objects.filter(user=self.user1, item=item).exists()
        )
        self.assertEqual(
            MoviePlay.objects.filter(user=self.user1, item=item).count(),
            plays_before,
        )
        self.assertEqual(
            PlaybackProgress.objects.filter(user=self.user1, item=item).count(),
            progress_before,
        )
        if tracked is not None:
            tracked.refresh_from_db()
            self.assertEqual(tracked.status, status_before)

    def test_remove_preserves_existing_tracked_media(self):
        """Removing saved intent cannot delete or downgrade an existing track row."""
        item, payload = self._movie_payload()
        tracked = Movie.objects.filter(user=self.user1, item=item).first()
        self.assertEqual(
            self.call_api("put", "api_watchlist", payload=payload, headers=self.auth_headers).status_code,
            HTTP.CREATED,
        )

        response = self.call_api(
            "delete",
            "api_watchlist",
            payload=payload,
            headers=self.auth_headers,
        )

        self.assertEqual(response.status_code, HTTP.NO_CONTENT)
        self.assertFalse(
            SavedMediaMembership.objects.filter(user=self.user1, item=item).exists()
        )
        self.assertTrue(item.__class__.objects.filter(pk=item.pk).exists())
        if tracked is not None:
            self.assertTrue(Movie.objects.filter(pk=tracked.pk).exists())

    def test_add_and_remove_are_explicit_ordered_changes(self):
        """The change feed reports only committed add/remove operations after C0."""
        item, payload = self._movie_payload()
        cursor = self._checkpoint()
        add = self.call_api(
            "put",
            "api_watchlist",
            payload=payload,
            headers=self.auth_headers,
        )
        self.assertEqual(add.status_code, HTTP.CREATED)
        remove = self.call_api(
            "delete",
            "api_watchlist",
            payload=payload,
            headers=self.auth_headers,
        )
        self.assertEqual(remove.status_code, HTTP.NO_CONTENT)

        delta = self.call_api(
            "get",
            "api_watchlist_changes",
            params={"cursor": cursor},
            headers=self.auth_headers,
        )

        self.assertEqual(delta.status_code, HTTP.OK)
        self.assertEqual(
            [row["operation"] for row in delta.data["results"]],
            ["add", "remove"],
        )
        self.assertEqual(
            [row["item_id"] for row in delta.data["results"]],
            [item.id, item.id],
        )
        sequences = [row["sequence_id"] for row in delta.data["results"]]
        self.assertEqual(sequences, sorted(sequences))

    def test_same_idempotency_key_replays_without_duplicate_change(self):
        """A retried add returns the first result and appends one add event."""
        item, payload = self._movie_payload()
        headers = {**self.auth_headers, "HTTP_IDEMPOTENCY_KEY": "saved-add-1"}

        first = self.call_api("put", "api_watchlist", payload=payload, headers=headers)
        second = self.call_api("put", "api_watchlist", payload=payload, headers=headers)

        self.assertEqual(first.status_code, HTTP.CREATED)
        self.assertEqual(second.status_code, HTTP.CREATED)
        self.assertEqual(
            SavedMediaMembership.objects.filter(user=self.user1, item=item).count(),
            1,
        )
        self.assertEqual(
            SavedMediaChange.objects.filter(
                user=self.user1,
                item_id=item.id,
                operation=SavedMediaChange.Operation.ADD,
            ).count(),
            1,
        )

    def test_title_only_payload_is_rejected(self):
        """Saved-media writes require a supported exact provider identity."""
        response = self.call_api(
            "put",
            "api_watchlist",
            payload={"media_type": "movie", "title": "A title is not identity", "ids": {}},
            headers=self.auth_headers,
        )

        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "missing_media_identity")
        self.assertFalse(SavedMediaMembership.objects.filter(user=self.user1).exists())

    def test_snapshot_is_user_scoped(self):
        """A saved item never appears in another user's snapshot."""
        _item, payload = self._movie_payload()
        self.assertEqual(
            self.call_api("put", "api_watchlist", payload=payload, headers=self.auth_headers).status_code,
            HTTP.CREATED,
        )

        response = self.call_api(
            "get",
            "api_watchlist",
            headers={"HTTP_X_API_KEY": self.user2.token},
        )

        self.assertEqual(response.status_code, HTTP.OK)
        self.assertEqual(response.data["results"], [])

    def test_cursor_is_user_bound(self):
        """Another user cannot consume a saved-media cursor."""
        cursor = self._checkpoint()
        response = self.call_api(
            "get",
            "api_watchlist_changes",
            params={"cursor": cursor},
            headers={"HTTP_X_API_KEY": self.user2.token},
        )
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "invalid_cursor")

    def test_watchlist_scopes_are_enforced(self):
        """Read and write watchlist scopes remain independent."""
        item, payload = self._movie_payload()
        _read_token, read_raw = IntegrationToken.generate(
            user=self.user1,
            name="Saved reader",
            scopes=["watchlist:read"],
        )
        _write_token, write_raw = IntegrationToken.generate(
            user=self.user1,
            name="Saved writer",
            scopes=["watchlist:write"],
        )

        denied_write = self.call_api(
            "put",
            "api_watchlist",
            payload=payload,
            headers={"HTTP_X_API_KEY": read_raw},
        )
        self.assertEqual(denied_write.status_code, HTTP.FORBIDDEN)

        allowed_write = self.call_api(
            "put",
            "api_watchlist",
            payload=payload,
            headers={"HTTP_X_API_KEY": write_raw},
        )
        self.assertEqual(allowed_write.status_code, HTTP.CREATED)
        self.assertTrue(
            SavedMediaMembership.objects.filter(user=self.user1, item=item).exists()
        )

        denied_read = self.call_api(
            "get",
            "api_watchlist",
            headers={"HTTP_X_API_KEY": write_raw},
        )
        self.assertEqual(denied_read.status_code, HTTP.FORBIDDEN)

        allowed_read = self.call_api(
            "get",
            "api_watchlist",
            headers={"HTTP_X_API_KEY": read_raw},
        )
        self.assertEqual(allowed_read.status_code, HTTP.OK)
        self.assertEqual(len(allowed_read.data["results"]), 1)
