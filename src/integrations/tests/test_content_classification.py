from django.test import SimpleTestCase

from app.models import MediaTypes
from integrations.content_classification import (
    ExternalContentClass,
    classify_external_content_type,
    floppy_catalog_content_type,
    floppy_catalog_media_types,
)


class ExternalContentClassificationTests(SimpleTestCase):
    def test_known_nuvio_classifiers_are_case_insensitive(self):
        expected = {
            "movie": ExternalContentClass.MOVIE,
            " SERIES ": ExternalContentClass.SERIES,
            "channel": ExternalContentClass.CHANNEL,
            "TV": ExternalContentClass.TV,
        }
        for raw_type, classification in expected.items():
            with self.subTest(raw_type=raw_type):
                result = classify_external_content_type(raw_type)
                self.assertEqual(result.classification, classification)
                self.assertEqual(result.api_type, classification.value)

    def test_unknown_classifier_preserves_raw_value_without_guessing(self):
        result = classify_external_content_type(" anime-special ")

        self.assertEqual(result.classification, ExternalContentClass.UNKNOWN)
        self.assertEqual(result.raw_type, "anime-special")
        self.assertEqual(result.api_type, "anime-special")

    def test_empty_unknown_classifier_uses_protocol_fallback_only_for_display(self):
        result = classify_external_content_type(None)

        self.assertEqual(result.classification, ExternalContentClass.UNKNOWN)
        self.assertEqual(result.raw_type, "")
        self.assertEqual(result.api_type, "movie")

    def test_floppy_catalog_mapping_is_screen_media_only(self):
        self.assertEqual(
            floppy_catalog_content_type(MediaTypes.MOVIE.value),
            ExternalContentClass.MOVIE,
        )
        self.assertEqual(
            floppy_catalog_content_type(MediaTypes.TV.value),
            ExternalContentClass.SERIES,
        )
        self.assertEqual(
            floppy_catalog_content_type(MediaTypes.ANIME.value),
            ExternalContentClass.SERIES,
        )
        self.assertIsNone(floppy_catalog_content_type(MediaTypes.BOOK.value))
        self.assertIsNone(floppy_catalog_content_type(MediaTypes.GAME.value))

    def test_series_catalog_contains_tv_and_anime_only(self):
        self.assertEqual(
            floppy_catalog_media_types("series"),
            (MediaTypes.TV.value, MediaTypes.ANIME.value),
        )
        self.assertEqual(
            floppy_catalog_media_types("movie"),
            (MediaTypes.MOVIE.value,),
        )
        self.assertEqual(floppy_catalog_media_types("channel"), ())
        self.assertEqual(floppy_catalog_media_types("podcast"), ())
