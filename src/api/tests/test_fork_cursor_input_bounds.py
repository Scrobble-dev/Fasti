"""Security regression coverage for bounded integration cursor inputs."""

from http import HTTPStatus as HTTP  # noqa: N814

from integrations.sync_policy import CURSOR_MAX_LENGTH

from .base import FloppyApiTestCase


class IntegrationCursorInputBoundsTests(FloppyApiTestCase):
    """Reject oversized opaque cursors before expensive decoding work."""

    def setUp(self):
        """Create one cursor value just beyond the supported public limit."""
        super().setUp()
        self.oversized_cursor = "x" * (CURSOR_MAX_LENGTH + 1)

    def _get(self, url_name):
        return self.call_api(
            "get",
            url_name,
            params={"cursor": self.oversized_cursor},
            headers=self.auth_headers,
        )

    def test_progress_change_cursor_is_bounded(self):
        """Incremental progress rejects oversized cursor input."""
        response = self._get("api_playback_progress_changes")
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "invalid_cursor")

    def test_saved_media_change_cursor_is_bounded(self):
        """Incremental saved-media sync rejects oversized cursor input."""
        response = self._get("api_watchlist_changes")
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "invalid_cursor")

    def test_progress_snapshot_cursor_is_bounded(self):
        """Stable progress snapshot rejects oversized cursor input."""
        response = self._get("api_playback_progress_snapshot")
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(
            response.data["error"]["code"],
            "invalid_snapshot_cursor",
        )

    def test_saved_media_snapshot_cursor_is_bounded(self):
        """Stable saved-media snapshot rejects oversized cursor input."""
        response = self._get("api_watchlist_snapshot")
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(
            response.data["error"]["code"],
            "invalid_snapshot_cursor",
        )
