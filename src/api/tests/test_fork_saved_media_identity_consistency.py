# FORK: exact-identity consistency coverage for saved-library writes.
from http import HTTPStatus as HTTP  # noqa: N814
from types import SimpleNamespace
from unittest.mock import patch

from app.models import MediaTypes, SavedMediaMembership

from .base import FloppyApiTestCase


class SavedMediaIdentityConsistencyTests(FloppyApiTestCase):
    """Require all supplied provider aliases to describe the same media item."""

    def test_conflicting_aliases_do_not_create_membership_after_hydration(self):
        """A matching TMDB ID cannot hide a conflicting IMDb ID."""
        item = self.items_by_type[MediaTypes.MOVIE.value][0]
        item.provider_external_ids = {"imdb_id": "tt-correct"}
        item.save(update_fields=["provider_external_ids"])

        with patch(
            "api.fork_views_saved_media.ensure_item_metadata",
            return_value=SimpleNamespace(item=item),
        ):
            response = self.call_api(
                "put",
                "api_watchlist",
                payload={
                    "media_type": "movie",
                    "ids": {
                        "tmdb": item.media_id,
                        "imdb": "tt-conflicting",
                    },
                },
                headers=self.auth_headers,
            )

        self.assertEqual(response.status_code, HTTP.NOT_FOUND)
        self.assertEqual(response.data["error"]["code"], "unresolved_media")
        self.assertFalse(
            SavedMediaMembership.objects.filter(user=self.user1, item=item).exists()
        )
