"""Regression coverage for stable third-party reconciliation snapshots."""

from http import HTTPStatus as HTTP  # noqa: N814

from app.models import MediaTypes, PlaybackProgress, SavedMediaMembership
from integrations.models import IntegrationToken

from .base import FloppyApiTestCase


class StableProgressSnapshotTests(FloppyApiTestCase):
    """Keep progress snapshots stable while the live library changes."""

    def _get(self, params=None, headers=None):
        return self.call_api(
            "get",
            "api_playback_progress_snapshot",
            params=params,
            headers=self.auth_headers if headers is None else headers,
        )

    def _seed_movies(self, count):
        rows = []
        for index, item in enumerate(
            self.items_by_type[MediaTypes.MOVIE.value][:count],
            start=1,
        ):
            rows.append(
                PlaybackProgress.objects.create(
                    user=self.user1,
                    item=item,
                    position_seconds=index * 100,
                )
            )
        return rows

    def test_delete_between_pages_does_not_shift_or_skip_rows(self):
        """Keyset paging keeps later rows reachable after an earlier row is deleted."""
        rows = self._seed_movies(3)
        expected_media_ids = [str(row.item.media_id) for row in rows]

        first = self._get({"media_type": "movie", "limit": 1})
        self.assertEqual(first.status_code, HTTP.OK)
        self.assertTrue(first.data["has_more"])
        emitted = [str(first.data["results"][0]["media_id"])]

        rows[0].delete()

        second = self._get({"cursor": first.data["next_cursor"], "limit": 1})
        self.assertEqual(second.status_code, HTTP.OK)
        self.assertTrue(second.data["has_more"])
        emitted.append(str(second.data["results"][0]["media_id"]))

        third = self._get({"cursor": second.data["next_cursor"], "limit": 1})
        self.assertEqual(third.status_code, HTTP.OK)
        self.assertTrue(third.data["snapshot_complete"])
        emitted.append(str(third.data["results"][0]["media_id"]))

        self.assertEqual(emitted, expected_media_ids)

    def test_insert_after_first_page_is_excluded_and_arrives_in_delta(self):
        """Rows newer than the fixed snapshot boundary are delivered by the delta feed."""
        rows = self._seed_movies(2)
        checkpoint = self.call_api(
            "get",
            "api_playback_progress_changes",
            headers=self.auth_headers,
        )
        self.assertEqual(checkpoint.status_code, HTTP.OK)

        first = self._get({"media_type": "movie", "limit": 1})
        self.assertEqual(first.status_code, HTTP.OK)
        new_item = self.items_by_type[MediaTypes.MOVIE.value][2]
        PlaybackProgress.objects.create(
            user=self.user1,
            item=new_item,
            position_seconds=999,
        )

        second = self._get({"cursor": first.data["next_cursor"], "limit": 10})
        self.assertEqual(second.status_code, HTTP.OK)
        self.assertTrue(second.data["snapshot_complete"])
        snapshot_media_ids = {
            str(first.data["results"][0]["media_id"]),
            *[str(row["media_id"]) for row in second.data["results"]],
        }
        self.assertEqual(
            snapshot_media_ids,
            {str(row.item.media_id) for row in rows},
        )
        self.assertNotIn(str(new_item.media_id), snapshot_media_ids)

        delta = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": checkpoint.data["next_cursor"]},
            headers=self.auth_headers,
        )
        self.assertEqual(delta.status_code, HTTP.OK)
        self.assertEqual(len(delta.data["results"]), 1)
        self.assertEqual(str(delta.data["results"][0]["media_id"]), str(new_item.media_id))

    def test_cursor_is_bound_to_scoped_client(self):
        """Two scoped clients on one account cannot exchange snapshot cursors."""
        self._seed_movies(2)
        _first_token, first_raw = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio snapshot A",
            scopes=["progress:read"],
        )
        _second_token, second_raw = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio snapshot B",
            scopes=["progress:read"],
        )
        first_headers = {"HTTP_X_API_KEY": first_raw}
        second_headers = {"HTTP_X_API_KEY": second_raw}

        first = self._get({"media_type": "movie", "limit": 1}, first_headers)
        self.assertEqual(first.status_code, HTTP.OK)
        cursor = first.data["next_cursor"]

        denied = self._get({"cursor": cursor}, second_headers)
        self.assertEqual(denied.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(
            denied.data["error"]["code"],
            "invalid_snapshot_cursor",
        )

        allowed = self._get({"cursor": cursor}, first_headers)
        self.assertEqual(allowed.status_code, HTTP.OK)

    def test_media_filter_cannot_change_mid_snapshot(self):
        """A signed snapshot cannot be resumed under a different media filter."""
        self._seed_movies(2)
        first = self._get({"media_type": "movie", "limit": 1})
        self.assertEqual(first.status_code, HTTP.OK)

        response = self._get(
            {
                "cursor": first.data["next_cursor"],
                "media_type": "episode",
            }
        )
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "snapshot_filter_conflict")

    def test_tampered_snapshot_cursor_is_rejected(self):
        """Clients cannot edit an opaque snapshot cursor to change its boundary."""
        self._seed_movies(2)
        first = self._get({"media_type": "movie", "limit": 1})
        self.assertEqual(first.status_code, HTTP.OK)

        response = self._get({"cursor": f"{first.data['next_cursor']}x"})
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(
            response.data["error"]["code"],
            "invalid_snapshot_cursor",
        )

    def test_snapshot_requires_progress_read_scope(self):
        """A scoped token without progress:read cannot read stable progress state."""
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Watchlist only",
            scopes=["watchlist:read"],
        )
        response = self._get(headers={"HTTP_X_API_KEY": raw_token})
        self.assertEqual(response.status_code, HTTP.FORBIDDEN)


