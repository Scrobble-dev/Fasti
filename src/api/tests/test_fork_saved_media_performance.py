# FORK: performance regression coverage for saved-library snapshots.
from http import HTTPStatus as HTTP  # noqa: N814
from unittest.mock import patch

from api import fork_views_saved_media
from app.models import MediaTypes, SavedMediaMembership

from .base import FloppyApiTestCase


class SavedMediaSnapshotPerformanceTests(FloppyApiTestCase):
    """Keep saved-media snapshot work proportional to the requested page."""

    def test_snapshot_serializes_only_requested_page(self):
        """A one-row page does not serialize the user's whole saved library."""
        movie_items = self.items_by_type[MediaTypes.MOVIE.value]
        SavedMediaMembership.objects.bulk_create(
            [
                SavedMediaMembership(user=self.user1, item=item)
                for item in movie_items
            ]
        )

        with patch.object(
            fork_views_saved_media,
            "_serialize_membership",
            wraps=fork_views_saved_media._serialize_membership,
        ) as serializer:
            response = self.call_api(
                "get",
                "api_watchlist",
                params={"limit": 1, "offset": 1},
                headers=self.auth_headers,
            )

        self.assertEqual(response.status_code, HTTP.OK)
        self.assertEqual(response.data["pagination"]["total"], len(movie_items))
        self.assertEqual(response.data["pagination"]["limit"], 1)
        self.assertEqual(response.data["pagination"]["offset"], 1)
        self.assertEqual(len(response.data["results"]), 1)
        self.assertEqual(serializer.call_count, 1)
