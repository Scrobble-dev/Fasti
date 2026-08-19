# FORK: client-namespace coverage for durable integration receipts.
import hashlib
from http import HTTPStatus as HTTP  # noqa: N814
from unittest.mock import MagicMock

from rest_framework.response import Response

from integrations.delivery import get_or_record_receipt
from integrations.models import IntegrationEventReceipt, IntegrationToken

from .base import FloppyApiTestCase

_CLIENT_NAMESPACE_DIGEST_LENGTH = 24


def _expected_client_storage_id(client_identifier: str, event_id: str) -> str:
    """Return the opaque storage key expected for a named integration client."""
    digest = hashlib.sha256(client_identifier.encode("utf-8")).hexdigest()
    return f"client-{digest[:_CLIENT_NAMESPACE_DIGEST_LENGTH]}:{event_id}"


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
        self.assertEqual(
            receipts[0].client_event_id,
            _expected_client_storage_id("nuvio-living-room", "shared-event"),
        )
        self.assertEqual(receipts[0].token, token_one)
        self.assertEqual(
            receipts[1].client_event_id,
            _expected_client_storage_id("nuvio-bedroom", "shared-event"),
        )
        self.assertEqual(receipts[1].token, token_two)
        self.assertNotIn("nuvio-living-room", receipts[0].client_event_id)
        self.assertNotIn("nuvio-bedroom", receipts[1].client_event_id)
        self.assertNotIn(token_one.token_digest, receipts[0].client_event_id)
        self.assertNotIn(token_two.token_digest, receipts[1].client_event_id)

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

    def test_rotated_token_reuses_named_client_namespace(self):
        """A client identifier keeps idempotency state stable across token rotation."""
        old_token, _ = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio old",
            client_identifier="nuvio-living-room",
        )
        new_token, _ = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio rotated",
            client_identifier="nuvio-living-room",
        )
        initial_execute = MagicMock(
            return_value=Response({"detail": "accepted"}, status=HTTP.OK)
        )
        replay_execute = MagicMock()

        initial, initial_replay = get_or_record_receipt(
            user=self.user1,
            client_event_id="rotation-event",
            payload={"position_seconds": 42},
            execute_fn=initial_execute,
            token=old_token,
        )
        replay, is_replay = get_or_record_receipt(
            user=self.user1,
            client_event_id="rotation-event",
            payload={"position_seconds": 42},
            execute_fn=replay_execute,
            token=new_token,
        )

        self.assertFalse(initial_replay)
        self.assertTrue(is_replay)
        self.assertEqual(initial.data, {"detail": "accepted"})
        self.assertEqual(replay.data, {"detail": "accepted"})
        initial_execute.assert_called_once()
        replay_execute.assert_not_called()
        self.assertEqual(
            IntegrationEventReceipt.objects.filter(user=self.user1).count(),
            1,
        )

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
        expected_digest = hashlib.sha256(token.token_digest.encode("utf-8")).hexdigest()
        self.assertTrue(
            IntegrationEventReceipt.objects.filter(
                user=self.user1,
                client_event_id=(
                    f"client-{expected_digest[:_CLIENT_NAMESPACE_DIGEST_LENGTH]}:"
                    "shared-legacy-event"
                ),
                token=token,
            ).exists()
        )
