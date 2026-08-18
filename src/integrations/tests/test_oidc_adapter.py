from unittest.mock import patch

from allauth.idp.oidc.adapter import DefaultOIDCAdapter
from django.core.exceptions import ValidationError
from django.test import SimpleTestCase

from integrations.oidc_adapter import FloppyOIDCAdapter


class FloppyOIDCAdapterTests(SimpleTestCase):
    """Keep delegated resource validation aligned with allauth and Floppy."""

    def test_resource_validation_preserves_allauth_checks(self):
        adapter = FloppyOIDCAdapter()
        resource = "https://floppy.example/api/v1/"

        with (
            patch.object(
                DefaultOIDCAdapter,
                "validate_resource_uris",
                autospec=True,
            ) as upstream_validation,
            patch.object(
                adapter,
                "get_issuer",
                return_value="https://floppy.example",
            ),
        ):
            adapter.validate_resource_uris(uris=[resource])

        upstream_validation.assert_called_once_with(adapter, uris=[resource])

    def test_resource_validation_rejects_cross_origin_target(self):
        adapter = FloppyOIDCAdapter()

        with (
            patch.object(
                DefaultOIDCAdapter,
                "validate_resource_uris",
                autospec=True,
            ),
            patch.object(
                adapter,
                "get_issuer",
                return_value="https://floppy.example",
            ),
            self.assertRaises(ValidationError),
        ):
            adapter.validate_resource_uris(
                uris=["https://attacker.example/api/v1/"],
            )

    def test_resource_validation_accepts_same_origin_path(self):
        adapter = FloppyOIDCAdapter()

        with (
            patch.object(
                DefaultOIDCAdapter,
                "validate_resource_uris",
                autospec=True,
            ),
            patch.object(
                adapter,
                "get_issuer",
                return_value="http://floppy.local:8000",
            ),
        ):
            adapter.validate_resource_uris(
                uris=["http://floppy.local:8000/api/v1/playback/progress"],
            )
