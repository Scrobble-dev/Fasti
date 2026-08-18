# FORK: account-session lifecycle coverage for scoped integration credentials.
import hashlib
from http import HTTPStatus as HTTP  # noqa: N814

from django.urls import reverse

from integrations.models import IntegrationToken

from .base import FloppyApiTestCase


class IntegrationTokenManagementTests(FloppyApiTestCase):
    """Verify user-managed scoped credentials remain least privilege."""

    def setUp(self):
        """Start token-management requests from a signed-in browser session."""
        super().setUp()
        self.client.force_login(self.user1)

    def test_create_returns_secret_once_and_never_returns_digest(self):
        """The full secret appears only in the create response."""
        response = self.client.post(
            reverse("api_integration_tokens"),
            data={
                "name": "Nuvio TV",
                "client_identifier": "living-room-tv",
                "scopes": ["progress:read", "watchlist:read", "watched:read"],
                "expires_in_days": 90,
            },
            content_type="application/json",
        )

        self.assertEqual(response.status_code, HTTP.CREATED)
        payload = response.json()
        raw_token = payload["token"]
        self.assertTrue(raw_token.startswith("flp_"))
        self.assertTrue(payload["secret_shown_once"])
        self.assertEqual(response.headers["Cache-Control"], "private, no-store")
        self.assertEqual(response.headers["Pragma"], "no-cache")
        token = IntegrationToken.objects.get(user=self.user1, name="Nuvio TV")
        self.assertEqual(
            token.token_digest,
            hashlib.sha256(raw_token.encode("utf-8")).hexdigest(),
        )
        self.assertNotIn("token_digest", payload)

        listed = self.client.get(reverse("api_integration_tokens"))
        self.assertEqual(listed.status_code, HTTP.OK)
        self.assertEqual(listed.headers["Cache-Control"], "private, no-store")
        self.assertEqual(listed.headers["Pragma"], "no-cache")
        listed_payload = listed.json()["results"][0]
        self.assertNotIn("token", listed_payload)
        self.assertNotIn("token_digest", listed_payload)
        self.assertEqual(listed_payload["token_prefix"], token.token_prefix)

    def test_unsupported_scope_is_rejected(self):
        """The account lifecycle API cannot mint wildcard or unknown scopes."""
        response = self.client.post(
            reverse("api_integration_tokens"),
            data={"name": "Too broad", "scopes": ["*"]},
            content_type="application/json",
        )

        self.assertEqual(response.status_code, HTTP.BAD_REQUEST)
        self.assertEqual(response.json()["error"]["code"], "invalid_scopes")
        self.assertEqual(response.headers["Cache-Control"], "private, no-store")
        self.assertFalse(IntegrationToken.objects.filter(user=self.user1).exists())

    def test_scoped_token_cannot_manage_other_credentials(self):
        """Integration credentials cannot mint persistent credentials for themselves."""
        _token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Existing client",
            scopes=["progress:read"],
        )
        self.client.logout()

        response = self.client.get(
            reverse("api_integration_tokens"),
            HTTP_X_API_KEY=raw_token,
        )

        self.assertIn(response.status_code, {HTTP.UNAUTHORIZED, HTTP.FORBIDDEN})

    def test_other_user_cannot_revoke_token(self):
        """Sequential token IDs do not permit cross-user revocation."""
        token, _raw_token = IntegrationToken.generate(
            user=self.user1,
            name="User one token",
            scopes=["progress:read"],
        )
        self.client.force_login(self.user2)

        response = self.client.delete(
            reverse("api_integration_token_detail", kwargs={"token_id": token.pk})
        )

        self.assertEqual(response.status_code, HTTP.NOT_FOUND)
        self.assertEqual(response.headers["Cache-Control"], "private, no-store")
        token.refresh_from_db()
        self.assertIsNone(token.revoked_at)

    def test_revoke_is_idempotent_and_denies_future_api_use(self):
        """Revocation keeps the token record but immediately invalidates the secret."""
        token, raw_token = IntegrationToken.generate(
            user=self.user1,
            name="Revocable",
            scopes=["watched:read"],
        )
        url = reverse("api_integration_token_detail", kwargs={"token_id": token.pk})

        first = self.client.delete(url)
        second = self.client.delete(url)

        self.assertEqual(first.status_code, HTTP.NO_CONTENT)
        self.assertEqual(second.status_code, HTTP.NO_CONTENT)
        self.assertEqual(first.headers["Cache-Control"], "private, no-store")
        self.assertEqual(second.headers["Cache-Control"], "private, no-store")
        token.refresh_from_db()
        self.assertIsNotNone(token.revoked_at)

        self.client.logout()
        denied = self.call_api(
            "get",
            "api_watched_state",
            params={"media_type": "movie"},
            headers={"HTTP_X_API_KEY": raw_token},
        )
        self.assertIn(denied.status_code, {HTTP.UNAUTHORIZED, HTTP.FORBIDDEN})

    def test_demo_account_cannot_create_token(self):
        """Demo accounts remain view-only for credential management."""
        self.user1.is_demo = True
        self.user1.save(update_fields=["is_demo"])

        response = self.client.post(
            reverse("api_integration_tokens"),
            data={"name": "Demo token", "scopes": ["progress:read"]},
            content_type="application/json",
        )

        self.assertEqual(response.status_code, HTTP.FORBIDDEN)
        self.assertEqual(response.headers["Cache-Control"], "private, no-store")
        self.assertFalse(IntegrationToken.objects.filter(user=self.user1).exists())
