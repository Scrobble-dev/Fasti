"""Regression and security tests for exact current watched-state synchronization."""

from http import HTTPStatus as HTTP  # noqa: N814

from django.utils import timezone

from app.models import MediaTypes, WatchedState, WatchedStateChange
from integrations.models import IntegrationToken

from .base import FloppyApiTestCase


class WatchedStateScopeTests(FloppyApiTestCase):
    """Keep watched-state read and write authority independent."""

    def test_read_scope_can_snapshot_but_cannot_write(self):
        """A watched reader can reconcile state but cannot mutate it."""
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Watched reader",
            scopes=["watched:read"],
        )
        headers = {"HTTP_X_API_KEY": raw_token}

        snapshot = self.call_api(
            "get",
            "api_watched_state_snapshot",
            headers=headers,
        )
        write = self.call_api(
            "put",
            "api_watched_state",
            payload={
                "media_type": "movie",
                "ids": {"tmdb": "701"},
                "watched": True,
            },
            headers=headers,
        )

        self.assertEqual(snapshot.status_code, HTTP.OK)
        self.assertEqual(write.status_code, HTTP.FORBIDDEN)

    def test_write_scope_can_mutate_but_cannot_read(self):
        """A watched writer cannot use the read-side reconciliation surfaces."""
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Watched writer",
            scopes=["watched:write"],
        )
        headers = {"HTTP_X_API_KEY": raw_token}

        write = self.call_api(
            "put",
            "api_watched_state",
            payload={
                "media_type": "movie",
                "ids": {"tmdb": "701"},
                "watched": False,
            },
            headers=headers,
        )
        snapshot = self.call_api(
            "get",
            "api_watched_state_snapshot",
            headers=headers,
        )

        self.assertEqual(write.status_code, HTTP.OK)
        self.assertEqual(snapshot.status_code, HTTP.FORBIDDEN)


class WatchedStateHistoryIsolationTests(FloppyApiTestCase):
    """Current watched state must never rewrite viewing-occurrence history."""

    def setUp(self):
        super().setUp()
        _token, self.raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio state sync",
            scopes=["watched:read", "watched:write"],
        )
        self.headers = {"HTTP_X_API_KEY": self.raw_token}

    def test_movie_unwatched_state_preserves_movie_play(self):
        """watched=false is a current-state operation, not a history delete."""
        movie = self.movie_medias[0]
        movie.watch(timezone.now())
        play_count = movie.plays.count()

        response = self.call_api(
            "put",
            "api_watched_state",
            payload={
                "media_type": "movie",
                "ids": {"tmdb": movie.item.media_id},
                "watched": False,
            },
            headers=self.headers,
        )

        self.assertEqual(response.status_code, HTTP.OK)
        self.assertEqual(movie.plays.count(), play_count)
        state = WatchedState.objects.get(user=self.user1, item=movie.item)
        self.assertFalse(state.watched)

    def test_episode_clear_preserves_exact_episode_occurrence(self):
        """Clearing sync state does not remove or infer sibling episode history."""
        episode = self.episode_medias[0]
        occurrence_count = type(episode).objects.filter(
            related_season__user=self.user1,
            item=episode.item,
        ).count()

        set_response = self.call_api(
            "put",
            "api_watched_state",
            payload={
                "media_type": "episode",
                "ids": {"tmdb": episode.item.media_id},
                "season_number": episode.item.season_number,
                "episode_number": episode.item.episode_number,
                "watched": True,
            },
            headers=self.headers,
        )
        clear_response = self.call_api(
            "delete",
            "api_watched_state",
            payload={
                "media_type": "episode",
                "ids": {"tmdb": episode.item.media_id},
                "season_number": episode.item.season_number,
                "episode_number": episode.item.episode_number,
            },
            headers=self.headers,
        )

        self.assertEqual(set_response.status_code, HTTP.OK)
        self.assertEqual(clear_response.status_code, HTTP.OK)
        self.assertEqual(
            type(episode).objects.filter(
                related_season__user=self.user1,
                item=episode.item,
            ).count(),
            occurrence_count,
        )
        state = WatchedState.objects.get(user=self.user1, item=episode.item)
        self.assertFalse(state.watched)

    def test_same_idempotency_key_and_payload_does_not_append_twice(self):
        """A transport retry cannot become a second watched-state mutation."""
        headers = {
            **self.headers,
            "HTTP_IDEMPOTENCY_KEY": "watched-retry-1",
        }
        payload = {
            "media_type": "movie",
            "ids": {"tmdb": "701"},
            "watched": False,
        }

        first = self.call_api(
            "put",
            "api_watched_state",
            payload=payload,
            headers=headers,
        )
        changes_after_first = WatchedStateChange.objects.filter(user=self.user1).count()
        second = self.call_api(
            "put",
            "api_watched_state",
            payload=payload,
            headers=headers,
        )

        self.assertEqual(first.status_code, HTTP.OK)
        self.assertEqual(second.status_code, HTTP.OK)
        self.assertEqual(
            WatchedStateChange.objects.filter(user=self.user1).count(),
            changes_after_first,
        )

    def test_same_idempotency_key_cannot_cross_put_delete_semantics(self):
        """The delivery key includes method/resource meaning, not only request fields."""
        headers = {
            **self.headers,
            "HTTP_IDEMPOTENCY_KEY": "watched-method-conflict",
        }
        identity = {
            "media_type": "movie",
            "ids": {"tmdb": "701"},
        }

        first = self.call_api(
            "put",
            "api_watched_state",
            payload={**identity, "watched": True},
            headers=headers,
        )
        second = self.call_api(
            "delete",
            "api_watched_state",
            payload=identity,
            headers=headers,
        )

        self.assertEqual(first.status_code, HTTP.OK)
        self.assertEqual(second.status_code, HTTP.CONFLICT)


