# FORK: client-namespace coverage for durable integration receipts.
from http import HTTPStatus as HTTP  # noqa: N814
from unittest.mock import MagicMock

from rest_framework.response import Response

from integrations.delivery import get_or_record_receipt
from integrations.models import IntegrationEventReceipt, IntegrationToken

from .base import FloppyApiTestCase

_CLIENT_NAMESPACE_DIGEST_LENGTH = 24


class DeliveryClientIsolationTests(FloppyApiTestCase):
    """Verify one user's integrations do not share idempotency state."""

    def test_two_tokens_can_reuse_same_public_event_id(self):
        """Different authenticated clients execute and replay independently."""
        token_one, _ = IntegrationToken.generate(
            user=self.user1,
            name="Living room",
            client_identifier="nuvio-living-room",
        )
        token_two, _ = IntegrationToken.generate(
            user=self.user1,
            name="Bedroom",
            client_identifier="nuvio-bedroom",
        )
        first_execute = MagicMock(
            return_value=Response({"client": "one"}, status=HTTP.OK)
        )
        second_execute = MagicMock(
            return_value=Response({"client": "two"}, status=HTTP.OK)
        )

        first_response, first_replay = get_or_record_receipt(
            user=self.user1,
            client_event_id="shared-event",
            payload={"position_seconds": 10},
            execute_fn=first_execute,
            token=token_one,
        )
        second_response, second_replay = get_or_record_receipt(
            user=self.user1,
            client_event_id="shared-event",
            payload={"position_seconds": 20},
            execute_fn=second_execute,
            token=token_two,
        )

        self.assertFalse(first_replay)
        self.assertFalse(second_replay)
        self.assertEqual(first_response.data, {"client": "one"})
        self.assertEqual(second_response.data, {"client": "two"})
        first_execute.assert_called_once()
        second_execute.assert_called_once()

        receipts = IntegrationEventReceipt.objects.filter(user=self.user1).order_by("id")
        self.assertEqual(receipts.count(), 2)
        self.assertEqual(receipts[0].client_event_id, "shared-event")
        self.assertEqual(receipts[0].token, token_one)
        self.assertEqual(
            receipts[1].client_event_id,
            (
                f"token-{token_two.token_digest[:_CLIENT_NAMESPACE_DIGEST_LENGTH]}:"
                "shared-event"
            ),
        )
        self.assertNotIn(f"token-{token_two.pk}:", receipts[1].client_event_id)
        self.assertEqual(receipts[1].token, token_two)

        first_replay_execute = MagicMock()
        second_replay_execute = MagicMock()
        first_cached, first_is_replay = get_or_record_receipt(
            user=self.user1,
            client_event_id="shared-event",
            payload={"position_seconds": 10},
            execute_fn=first_replay_execute,
            token=token_one,
        )
        second_cached, second_is_replay = get_or_record_receipt(
            user=self.user1,
            client_event_id="shared-event",
            payload={"position_seconds": 20},
            execute_fn=second_replay_execute,
            token=token_two,
        )

        self.assertTrue(first_is_replay)
        self.assertTrue(second_is_replay)
        self.assertEqual(first_cached.data, {"client": "one"})
        self.assertEqual(second_cached.data, {"client": "two"})
        first_replay_execute.assert_not_called()
        second_replay_execute.assert_not_called()

    def test_legacy_and_scoped_clients_do_not_share_receipts(self):
        """Legacy account delivery state does not capture a scoped client's key."""
        token, _ = IntegrationToken.generate(
            user=self.user1,
            name="Scoped client",
        )
        legacy_execute = MagicMock(
            return_value=Response({"client": "legacy"}, status=HTTP.OK)
        )
        scoped_execute = MagicMock(
            return_value=Response({"client": "scoped"}, status=HTTP.OK)
        )

        legacy_response, _ = get_or_record_receipt(
            user=self.user1,
            client_event_id="shared-legacy-event",
            payload={"value": 1},
            execute_fn=legacy_execute,
            token=None,
        )
        scoped_response, _ = get_or_record_receipt(
            user=self.user1,
            client_event_id="shared-legacy-event",
            payload={"value": 2},
            execute_fn=scoped_execute,
            token=token,
        )

        self.assertEqual(legacy_response.data, {"client": "legacy"})
        self.assertEqual(scoped_response.data, {"client": "scoped"})
        self.assertTrue(
            IntegrationEventReceipt.objects.filter(
                user=self.user1,
                client_event_id="shared-legacy-event",
                token__isnull=True,
            ).exists()
        )
        self.assertTrue(
            IntegrationEventReceipt.objects.filter(
                user=self.user1,
                client_event_id=(
                    f"token-{token.token_digest[:_CLIENT_NAMESPACE_DIGEST_LENGTH]}:"
                    "shared-legacy-event"
                ),
                token=token,
            ).exists()
        )
