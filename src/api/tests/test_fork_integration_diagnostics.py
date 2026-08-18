# FORK: local-only diagnostics coverage for third-party integration state.
from http import HTTPStatus as HTTP  # noqa: N814

from django.urls import reverse

from app.models import MediaTypes, PlaybackProgress, SavedMediaMembership
from integrations.delivery import calculate_payload_digest
from integrations.models import IntegrationEventReceipt, IntegrationToken

from .base import FloppyApiTestCase


class IntegrationDiagnosticsTests(FloppyApiTestCase):
    """Verify diagnostics are private, local, bounded, and non-sensitive."""

    def setUp(self):
        """Start diagnostics requests from the signed-in browser session."""
        super().setUp()
        self.client.force_login(self.user1)

    def _diagnostics(self):
        return self.client.get(reverse("api_integration_diagnostics"))

    def test_scoped_api_token_cannot_read_account_diagnostics(self):
        """A third-party API token cannot enumerate account-level integration state."""
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Nuvio client",
            scopes=["progress:read"],
        )
        self.client.logout()

        response = self.client.get(
            reverse("api_integration_diagnostics"),
            HTTP_X_API_KEY=raw_token,
        )

        self.assertIn(response.status_code, {HTTP.UNAUTHORIZED, HTTP.FORBIDDEN})

    def test_counts_and_recovery_are_scoped_to_signed_in_user(self):
        """Other users' tokens and incomplete receipts never affect this account."""
        token, _raw_token = IntegrationToken.generate(
            user=self.user1,
            name="User one",
            scopes=["progress:read"],
        )
        other_token, _other_raw = IntegrationToken.generate(
            user=self.user2,
            name="User two",
            scopes=["progress:read"],
        )
        IntegrationEventReceipt.objects.create(
            user=self.user1,
            token=token,
            client_event_id="needs-recovery",
            payload_digest=calculate_payload_digest({"position": 10}),
            response_status_code=0,
            response_body={},
        )
        IntegrationEventReceipt.objects.create(
            user=self.user2,
            token=other_token,
            client_event_id="other-user-event",
            payload_digest=calculate_payload_digest({"position": 20}),
            response_status_code=0,
            response_body={},
        )

        response = self._diagnostics()

        self.assertEqual(response.status_code, HTTP.OK)
        payload = response.json()
        self.assertEqual(payload["state"], "needs_attention")
        self.assertEqual(payload["tokens"]["active"], 1)
        self.assertEqual(payload["delivery"]["receipt_count"], 1)
        self.assertEqual(payload["delivery"]["incomplete_receipt_count"], 1)
        self.assertEqual(len(payload["recovery"]), 1)
        self.assertEqual(
            payload["recovery"][0]["code"],
            "incomplete_delivery_receipts",
        )

    def test_response_omits_token_and_receipt_secrets(self):
        """Diagnostics expose health summaries, not credentials or client payloads."""
        token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Sensitive token",
            scopes=["progress:read"],
        )
        event_id = "private-client-event-id"
        payload_marker = "private-response-payload"
        IntegrationEventReceipt.objects.create(
            user=self.user1,
            token=token,
            client_event_id=event_id,
            payload_digest=calculate_payload_digest({"secret": "payload"}),
            response_status_code=HTTP.OK,
            response_body={"marker": payload_marker},
        )

        response = self._diagnostics()

        self.assertEqual(response.status_code, HTTP.OK)
        body = response.content.decode("utf-8")
        self.assertNotIn(raw_token, body)
        self.assertNotIn(token.token_digest, body)
        self.assertNotIn(event_id, body)
        self.assertNotIn(payload_marker, body)

    def test_local_state_counts_do_not_require_external_client_status(self):
        """Local durable state stays inspectable while Nuvio status is unknown."""
        movie_item = self.items_by_type[MediaTypes.MOVIE.value][0]
        PlaybackProgress.objects.create(
            user=self.user1,
            item=movie_item,
            position_seconds=120,
            duration_seconds=3600,
        )
        SavedMediaMembership.objects.create(user=self.user1, item=movie_item)

        response = self._diagnostics()

        self.assertEqual(response.status_code, HTTP.OK)
        payload = response.json()
        self.assertGreaterEqual(payload["progress"]["current_count"], 1)
        self.assertEqual(payload["saved_media"]["current_count"], 1)
        self.assertEqual(payload["external_client_sync"]["state"], "not_reported")
        self.assertFalse(payload["offline"]["diagnostics_require_external_network"])
        self.assertFalse(
            payload["offline"][
                "existing_identity_progress_writes_require_external_network"
            ]
        )
