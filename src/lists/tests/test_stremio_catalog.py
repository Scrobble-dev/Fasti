from datetime import UTC, datetime

from django.contrib.auth import get_user_model
from django.test import TestCase
from django.urls import reverse

from app.models import Item, MediaTypes, Sources
from lists.models import CustomList, CustomListItem


class StremioPublicListContractTests(TestCase):
    """Verify public list publication is exact, bounded, and read-only."""

    def setUp(self):
        self.user = get_user_model().objects.create_user(
            username="stremio-list-owner",
            password="testpassword",
        )
        self.custom_list = CustomList.objects.create(
            name="Living Room Picks",
            description="A public catalog",
            owner=self.user,
            visibility="public",
            public_slug="living-room-picks",
        )

    def _add_item(self, **fields):
        item = Item.objects.create(**fields)
        CustomListItem.objects.create(custom_list=self.custom_list, item=item)
        return item

    def test_manifest_declares_only_supported_stored_types(self):
        self._add_item(
            media_id="603",
            source=Sources.TMDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="The Matrix",
        )
        self._add_item(
            media_id="1396",
            source=Sources.TMDB.value,
            media_type=MediaTypes.TV.value,
            title="Breaking Bad",
        )
        self._add_item(
            media_id="game-1",
            source=Sources.IGDB.value,
            media_type=MediaTypes.GAME.value,
            title="Unsupported Game",
        )

        response = self.client.get(
            reverse("list_stremio_manifest", args=[self.custom_list.public_slug])
        )

        self.assertEqual(response.status_code, 200)
        payload = response.json()
        self.assertEqual(payload["id"], f"dev.floppy.list.{self.custom_list.pk}")
        self.assertEqual(payload["types"], ["movie", "series"])
        self.assertEqual(payload["resources"], ["catalog", "meta"])
        self.assertEqual(
            {(catalog["type"], catalog["id"]) for catalog in payload["catalogs"]},
            {
                ("movie", f"floppy-list-{self.custom_list.pk}"),
                ("series", f"floppy-list-{self.custom_list.pk}"),
            },
        )
        self.assertNotIn("token", response.content.decode().lower())
        self.assertEqual(response["Access-Control-Allow-Origin"], "*")

    def test_catalog_uses_exact_nuvio_compatible_identity_and_list_order(self):
        first = self._add_item(
            media_id="603",
            source=Sources.TMDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="The Matrix",
            release_datetime=datetime(1999, 3, 31, tzinfo=UTC),
            provider_external_ids={"imdb_id": "tt0133093"},
        )
        second = self._add_item(
            media_id="155",
            source=Sources.TMDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="The Dark Knight",
        )
        self.assertLess(
            first.customlistitem_set.get(custom_list=self.custom_list).list_item_id,
            second.customlistitem_set.get(custom_list=self.custom_list).list_item_id,
        )

        response = self.client.get(
            reverse(
                "list_stremio_catalog",
                args=[
                    self.custom_list.public_slug,
                    "movie",
                    f"floppy-list-{self.custom_list.pk}",
                ],
            )
        )

        self.assertEqual(response.status_code, 200)
        metas = response.json()["metas"]
        self.assertEqual([meta["id"] for meta in metas], ["tt0133093", "tmdb:155"])
        self.assertEqual([meta["name"] for meta in metas], ["The Matrix", "The Dark Knight"])
        self.assertEqual(metas[0]["releaseInfo"], "1999")

    def test_catalog_skip_applies_after_exact_identity_filter(self):
        self._add_item(
            media_id="unsupported",
            source=Sources.IGDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="No Supported Exact Identity",
        )
        self._add_item(
            media_id="1",
            source=Sources.TMDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="First Resolved",
        )
        self._add_item(
            media_id="2",
            source=Sources.TMDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="Second Resolved",
        )

        response = self.client.get(
            reverse(
                "list_stremio_catalog_extra",
                args=[
                    self.custom_list.public_slug,
                    "movie",
                    f"floppy-list-{self.custom_list.pk}",
                    "skip=1",
                ],
            )
        )

        self.assertEqual(response.status_code, 200)
        payload = response.json()
        self.assertEqual([meta["name"] for meta in payload["metas"]], ["Second Resolved"])
        self.assertEqual(payload["floppy"]["skipped_unresolved"], 1)
        self.assertEqual(payload["floppy"]["total_resolved"], 2)

    def test_meta_is_exact_and_scoped_to_selected_public_list(self):
        self._add_item(
            media_id="603",
            source=Sources.TMDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="The Matrix",
            provider_external_ids={"imdb_id": "tt0133093"},
            manual_metadata={"synopsis": "Private local text must not leak."},
        )
        other_list = CustomList.objects.create(
            name="Other",
            owner=self.user,
            visibility="public",
        )
        other_item = Item.objects.create(
            media_id="999",
            source=Sources.TMDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="Other Movie",
        )
        CustomListItem.objects.create(custom_list=other_list, item=other_item)

        response = self.client.get(
            reverse(
                "list_stremio_meta",
                args=[self.custom_list.public_slug, "movie", "tt0133093"],
            )
        )
        missing = self.client.get(
            reverse(
                "list_stremio_meta",
                args=[self.custom_list.public_slug, "movie", "tmdb:999"],
            )
        )

        self.assertEqual(response.status_code, 200)
        self.assertEqual(response.json()["meta"]["id"], "tt0133093")
        self.assertNotIn("description", response.json()["meta"])
        self.assertEqual(missing.status_code, 404)

    def test_private_list_revokes_protocol_surface(self):
        self._add_item(
            media_id="603",
            source=Sources.TMDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="The Matrix",
        )
        manifest_url = reverse(
            "list_stremio_manifest",
            args=[self.custom_list.public_slug],
        )
        self.assertEqual(self.client.get(manifest_url).status_code, 200)

        self.custom_list.visibility = "private"
        self.custom_list.save(update_fields=["visibility"])

        self.assertEqual(self.client.get(manifest_url).status_code, 404)

    def test_invalid_skip_fails_bounded_and_does_not_mutate(self):
        item = self._add_item(
            media_id="603",
            source=Sources.TMDB.value,
            media_type=MediaTypes.MOVIE.value,
            title="The Matrix",
        )
        before = CustomListItem.objects.filter(custom_list=self.custom_list).count()

        response = self.client.get(
            reverse(
                "list_stremio_catalog_extra",
                args=[
                    self.custom_list.public_slug,
                    "movie",
                    f"floppy-list-{self.custom_list.pk}",
                    "skip=1000001",
                ],
            )
        )

        self.assertEqual(response.status_code, 400)
        self.assertEqual(
            CustomListItem.objects.filter(custom_list=self.custom_list).count(),
            before,
        )
        self.assertTrue(Item.objects.filter(pk=item.pk).exists())
