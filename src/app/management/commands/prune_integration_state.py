"""Prune expired integration change history and completed delivery receipts."""

from datetime import timedelta

from django.core.management.base import BaseCommand, CommandError
from django.db import transaction
from django.utils import timezone

from app.models import PlaybackProgressChange, SavedMediaChange, WatchedStateChange
from integrations.models import IntegrationEventReceipt
from integrations.sync_policy import (
    CHANGE_RETENTION_SECONDS,
    COMPLETED_RECEIPT_RETENTION_SECONDS,
    INCOMPLETE_RECEIPT_STATUS,
    PRUNE_BATCH_SIZE,
)


class Command(BaseCommand):
    """Remove old integration state in bounded, database-only batches."""

    help = (
        "Report or remove expired integration change events and completed "
        "idempotency receipts. Dry-run is the default."
    )

    def add_arguments(self, parser):
        """Register explicit write and batch controls."""
        parser.add_argument(
            "--apply",
            action="store_true",
            help="Delete eligible rows. Without this flag, only report counts.",
        )
        parser.add_argument(
            "--batch-size",
            type=int,
            default=PRUNE_BATCH_SIZE,
            help=f"Rows deleted per transaction (default: {PRUNE_BATCH_SIZE}).",
        )

    def handle(self, *args, **options):
        """Report or prune rows older than the supported recovery windows."""
        del args
        batch_size = options["batch_size"]
        if batch_size < 1 or batch_size > 5000:
            raise CommandError("--batch-size must be between 1 and 5000")

        now = timezone.now()
        change_cutoff = now - timedelta(seconds=CHANGE_RETENTION_SECONDS)
        receipt_cutoff = now - timedelta(
            seconds=COMPLETED_RECEIPT_RETENTION_SECONDS,
        )

        targets = (
            (
                "playback_changes",
                PlaybackProgressChange.objects.filter(created_at__lt=change_cutoff),
            ),
            (
                "saved_media_changes",
                SavedMediaChange.objects.filter(created_at__lt=change_cutoff),
            ),
            (
                "watched_state_changes",
                WatchedStateChange.objects.filter(created_at__lt=change_cutoff),
            ),
            (
                "completed_receipts",
                IntegrationEventReceipt.objects.filter(
                    created_at__lt=receipt_cutoff,
                ).exclude(response_status_code=INCOMPLETE_RECEIPT_STATUS),
            ),
        )

        counts = {name: queryset.count() for name, queryset in targets}
        for name, count in counts.items():
            self.stdout.write(f"{name}: {count}")

        if not options["apply"]:
            self.stdout.write("Dry run only. Use --apply to delete eligible rows.")
            return

        deleted = {}
        for name, queryset in targets:
            deleted[name] = self._delete_in_batches(queryset, batch_size)
            self.stdout.write(f"deleted {name}: {deleted[name]}")

    @staticmethod
    def _delete_in_batches(queryset, batch_size):
        """Delete one bounded primary-key batch per transaction until empty."""
        total = 0
        while True:
            primary_keys = list(
                queryset.order_by("pk").values_list("pk", flat=True)[:batch_size]
            )
            if not primary_keys:
                return total
            with transaction.atomic():
                deleted, _details = queryset.model.objects.filter(
                    pk__in=primary_keys,
                ).delete()
            total += deleted
