# FORK: client-namespace coverage for durable integration receipts.
import hashlib
from http import HTTPStatus as HTTP  # noqa: N814
from unittest.mock import MagicMock

from django.db import IntegrityError, transaction
from rest_framework.response import Response

from integrations.delivery import get_or_record_receipt
from integrations.models import IntegrationEventReceipt, IntegrationToken

from .base import FloppyApiTestCase

_CLIENT_NAMESPACE_DIGEST_LENGTH = 24


def _expected_namespace(source: str) -> str:
    """Return the opaque client namespace derived from a stable source."""
    digest = hashlib.sha256(source.encode("utf-8")).hexdigest()
    return f"client-{digest[:_CLIENT_NAMESPACE_DIGEST_LENGTH]}"


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
        self.assertEqual(receipts[1].client_event_id, "shared-event")
        self.assertEqual(
            receipts[0].client_namespace,
            _expected_namespace("nuvio-living-room"),
        )
        self.assertEqual(
            receipts[1].client_namespace,
            _expected_namespace("nuvio-bedroom"),
        )
        self.assertEqual(receipts[0].token, token_one)
        self.assertEqual(receipts[1].token, token_two)
        self.assertNotIn("nuvio-living-room", receipts[0].client_namespace)
        self.assertNotIn("nuvio-bedroom", receipts[1].client_namespace)
        self.assertNotIn(token_one.token_digest, receipts[0].client_namespace)
        self.assertNotIn(token_two.token_digest, receipts[1].client_namespace)

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

    def test_database_uniqueness_is_client_scoped(self):
        """The database rejects duplicate keys only inside one client namespace."""
        token, _ = IntegrationToken.generate(
            user=self.user1,
            name="Living room",
            client_identifier="nuvio-living-room",
        )
        namespace = _expected_namespace("nuvio-living-room")
        IntegrationEventReceipt.objects.create(
            user=self.user1,
            token=token,
            client_namespace=namespace,
            client_event_id="same-event",
            payload_digest="a" * 64,
        )

        with self.assertRaises(IntegrityError), transaction.atomic():
            IntegrationEventReceipt.objects.create(
                user=self.user1,
                token=token,
                client_namespace=namespace,
                client_event_id="same-event",
                payload_digest="b" * 64,
            )

        other_token, _ = IntegrationToken.generate(
            user=self.user1,
            name="Bedroom",
            client_identifier="nuvio-bedroom",
        )
        IntegrationEventReceipt.objects.create(
            user=self.user1,
            token=other_token,
            client_namespace=_expected_namespace("nuvio-bedroom"),
            client_event_id="same-event",
            payload_digest="c" * 64,
        )
        self.assertEqual(
            IntegrationEventReceipt.objects.filter(
                user=self.user1,
                client_event_id="same-event",
            ).count(),
            2,
        )

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
        receipt = IntegrationEventReceipt.objects.get(user=self.user1)
        self.assertEqual(receipt.client_event_id, "rotation-event")
        self.assertEqual(
            receipt.client_namespace,
            _expected_namespace("nuvio-living-room"),
        )

    def test_token_deletion_preserves_receipt_and_named_client_replay(self):
        """Receipt evidence and replay survive physical credential deletion."""
        token, _ = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio old",
            client_identifier="nuvio-living-room",
        )
        initial_execute = MagicMock(
            return_value=Response({"detail": "accepted"}, status=HTTP.OK)
        )
        get_or_record_receipt(
            user=self.user1,
            client_event_id="deleted-token-event",
            payload={"position_seconds": 60},
            execute_fn=initial_execute,
            token=token,
        )
        receipt = IntegrationEventReceipt.objects.get(user=self.user1)
        namespace = receipt.client_namespace

        token.delete()
        receipt.refresh_from_db()
        self.assertIsNone(receipt.token_id)
        self.assertEqual(receipt.client_namespace, namespace)
        self.assertEqual(receipt.client_event_id, "deleted-token-event")

        rotated_token, _ = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio rotated",
            client_identifier="nuvio-living-room",
        )
        replay_execute = MagicMock()
        replay, is_replay = get_or_record_receipt(
            user=self.user1,
            client_event_id="deleted-token-event",
            payload={"position_seconds": 60},
            execute_fn=replay_execute,
            token=rotated_token,
        )

        self.assertTrue(is_replay)
        self.assertEqual(replay.data, {"detail": "accepted"})
        replay_execute.assert_not_called()
        self.assertEqual(IntegrationEventReceipt.objects.filter(user=self.user1).count(), 1)

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
                client_namespace="legacy",
                client_event_id="shared-legacy-event",
                token__isnull=True,
            ).exists()
        )
        self.assertTrue(
            IntegrationEventReceipt.objects.filter(
                user=self.user1,
                client_namespace=_expected_namespace(token.token_digest),
                client_event_id="shared-legacy-event",
                token=token,
            ).exists()
        )
