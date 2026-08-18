from django.test import SimpleTestCase

from integrations.content_classification import ExternalContentClass
from integrations.workspace_contract import (
    WorkspaceValidationError,
    parse_workspace_collection,
)


class WorkspaceCollectionContractTests(SimpleTestCase):
    def _descriptor(self, **overrides):
        payload = {
            "schemaVersion": "1.0",
            "id": "collection.favorites",
            "title": "Favorites",
            "folders": [
                {
                    "id": "movies",
                    "title": "Movies",
                    "sources": [
                        {
                            "kind": "addon-catalog",
                            "addonId": "dev.floppy.list.42",
                            "type": "movie",
                            "catalogId": "floppy-list-42",
                        }
                    ],
                }
            ],
        }
        payload.update(overrides)
        return payload

    def test_addon_catalog_source_round_trips_nuvio_compatible_fields(self):
        collection = parse_workspace_collection(self._descriptor())

        self.assertEqual(collection.collection_id, "collection.favorites")
        self.assertEqual(collection.folders[0].folder_id, "movies")
        source = collection.folders[0].sources[0]
        self.assertEqual(source.addon_id, "dev.floppy.list.42")
        self.assertEqual(source.content_type.classification, ExternalContentClass.MOVIE)
        self.assertEqual(source.catalog_id, "floppy-list-42")
        self.assertEqual(collection.as_dict(), self._descriptor())

    def test_unknown_source_classifier_is_preserved_not_guessed(self):
        payload = self._descriptor()
        payload["folders"][0]["sources"][0]["type"] = "anime-special"

        collection = parse_workspace_collection(payload)
        source = collection.folders[0].sources[0]

        self.assertEqual(source.content_type.classification, ExternalContentClass.UNKNOWN)
        self.assertEqual(source.content_type.raw_type, "anime-special")
        self.assertEqual(source.as_dict()["type"], "anime-special")

    def test_optional_genre_round_trips(self):
        payload = self._descriptor()
        payload["folders"][0]["sources"][0]["genre"] = "Drama"

        collection = parse_workspace_collection(payload)

        self.assertEqual(collection.as_dict(), payload)

    def test_rejects_secret_or_executable_source_fields(self):
        for field, value in {
            "configuredUrl": "https://addon.example/private-token",
            "token": "secret",
            "password": "secret",
            "plugin": {"runtime": "js"},
            "code": "doEvil()",
            "items": [{"id": "tt1"}],
        }.items():
            with self.subTest(field=field):
                payload = self._descriptor()
                payload["folders"][0]["sources"][0][field] = value
                with self.assertRaisesRegex(
                    WorkspaceValidationError,
                    "must remain outside",
                ) as raised:
                    parse_workspace_collection(payload)
                self.assertEqual(raised.exception.code, "unsafe_workspace_source")

    def test_rejects_non_declarative_source_kind(self):
        payload = self._descriptor()
        payload["folders"][0]["sources"][0]["kind"] = "plugin"

        with self.assertRaisesRegex(WorkspaceValidationError, "declarative add-on") as raised:
            parse_workspace_collection(payload)

        self.assertEqual(raised.exception.code, "unsupported_workspace_source")

    def test_rejects_future_schema_until_explicitly_supported(self):
        with self.assertRaisesRegex(WorkspaceValidationError, "schema version") as raised:
            parse_workspace_collection(self._descriptor(schemaVersion="2.0"))

        self.assertEqual(raised.exception.code, "unsupported_workspace_version")

    def test_folder_and_source_counts_are_bounded(self):
        too_many_folders = [
            {"id": f"folder-{index}", "title": "Folder", "sources": []}
            for index in range(129)
        ]
        with self.assertRaisesRegex(WorkspaceValidationError, "too many entries"):
            parse_workspace_collection(self._descriptor(folders=too_many_folders))

        payload = self._descriptor()
        payload["folders"][0]["sources"] = [
            {
                "kind": "addon-catalog",
                "addonId": "dev.floppy.list.42",
                "type": "movie",
                "catalogId": f"catalog-{index}",
            }
            for index in range(129)
        ]
        with self.assertRaisesRegex(WorkspaceValidationError, "too many entries"):
            parse_workspace_collection(payload)