class WatchedStateDeltaTests(FloppyApiTestCase):
    """Match Nuvio's checkpoint, snapshot, explicit-delete, and client-origin contract."""

    def setUp(self):
        super().setUp()
        _token, self.raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio client A",
            scopes=["watched:read", "watched:write"],
        )
        self.headers = {"HTTP_X_API_KEY": self.raw_token}

    def test_checkpoint_snapshot_then_unwatched_delta(self):
        """A client can take C0, snapshot current truth, then apply explicit false deltas."""
        checkpoint = self.call_api(
            "get",
            "api_watched_state_changes",
            headers=self.headers,
        )
        self.assertEqual(checkpoint.status_code, HTTP.OK)
        self.assertTrue(checkpoint.data["snapshot_required"])
        cursor = checkpoint.data["next_cursor"]

        snapshot = self.call_api(
            "get",
            "api_watched_state_snapshot",
            headers=self.headers,
        )
        self.assertEqual(snapshot.status_code, HTTP.OK)

        mutation = self.call_api(
            "put",
            "api_watched_state",
            payload={
                "media_type": "movie",
                "ids": {"tmdb": "701"},
                "watched": False,
            },
            headers=self.headers,
        )
        self.assertEqual(mutation.status_code, HTTP.OK)

        delta = self.call_api(
            "get",
            "api_watched_state_changes",
            params={"cursor": cursor},
            headers=self.headers,
        )
        self.assertEqual(delta.status_code, HTTP.OK)
        matching = [
            row
            for row in delta.data["results"]
            if row["media_type"] == MediaTypes.MOVIE.value
            and row["media_id"] == "701"
        ]
        self.assertEqual(len(matching), 1)
        self.assertFalse(matching[0]["watched"])
        self.assertTrue(matching[0]["source_client_id"].startswith("token:"))

    def test_change_cursor_cannot_be_reused_by_another_client(self):
        """Two scoped clients for one account cannot exchange reconciliation cursors."""
        checkpoint = self.call_api(
            "get",
            "api_watched_state_changes",
            headers=self.headers,
        )
        cursor = checkpoint.data["next_cursor"]

        _other, other_raw = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio client B",
            scopes=["watched:read"],
        )
        response = self.call_api(
            "get",
            "api_watched_state_changes",
            params={"cursor": cursor},
            headers={"HTTP_X_API_KEY": other_raw},
        )

        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "invalid_cursor")

    def test_snapshot_cursor_cannot_be_reused_by_another_client(self):
        """Stable snapshot continuation is also bound to the authenticated client."""
        # Force more than one page from the existing completed movie fixtures.
        first_page = self.call_api(
            "get",
            "api_watched_state_snapshot",
            params={"limit": 1, "media_type": "movie"},
            headers=self.headers,
        )
        self.assertEqual(first_page.status_code, HTTP.OK)
        cursor = first_page.data["next_cursor"]
        self.assertIsNotNone(cursor)

        _other, other_raw = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio client C",
            scopes=["watched:read"],
        )
        response = self.call_api(
            "get",
            "api_watched_state_snapshot",
            params={"cursor": cursor},
            headers={"HTTP_X_API_KEY": other_raw},
        )

        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "invalid_snapshot_cursor")
