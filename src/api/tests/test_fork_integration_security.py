# FORK: security regression tests for scoped integration credentials and receipts.
from http import HTTPStatus as HTTP  # noqa: N814
from unittest.mock import MagicMock

from rest_framework.response import Response

from integrations.delivery import get_or_record_receipt
from integrations.models import IntegrationEventReceipt, IntegrationToken

from .base import FloppyApiTestCase


class IntegrationScopeEnforcementTests(FloppyApiTestCase):
    """Verify scoped credentials fail closed at real API boundaries."""

    def test_progress_read_token_cannot_scrobble(self):
        """A read-only progress credential cannot mutate scrobble state."""
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Progress reader",
            scopes=["progress:read"],
        )
        movie_item = self.items_by_type["movie"][0]

        response = self.call_api(
            "post",
            "api_scrobble",
            payload={
                "action": "start",
                "media_type": "movie",
                "ids": {"tmdb": movie_item.media_id},
            },
            headers={"HTTP_X_API_KEY": raw_token},
        )

        self.assertEqual(response.status_code, HTTP.FORBIDDEN)

    def test_scrobble_token_cannot_read_progress(self):
        """A scrobble-only credential cannot read resume positions."""
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Scrobble writer",
            scopes=["scrobble:write"],
        )

        response = self.call_api(
            "get",
            "api_playback_progress",
            headers={"HTTP_X_API_KEY": raw_token},
        )

        self.assertEqual(response.status_code, HTTP.FORBIDDEN)

    def test_integration_token_cannot_use_undeclared_endpoint(self):
        """A scoped credential cannot inherit account-wide access by omission."""
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Default integration",
        )

        response = self.call_api(
            "get",
            "api_history",
            headers={"HTTP_X_API_KEY": raw_token},
        )

        self.assertEqual(response.status_code, HTTP.FORBIDDEN)

    def test_successful_use_records_last_used_at(self):
        """Successful scoped authentication updates bounded usage metadata."""
        token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Progress reader",
            scopes=["progress:read"],
        )
        self.assertIsNone(token.last_used_at)

        response = self.call_api(
            "get",
            "api_playback_progress",
            headers={"HTTP_X_API_KEY": raw_token},
        )

        self.assertEqual(response.status_code, HTTP.OK)
        token.refresh_from_db()
        self.assertIsNotNone(token.last_used_at)


class DeliveryReservationSecurityTests(FloppyApiTestCase):
    """Verify a delivery key is reserved before a protected mutation runs."""

    def test_exception_leaves_fail_closed_reservation(self):
        """A crash cannot make a retry execute an uncertain mutation again."""
        execute = MagicMock(side_effect=RuntimeError("simulated crash"))

        with self.assertRaises(RuntimeError):
            get_or_record_receipt(
                user=self.user1,
                client_event_id="crash-safe-key",
                payload={"action": "stop"},
                execute_fn=execute,
            )

        receipt = IntegrationEventReceipt.objects.get(
            user=self.user1,
            client_event_id="crash-safe-key",
        )
        self.assertEqual(receipt.response_status_code, 0)

        second_execute = MagicMock(
            return_value=Response({"detail": "must not run"}, status=HTTP.OK)
        )
        response, is_replay = get_or_record_receipt(
            user=self.user1,
            client_event_id="crash-safe-key",
            payload={"action": "stop"},
            execute_fn=second_execute,
        )

        self.assertTrue(is_replay)
        self.assertEqual(response.status_code, HTTP.CONFLICT)
        self.assertEqual(response.data["error"]["code"], "idempotency_incomplete")
        second_execute.assert_not_called()

    def test_oversized_idempotency_key_is_rejected_before_execution(self):
        """Oversized attacker-controlled keys cannot reach receipt storage."""
        execute = MagicMock(return_value=Response({"detail": "ok"}, status=HTTP.OK))

        response, is_replay = get_or_record_receipt(
            user=self.user1,
            client_event_id="x" * 129,
            payload={"action": "start"},
            execute_fn=execute,
        )

        self.assertFalse(is_replay)
        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "invalid_idempotency_key")
        execute.assert_not_called()

    def test_control_characters_in_idempotency_key_are_rejected(self):
        """Control characters cannot enter receipt identifiers or error output."""
        execute = MagicMock(return_value=Response({"detail": "ok"}, status=HTTP.OK))

        response, _ = get_or_record_receipt(
            user=self.user1,
            client_event_id="bad\nkey",
            payload={"action": "start"},
            execute_fn=execute,
        )

        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.data["error"]["code"], "invalid_idempotency_key")
        execute.assert_not_called()
