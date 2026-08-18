# FORK: canonical authorization contract shared by all integration credentials.

from django.test import SimpleTestCase

from integrations.scopes import (
    INTEGRATION_SCOPE_RULES,
    INTEGRATION_SCOPES,
    NUVIO_RECOMMENDED_SCOPES,
    normalize_requested_scopes,
    required_scope_for_route,
)


class IntegrationScopeContractTests(SimpleTestCase):
    """Keep every integration credential on the same fail-closed scope contract."""

    def test_every_route_rule_uses_a_declared_scope(self):
        """Route rules cannot reference undocumented permissions."""
        for method_scopes in INTEGRATION_SCOPE_RULES.values():
            for scope in method_scopes.values():
                self.assertIn(scope, INTEGRATION_SCOPES)

    def test_nuvio_recommended_scopes_are_declared(self):
        """The Nuvio preset cannot silently mint unknown authority."""
        self.assertTrue(set(NUVIO_RECOMMENDED_SCOPES).issubset(INTEGRATION_SCOPES))

    def test_unknown_route_and_method_fail_closed(self):
        """Scoped credentials are denied when no authorization rule exists."""
        self.assertEqual(required_scope_for_route("unknown_route", "GET"), (False, None))
        self.assertEqual(required_scope_for_route("api_watchlist", "POST"), (False, None))
        self.assertEqual(required_scope_for_route(None, "GET"), (False, None))

    def test_options_is_allowed_only_for_a_declared_route(self):
        """Preflight cannot turn an unknown route into a scoped API surface."""
        self.assertEqual(required_scope_for_route("api_watchlist", "OPTIONS"), (True, None))
        self.assertEqual(required_scope_for_route("unknown_route", "OPTIONS"), (False, None))

    def test_requested_scopes_are_deduplicated_without_widening(self):
        """Token creation preserves explicit least-privilege requests."""
        scopes, unsupported = normalize_requested_scopes(
            ["progress:read", "progress:read", "watched:read"]
        )
        self.assertEqual(scopes, ["progress:read", "watched:read"])
        self.assertIsNone(unsupported)

    def test_requested_scopes_reject_unknown_or_invalid_values(self):
        """Wildcard, unknown, and malformed requests never become valid grants."""
        scopes, unsupported = normalize_requested_scopes(["progress:read", "*"])
        self.assertEqual(scopes, ["progress:read", "*"])
        self.assertEqual(unsupported, "*")

        self.assertEqual(normalize_requested_scopes([]), (None, None))
        self.assertEqual(normalize_requested_scopes("progress:read"), (None, None))
        self.assertEqual(normalize_requested_scopes(["progress:read", 1]), (None, None))
