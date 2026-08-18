from types import SimpleNamespace

from django.test import SimpleTestCase

from integrations.allauth_oidc import FloppyOIDCAdapter
from integrations.scopes import INTEGRATION_SCOPES


class FloppyOIDCAdapterTests(SimpleTestCase):
    """Keep allauth delegated authorization aligned with Floppy scopes."""

    def test_scope_display_contains_canonical_integration_scopes(self):
        """Consent UI cannot drift from the API authorization vocabulary."""
        self.assertTrue(INTEGRATION_SCOPES.issubset(FloppyOIDCAdapter.scope_display))
        self.assertIn("openid", FloppyOIDCAdapter.scope_display)
        self.assertIn("profile", FloppyOIDCAdapter.scope_display)
        self.assertIn("email", FloppyOIDCAdapter.scope_display)

    def test_access_token_claims_only_add_client_identity(self):
        """The adapter does not copy media or provider state into auth claims."""
        claims = {"sub": "user-1"}
        client = SimpleNamespace(pk="nuvio-tv")

        FloppyOIDCAdapter().populate_access_token(
            claims,
            client=client,
            scopes=["progress:read"],
            user=SimpleNamespace(pk=1),
        )

        self.assertEqual(
            claims,
            {
                "sub": "user-1",
                "floppy_client_id": "nuvio-tv",
            },
        )
