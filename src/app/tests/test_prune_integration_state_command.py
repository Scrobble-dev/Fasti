"""Regression coverage for bounded integration-state retention."""

from datetime import timedelta
from io import StringIO

from django.contrib.auth import get_user_model
from django.core.management import call_command
from django.core.management.base import CommandError
from django.test import TestCase
from django.utils import timezone

from app.models import PlaybackProgressChange, SavedMediaChange, WatchedStateChange
from integrations.models import IntegrationEventReceipt
from integrations.sync_policy import (
    CHANGE_RETENTION_DAYS,
    COMPLETED_RECEIPT_RETENTION_DAYS,
    MAX_PRUNE_BATCH_SIZE,
)


class PruneIntegrationStateCommandTests(TestCase):
    """Keep cleanup explicit, bounded, and safe for uncertain receipts."""

    def setUp(self):
        """Create one user and representative durable integration rows."""
        self.user = get_user_model().objects.create_user(
            username="retention-user",
            password="test-password",
        )
        now = timezone.now()
        old_change_time = now - timedelta(days=CHANGE_RETENTION_DAYS + 1)
        old_receipt_time = now - timedelta(
            days=COMPLETED_RECEIPT_RETENTION_DAYS + 1
        )

        self.old_progress = PlaybackProgressChange.objects.create(
            user=self.user,
            item_id=1,
            operation=PlaybackProgressChange.Operation.UPSERT,
            media_type="movie",
            source="tmdb",
            media_id="1",
            ids={"tmdb": "1"},
            position_seconds=120,
        )
        PlaybackProgressChange.objects.filter(pk=self.old_progress.pk).update(
            created_at=old_change_time
        )
        self.recent_progress = PlaybackProgressChange.objects.create(
            user=self.user,
            item_id=2,
            operation=PlaybackProgressChange.Operation.UPSERT,
            media_type="movie",
            source="tmdb",
            media_id="2",
            ids={"tmdb": "2"},
            position_seconds=240,
        )

        self.old_saved = SavedMediaChange.objects.create(
            user=self.user,
            item_id=3,
            operation=SavedMediaChange.Operation.ADD,
            media_type="movie",
            source="tmdb",
            media_id="3",
            ids={"tmdb": "3"},
        )
        SavedMediaChange.objects.filter(pk=self.old_saved.pk).update(
            created_at=old_change_time
        )
        self.recent_saved = SavedMediaChange.objects.create(
            user=self.user,
            item_id=4,
            operation=SavedMediaChange.Operation.ADD,
            media_type="movie",
            source="tmdb",
            media_id="4",
            ids={"tmdb": "4"},
        )

        self.old_watched = WatchedStateChange.objects.create(
            user=self.user,
            item_id=5,
            watched=True,
            media_type="movie",
            source="tmdb",
            media_id="5",
            ids={"tmdb": "5"},
        )
        WatchedStateChange.objects.filter(pk=self.old_watched.pk).update(
            created_at=old_change_time
        )
        self.recent_watched = WatchedStateChange.objects.create(
            user=self.user,
            item_id=6,
            watched=False,
            media_type="movie",
            source="tmdb",
            media_id="6",
            ids={"tmdb": "6"},
        )

        self.old_completed_receipt = IntegrationEventReceipt.objects.create(
            user=self.user,
            client_event_id="old-completed",
            payload_digest="a" * 64,
            response_status_code=200,
            response_body={"ok": True},
        )
        IntegrationEventReceipt.objects.filter(
            pk=self.old_completed_receipt.pk
        ).update(created_at=old_receipt_time)

        self.old_incomplete_receipt = IntegrationEventReceipt.objects.create(
            user=self.user,
            client_event_id="old-incomplete",
            payload_digest="b" * 64,
            response_status_code=0,
            response_body={},
        )
        IntegrationEventReceipt.objects.filter(
            pk=self.old_incomplete_receipt.pk
        ).update(created_at=old_receipt_time)

        self.recent_completed_receipt = IntegrationEventReceipt.objects.create(
            user=self.user,
            client_event_id="recent-completed",
            payload_digest="c" * 64,
            response_status_code=204,
            response_body={},
        )

    def test_default_run_is_read_only(self):
        """The command reports eligible state but never deletes without --apply."""
        output = StringIO()
        call_command("prune_integration_state", stdout=output)

        self.assertTrue(
            PlaybackProgressChange.objects.filter(pk=self.old_progress.pk).exists()
        )
        self.assertTrue(SavedMediaChange.objects.filter(pk=self.old_saved.pk).exists())
        self.assertTrue(
            WatchedStateChange.objects.filter(pk=self.old_watched.pk).exists()
        )
        self.assertTrue(
            IntegrationEventReceipt.objects.filter(
                pk=self.old_completed_receipt.pk
            ).exists()
        )
        self.assertIn("watched-state changes: 1", output.getvalue())
        self.assertIn("Dry run only", output.getvalue())

    def test_apply_prunes_only_expired_safe_rows(self):
        """Expired changes and completed receipts are deleted in bounded batches."""
        call_command("prune_integration_state", "--apply", "--batch-size", "1")

        self.assertFalse(
            PlaybackProgressChange.objects.filter(pk=self.old_progress.pk).exists()
        )
        self.assertTrue(
            PlaybackProgressChange.objects.filter(pk=self.recent_progress.pk).exists()
        )
        self.assertFalse(
            SavedMediaChange.objects.filter(pk=self.old_saved.pk).exists()
        )
        self.assertTrue(
            SavedMediaChange.objects.filter(pk=self.recent_saved.pk).exists()
        )
        self.assertFalse(
            WatchedStateChange.objects.filter(pk=self.old_watched.pk).exists()
        )
        self.assertTrue(
            WatchedStateChange.objects.filter(pk=self.recent_watched.pk).exists()
        )
        self.assertFalse(
            IntegrationEventReceipt.objects.filter(
                pk=self.old_completed_receipt.pk
            ).exists()
        )
        self.assertTrue(
            IntegrationEventReceipt.objects.filter(
                pk=self.old_incomplete_receipt.pk
            ).exists()
        )
        self.assertTrue(
            IntegrationEventReceipt.objects.filter(
                pk=self.recent_completed_receipt.pk
            ).exists()
        )

    def test_batch_size_is_bounded(self):
        """Invalid batch sizes fail before any state can be changed."""
        with self.assertRaises(CommandError):
            call_command(
                "prune_integration_state",
                "--apply",
                "--batch-size",
                str(MAX_PRUNE_BATCH_SIZE + 1),
            )
        with self.assertRaises(CommandError):
            call_command(
                "prune_integration_state",
                "--apply",
                "--batch-size",
                "0",
            )
