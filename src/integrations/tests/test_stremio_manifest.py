import hashlib
from unittest.mock import patch

from django.test import SimpleTestCase

from integrations.safe_fetch import SafeFetchResult
from integrations.stremio_manifest import (
    ManifestValidationError,
    load_manifest,
    parse_manifest,
)


class DeclarativeManifestTests(SimpleTestCase):
    """Verify Stremio manifests become bounded declarative capability data."""

    def _manifest(self, **overrides):
        payload = {
            "id": "example.catalog",
            "name": "Example Catalog",
            "version": "1.2.3",
            "description": "Catalog and metadata only.",
            "types": ["movie", "series", "anime"],
            "resources": ["catalog", "meta"],
            "catalogs": [
                {
                    "type": "movie",
                    "id": "popular",
                    "name": "Popular",
                    "extra": [
                        {
                            "name": "genre",
                            "isRequired": False,
                            "options": ["Drama", "Comedy"],
                        }
                    ],
                }
            ],
        }
        payload.update(overrides)
        return payload

    def test_preserves_raw_classifiers_and_consumable_capabilities(self):
        manifest = parse_manifest(
            self._manifest(),
            origin="https://addon.example",
            configuration_digest="abc123",
        )

        self.assertEqual(manifest.raw_types, ("movie", "series", "anime"))
        self.assertEqual(manifest.declared_resources, ("catalog", "meta"))
        self.assertEqual(manifest.consumable_resources, ("catalog", "meta"))
        self.assertEqual(manifest.ignored_resources, ())
        self.assertTrue(manifest.provides_catalogs)
        self.assertTrue(manifest.provides_metadata)
        self.assertEqual(manifest.catalogs[0].content_type, "movie")
        self.assertEqual(manifest.catalogs[0].extras[0].options, ("Drama", "Comedy"))

    def test_stream_and_subtitle_resources_are_never_consumable(self):
        manifest = parse_manifest(
            self._manifest(resources=["catalog", "stream", "subtitles", "meta"]),
            origin="https://addon.example",
            configuration_digest="abc123",
        )

        self.assertEqual(manifest.consumable_resources, ("catalog", "meta"))
        self.assertEqual(set(manifest.ignored_resources), {"stream", "subtitles"})

    def test_resource_object_syntax_is_supported_without_copying_runtime_details(self):
        manifest = parse_manifest(
            self._manifest(
                resources=[
                    {
                        "name": "catalog",
                        "types": ["movie"],
                        "idPrefixes": ["tt"],
                    },
                    {"name": "meta", "types": ["movie"]},
                ]
            ),
            origin="https://addon.example",
            configuration_digest="abc123",
        )

        self.assertEqual(manifest.declared_resources, ("catalog", "meta"))

    def test_manifest_without_safe_consumable_resource_is_rejected(self):
        with self.assertRaisesRegex(ManifestValidationError, "catalog or metadata") as raised:
            parse_manifest(
                self._manifest(resources=["stream", "subtitles"]),
                origin="https://addon.example",
                configuration_digest="abc123",
            )

        self.assertEqual(raised.exception.code, "unsupported_manifest")
        self.assertEqual(raised.exception.param, "resources")

    def test_manifest_limits_catalogs_and_extras(self):
        too_many_catalogs = [
            {"type": "movie", "id": f"catalog-{index}", "name": "Catalog"}
            for index in range(257)
        ]
        with self.assertRaisesRegex(ManifestValidationError, "too many entries"):
            parse_manifest(
                self._manifest(catalogs=too_many_catalogs),
                origin="https://addon.example",
                configuration_digest="abc123",
            )

        too_many_extras = [
            {"name": f"extra-{index}"}
            for index in range(33)
        ]
        with self.assertRaisesRegex(ManifestValidationError, "too many entries"):
            parse_manifest(
                self._manifest(
                    catalogs=[
                        {
                            "type": "movie",
                            "id": "popular",
                            "name": "Popular",
                            "extra": too_many_extras,
                        }
                    ]
                ),
                origin="https://addon.example",
                configuration_digest="abc123",
            )

    @patch("integrations.stremio_manifest.fetch_json")
    def test_load_manifest_retains_only_safe_origin_and_configuration_digest(self, fetch_json):
        configured_url = "https://addon.example/secret-token/manifest.json?user=private"
        fetch_json.return_value = (
            self._manifest(),
            SafeFetchResult(
                status_code=200,
                content_type="application/json",
                body=b"{}",
                origin="https://addon.example",
            ),
        )

        manifest = load_manifest(configured_url)

        self.assertEqual(manifest.origin, "https://addon.example")
        self.assertEqual(
            manifest.configuration_digest,
            hashlib.sha256(configured_url.encode()).hexdigest(),
        )
        serialized = repr(manifest)
        self.assertNotIn("secret-token", serialized)
        self.assertNotIn("user=private", serialized)

    def test_manifest_rejects_non_scalar_classifier_and_required_extra_flag(self):
        with self.assertRaises(ManifestValidationError) as type_error:
            parse_manifest(
                self._manifest(types=[{"name": "movie"}]),
                origin="https://addon.example",
                configuration_digest="abc123",
            )
        self.assertEqual(type_error.exception.param, "types[0]")

        with self.assertRaises(ManifestValidationError) as flag_error:
            parse_manifest(
                self._manifest(
                    catalogs=[
                        {
                            "type": "movie",
                            "id": "popular",
                            "name": "Popular",
                            "extra": [{"name": "genre", "isRequired": "yes"}],
                        }
                    ]
                ),
                origin="https://addon.example",
                configuration_digest="abc123",
            )
        self.assertEqual(
            flag_error.exception.param,
            "catalogs[0].extra[0].isRequired",
        )
