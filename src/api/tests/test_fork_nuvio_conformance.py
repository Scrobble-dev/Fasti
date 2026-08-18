# FORK: conformance coverage for the Nuvio-facing watched-state contract.
from http import HTTPStatus as HTTP  # noqa: N814

from django.utils import timezone

from app.models import (
    Episode,
    Item,
    MediaTypes,
    Movie,
    PlaybackProgress,
    Sources,
    Status,
)
from integrations.models import IntegrationToken

from .base import FloppyApiTestCase


class NuvioConformanceTests(FloppyApiTestCase):
    """Verify the additive client contract needed by Nuvio-compatible callers."""

    def test_capabilities_are_public_and_advertise_separate_state_and_history_paths(self):
        """Clients can discover exact watched sync without conflating it with scrobbling."""
        response = self.call_api("get", "api_integration_capabilities")

        self.assertEqual(response.status_code, HTTP.OK)
        self.assertEqual(response.data["contract_version"], "1")
        self.assertEqual(response.data["stability"], "experimental")
        watched = response.data["resources"]["watched_state"]
        self.assertEqual(watched["snapshot"]["scope"], "watched:read")
        self.assertEqual(watched["snapshot"]["pagination"], "stable_keyset")
        self.assertEqual(watched["write"]["scope"], "watched:write")
        self.assertEqual(watched["clear"]["scope"], "watched:write")
        self.assertFalse(watched["write"]["mutates_viewing_history"])
        self.assertFalse(watched["clear"]["mutates_viewing_history"])
        self.assertTrue(watched["changes"]["explicit_unwatched_events"])
        self.assertEqual(watched["occurrence_write"]["via"], "scrobble")
        self.assertFalse(response.data["identity"]["authoritative_title_matching"])
        self.assertTrue(response.data["identity"]["all_supplied_ids_must_agree"])
        self.assertTrue(
            response.data["sync"][
                "viewing_history_is_separate_from_current_watched_state"
            ]
        )

    def test_watched_read_scope_is_required_for_scoped_tokens(self):
        """A progress-only token cannot read watched-state history projections."""
        _denied_token, denied_raw = IntegrationToken.generate(
            user=self.user1,
            name="Progress only",
            scopes=["progress:read"],
        )
        denied = self.call_api(
            "get",
            "api_watched_state",
            params={"media_type": "movie"},
            headers={"HTTP_X_API_KEY": denied_raw},
        )
        self.assertEqual(denied.status_code, HTTP.FORBIDDEN)

        _allowed_token, allowed_raw = IntegrationToken.generate(
            user=self.user1,
            name="Watched reader",
            scopes=["watched:read"],
        )
        allowed = self.call_api(
            "get",
            "api_watched_state",
            params={"media_type": "movie"},
            headers={"HTTP_X_API_KEY": allowed_raw},
        )
        self.assertEqual(allowed.status_code, HTTP.OK)

    def test_movie_snapshot_uses_exact_ids_and_allows_unknown_watch_time(self):
        """Completed state can be represented without inventing a timestamp."""
        item = self.items_by_type[MediaTypes.MOVIE.value][0]
        movie = self.movie_medias[0]
        movie.status = Status.COMPLETED.value
        movie.end_date = None
        movie.save(update_fields=["status", "end_date"])

        response = self.call_api(
            "get",
            "api_watched_state",
            params={"media_type": "movie"},
            headers=self.auth_headers,
        )

        self.assertEqual(response.status_code, HTTP.OK)
        row = next(
            result
            for result in response.data["results"]
            if result["media_id"] == str(item.media_id)
        )
        self.assertEqual(row["media_type"], "movie")
        self.assertEqual(row["ids"]["tmdb"], str(item.media_id))
        self.assertIsNone(row["watched_at"])

    def test_progress_only_movie_is_not_watched(self):
        """Resume progress cannot manufacture watched state."""
        item = Item.objects.create(
            media_id="799",
            source=Sources.TMDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="Progress only",
        )
        Movie.objects.create(
            item=item,
            user=self.user1,
            status=Status.IN_PROGRESS.value,
        )
        PlaybackProgress.objects.create(
            user=self.user1,
            item=item,
            position_seconds=900,
            duration_seconds=3600,
            completed=False,
        )

        response = self.call_api(
            "get",
            "api_watched_state",
            params={"media_type": "movie"},
            headers=self.auth_headers,
        )

        self.assertEqual(response.status_code, HTTP.OK)
        self.assertFalse(
            any(row["media_id"] == "799" for row in response.data["results"])
        )

    def test_repeated_episode_occurrences_collapse_to_one_current_state_row(self):
        """Current watched state does not erase or duplicate Floppy occurrence history."""
        item = self.items_by_type[MediaTypes.EPISODE.value][0]
        first = self.episode_medias[0]
        first.end_date = timezone.now() - timezone.timedelta(days=1)
        first.status = Status.COMPLETED.value
        first.save(update_fields=["end_date", "status"])
        second_time = timezone.now()
        Episode.objects.create(
            item=item,
            related_season=self.season_medias[0],
            status=Status.COMPLETED.value,
            end_date=second_time,
        )

        response = self.call_api(
            "get",
            "api_watched_state",
            params={"media_type": "episode"},
            headers=self.auth_headers,
        )

        self.assertEqual(response.status_code, HTTP.OK)
        matches = [
            row
            for row in response.data["results"]
            if row["media_id"] == str(item.media_id)
            and row["season_number"] == item.season_number
            and row["episode_number"] == item.episode_number
        ]
        self.assertEqual(len(matches), 1)
        self.assertEqual(matches[0]["ids"]["tmdb"], str(item.media_id))
        self.assertEqual(matches[0]["season_number"], 1)
        self.assertEqual(matches[0]["episode_number"], 1)
        self.assertEqual(matches[0]["watched_at"], second_time)

    def test_watched_snapshot_requires_explicit_supported_media_type(self):
        """The compatibility endpoint retains its bounded one-type contract."""
        missing = self.call_api(
            "get",
            "api_watched_state",
            headers=self.auth_headers,
        )
        unsupported = self.call_api(
            "get",
            "api_watched_state",
            params={"media_type": "book"},
            headers=self.auth_headers,
        )

        self.assertEqual(missing.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(unsupported.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(missing.data["error"]["code"], "unsupported_media_type")
        self.assertEqual(unsupported.data["error"]["code"], "unsupported_media_type")
