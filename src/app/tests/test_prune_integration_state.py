from datetime import timedelta
from io import StringIO

from django.contrib.auth import get_user_model
from django.core.management import call_command
from django.test import TestCase
from django.utils import timezone

from app.models import (
    Item,
    PlaybackProgressChange,
    SavedMediaChange,
    WatchedStateChange,
)
from integrations.models import IntegrationEventReceipt
from integrations.sync_policy import (
    CHANGE_RETENTION_SECONDS,
    COMPLETED_RECEIPT_RETENTION_SECONDS,
    INCOMPLETE_RECEIPT_STATUS,
)


class PruneIntegrationStateTests(TestCase):
    """Integration lifecycle pruning must preserve recoverable state."""

    def setUp(self):
        self.user = get_user_model().objects.create_user(
            username="prune-user",
            password="test-password",
        )
        self.item = Item.objects.create(
            media_id="prune-movie",
            source="tmdb",
            media_type="movie",
            title="Prune Movie",
        )

    def _make_old_change_rows(self):
        playback = PlaybackProgressChange.objects.create(
            user=self.user,
            item=self.item,
            operation="upsert",
            media_type="movie",
            source="tmdb",
            media_id=self.item.media_id,
            ids={"tmdb": self.item.media_id},
            position_seconds=20,
            duration_seconds=100,
            completed=False,
        )
        saved = SavedMediaChange.objects.create(
            user=self.user,
            item=self.item,
            operation="add",
            media_type="movie",
            source="tmdb",
            media_id=self.item.media_id,
            ids={"tmdb": self.item.media_id},
        )
        watched = WatchedStateChange.objects.create(
            user=self.user,
            item=self.item,
            watched=True,
            media_type="movie",
            source="tmdb",
            media_id=self.item.media_id,
            ids={"tmdb": self.item.media_id},
        )
        cutoff = timezone.now() - timedelta(seconds=CHANGE_RETENTION_SECONDS + 60)
        for model, row in (
            (PlaybackProgressChange, playback),
            (SavedMediaChange, saved),
            (WatchedStateChange, watched),
        ):
            model.objects.filter(pk=row.pk).update(created_at=cutoff)

    def _make_receipts(self):
        old = timezone.now() - timedelta(
            seconds=COMPLETED_RECEIPT_RETENTION_SECONDS + 60,
        )
        completed = IntegrationEventReceipt.objects.create(
            user=self.user,
            client_event_id="completed-old",
            payload_digest="a" * 64,
            response_status_code=200,
            response_body={"ok": True},
        )
        incomplete = IntegrationEventReceipt.objects.create(
            user=self.user,
            client_event_id="incomplete-old",
            payload_digest="b" * 64,
            response_status_code=INCOMPLETE_RECEIPT_STATUS,
            response_body={},
        )
        IntegrationEventReceipt.objects.filter(
            pk__in=[completed.pk, incomplete.pk]
        ).update(created_at=old)

    def test_dry_run_does_not_delete_rows(self):
        self._make_old_change_rows()
        self._make_receipts()
        output = StringIO()

        call_command("prune_integration_state", stdout=output)

        self.assertEqual(PlaybackProgressChange.objects.count(), 1)
        self.assertEqual(SavedMediaChange.objects.count(), 1)
        self.assertEqual(WatchedStateChange.objects.count(), 1)
        self.assertEqual(IntegrationEventReceipt.objects.count(), 2)
        self.assertIn("Dry run only", output.getvalue())

    def test_apply_removes_old_changes_and_completed_receipts_only(self):
        self._make_old_change_rows()
        self._make_receipts()

        call_command("prune_integration_state", "--apply", "--batch-size", "1")

        self.assertFalse(PlaybackProgressChange.objects.exists())
        self.assertFalse(SavedMediaChange.objects.exists())
        self.assertFalse(WatchedStateChange.objects.exists())
        self.assertFalse(
            IntegrationEventReceipt.objects.filter(
                client_event_id="completed-old",
            ).exists()
        )
        self.assertTrue(
            IntegrationEventReceipt.objects.filter(
                client_event_id="incomplete-old",
                response_status_code=INCOMPLETE_RECEIPT_STATUS,
            ).exists()
        )
