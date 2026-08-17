# FORK: regression coverage for public API error-detail redaction.
from http import HTTPStatus as HTTP  # noqa: N814
from unittest.mock import patch

from .base import FloppyApiTestCase


class ApiErrorRedactionTests(FloppyApiTestCase):
    """Verify server-side exception text does not enter public API responses."""

    def test_playback_resolution_exception_is_not_returned_to_client(self):
        """A provider failure returns a stable error without its exception text."""
        secret_detail = "provider-secret-do-not-return"
        with patch(
            "api.fork_views_playback.resolve_video_item",
            side_effect=RuntimeError(secret_detail),
        ):
            response = self.call_api(
                "put",
                "api_playback_progress",
                payload={
                    "media_type": "movie",
                    "ids": {"tmdb": "701"},
                    "position_seconds": 30,
                },
                headers=self.auth_headers,
            )

        self.assertEqual(response.status_code, HTTP.NOT_FOUND)
        self.assertEqual(response.data, {"detail": "Could not resolve media."})
        self.assertNotIn(secret_detail, response.content.decode("utf-8"))

    def test_playback_clear_exception_is_not_returned_to_client(self):
        """Delete failures use the same redacted public error contract."""
        secret_detail = "database-or-provider-internal-detail"
        with patch(
            "api.fork_views_playback.resolve_video_item",
            side_effect=RuntimeError(secret_detail),
        ):
            response = self.call_api(
                "delete",
                "api_playback_progress",
                payload={
                    "media_type": "movie",
                    "ids": {"tmdb": "701"},
                },
                headers=self.auth_headers,
            )

        self.assertEqual(response.status_code, HTTP.NOT_FOUND)
        self.assertEqual(response.data, {"detail": "Could not resolve media."})
        self.assertNotIn(secret_detail, response.content.decode("utf-8"))
