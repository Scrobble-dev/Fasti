"""Security regression coverage for client-bound integration change cursors."""

from http import HTTPStatus as HTTP  # noqa: N814

from integrations.models import IntegrationToken

from .base import FloppyApiTestCase


class ChangeCursorClientIsolationTests(FloppyApiTestCase):
    """Keep incremental checkpoints bound to the client that received them."""

    def _token_headers(self, name, scope):
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name=name,
            scopes=[scope],
        )
        return {"HTTP_X_API_KEY": raw_token}

    def test_playback_cursor_cannot_cross_scoped_clients(self):
        """A second scoped client cannot consume another client's progress cursor."""
        first_headers = self._token_headers("Nuvio progress A", "progress:read")
        second_headers = self._token_headers("Nuvio progress B", "progress:read")

        checkpoint = self.call_api(
            "get",
            "api_playback_progress_changes",
            headers=first_headers,
        )
        self.assertEqual(checkpoint.status_code, HTTP.OK)
        self.assertEqual(checkpoint.data["version"], 2)
        cursor = checkpoint.data["next_cursor"]

        denied = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": cursor},
            headers=second_headers,
        )
        self.assertEqual(denied.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(denied.data["error"]["code"], "invalid_cursor")

        allowed = self.call_api(
            "get",
            "api_playback_progress_changes",
            params={"cursor": cursor},
            headers=first_headers,
        )
        self.assertEqual(allowed.status_code, HTTP.OK)

    def test_saved_media_cursor_cannot_cross_scoped_clients(self):
        """A second scoped client cannot consume another client's saved-media cursor."""
        first_headers = self._token_headers("Nuvio library A", "watchlist:read")
        second_headers = self._token_headers("Nuvio library B", "watchlist:read")

        checkpoint = self.call_api(
            "get",
            "api_watchlist_changes",
            headers=first_headers,
        )
        self.assertEqual(checkpoint.status_code, HTTP.OK)
        self.assertEqual(checkpoint.data["version"], 2)
        cursor = checkpoint.data["next_cursor"]

        denied = self.call_api(
            "get",
            "api_watchlist_changes",
            params={"cursor": cursor},
            headers=second_headers,
        )
        self.assertEqual(denied.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(denied.data["error"]["code"], "invalid_cursor")

        allowed = self.call_api(
            "get",
            "api_watchlist_changes",
            params={"cursor": cursor},
            headers=first_headers,
        )
        self.assertEqual(allowed.status_code, HTTP.OK)