class StableSavedMediaSnapshotTests(FloppyApiTestCase):
    """Keep saved-media snapshots stable while membership changes."""

    def _get(self, params=None, headers=None):
        return self.call_api(
            "get",
            "api_watchlist_snapshot",
            params=params,
            headers=self.auth_headers if headers is None else headers,
        )

    def _seed_movies(self, count):
        rows = []
        for item in self.items_by_type[MediaTypes.MOVIE.value][:count]:
            rows.append(
                SavedMediaMembership.objects.create(user=self.user1, item=item)
            )
        return rows

    def test_delete_between_pages_does_not_shift_or_skip_memberships(self):
        """Removing an earlier membership cannot shift a later page out of reach."""
        rows = self._seed_movies(3)
        expected_media_ids = [str(row.item.media_id) for row in rows]

        first = self._get({"media_type": "movie", "limit": 1})
        self.assertEqual(first.status_code, HTTP.OK)
        emitted = [str(first.data["results"][0]["media_id"])]

        rows[0].delete()

        second = self._get({"cursor": first.data["next_cursor"], "limit": 1})
        self.assertEqual(second.status_code, HTTP.OK)
        emitted.append(str(second.data["results"][0]["media_id"]))

        third = self._get({"cursor": second.data["next_cursor"], "limit": 1})
        self.assertEqual(third.status_code, HTTP.OK)
        self.assertTrue(third.data["snapshot_complete"])
        emitted.append(str(third.data["results"][0]["media_id"]))

        self.assertEqual(emitted, expected_media_ids)

    def test_snapshot_requires_watchlist_read_scope(self):
        """A scoped token without watchlist:read cannot read stable saved state."""
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Progress only",
            scopes=["progress:read"],
        )
        response = self._get(headers={"HTTP_X_API_KEY": raw_token})
        self.assertEqual(response.status_code, HTTP.FORBIDDEN)


class IntegrationCapabilitySecurityTests(FloppyApiTestCase):
    """Keep public discovery useful without exposing server-build fingerprints."""

    def test_public_capabilities_publish_contract_not_server_version(self):
        """Unauthenticated discovery exposes stable behavior but no exact build version."""
        response = self.call_api(
            "get",
            "api_integration_capabilities",
            headers={},
        )
        self.assertEqual(response.status_code, HTTP.OK)
        self.assertNotIn("server_version", response.data)
        self.assertEqual(
            response.data["resources"]["progress"]["snapshot"]["path"],
            "/api/v1/playback/progress/snapshot/",
        )
        self.assertEqual(
            response.data["resources"]["saved_media"]["snapshot"]["path"],
            "/api/v1/watchlist/snapshot/",
        )
        self.assertTrue(
            response.data["sync"]["cursor_bound_to_authenticated_client"]
        )
