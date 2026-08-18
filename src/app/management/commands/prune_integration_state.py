"""Prune expired third-party synchronization state in bounded batches."""

from datetime import timedelta

from django.core.management.base import BaseCommand, CommandError
from django.utils import timezone

from app.models import (
    PlaybackProgressChange,
    SavedMediaChange,
    WatchedStateChange,
)
from integrations.models import IntegrationEventReceipt
from integrations.sync_policy import (
    CHANGE_RETENTION_DAYS,
    COMPLETED_RECEIPT_RETENTION_DAYS,
    DEFAULT_PRUNE_BATCH_SIZE,
    INCOMPLETE_RECEIPT_STATUS,
    MAX_PRUNE_BATCH_SIZE,
)


class Command(BaseCommand):
    """Report or prune synchronization state that is outside its recovery window."""

    help = (
        "Report expired integration state. Use --apply to delete it in bounded "
        "batches. Incomplete idempotency receipts are always preserved."
    )

    def add_arguments(self, parser):
        """Add safe maintenance controls."""
        parser.add_argument(
            "--apply",
            action="store_true",
            help="Delete eligible rows. Without this flag the command is read-only.",
        )
        parser.add_argument(
            "--batch-size",
            type=int,
            default=DEFAULT_PRUNE_BATCH_SIZE,
            help=(
                "Rows processed per delete batch. Maximum: "
                f"{MAX_PRUNE_BATCH_SIZE}."
            ),
        )

    def handle(self, *args, **options):
        """Report eligible state and optionally prune it."""
        batch_size = options["batch_size"]
        if batch_size < 1 or batch_size > MAX_PRUNE_BATCH_SIZE:
            raise CommandError(
                f"--batch-size must be between 1 and {MAX_PRUNE_BATCH_SIZE}."
            )

        now = timezone.now()
        change_cutoff = now - timedelta(days=CHANGE_RETENTION_DAYS)
        receipt_cutoff = now - timedelta(days=COMPLETED_RECEIPT_RETENTION_DAYS)
        apply_changes = options["apply"]

        progress_qs = PlaybackProgressChange.objects.filter(
            created_at__lt=change_cutoff
        )
        saved_qs = SavedMediaChange.objects.filter(created_at__lt=change_cutoff)
        watched_qs = WatchedStateChange.objects.filter(created_at__lt=change_cutoff)

        if apply_changes:
            progress_count = self._delete_queryset(progress_qs, batch_size)
            saved_count = self._delete_queryset(saved_qs, batch_size)
            watched_count = self._delete_queryset(watched_qs, batch_size)
            receipt_count = self._scan_receipts(
                receipt_cutoff,
                batch_size,
                delete=True,
            )
            action = "Deleted"
        else:
            progress_count = progress_qs.count()
            saved_count = saved_qs.count()
            watched_count = watched_qs.count()
            receipt_count = self._scan_receipts(
                receipt_cutoff,
                batch_size,
                delete=False,
            )
            action = "Would delete"

        self.stdout.write(
            f"{action} playback progress changes: {progress_count}"
        )
        self.stdout.write(f"{action} saved-media changes: {saved_count}")
        self.stdout.write(f"{action} watched-state changes: {watched_count}")
        self.stdout.write(
            f"{action} completed idempotency receipts: {receipt_count}"
        )
        self.stdout.write(
            "Incomplete idempotency receipts are preserved for reconciliation."
        )
        if not apply_changes:
            self.stdout.write("Dry run only. Re-run with --apply to delete eligible rows.")

    @staticmethod
    def _delete_queryset(queryset, batch_size: int) -> int:
        """Delete an indexed expiry queryset without holding one long transaction."""
        deleted_total = 0
        model = queryset.model
        while True:
            row_ids = list(
                queryset.order_by("pk").values_list("pk", flat=True)[:batch_size]
            )
            if not row_ids:
                return deleted_total
            deleted, _details = model.objects.filter(pk__in=row_ids).delete()
            deleted_total += deleted

    @staticmethod
    def _scan_receipts(cutoff, batch_size: int, *, delete: bool) -> int:
        """Scan receipts once by primary key and prune only completed old rows.

        IntegrationEventReceipt predates the lifecycle policy and has no global
        creation-time index. A keyset scan keeps this maintenance path O(n), uses
        bounded memory, and avoids repeated full-table scans on SQLite or PostgreSQL.
        """
        matched_total = 0
        last_pk = 0
        while True:
            rows = list(
                IntegrationEventReceipt.objects.filter(pk__gt=last_pk)
                .order_by("pk")
                .values_list("pk", "created_at", "response_status_code")[:batch_size]
            )
            if not rows:
                return matched_total

            eligible_ids = [
                row_id
                for row_id, created_at, response_status_code in rows
                if created_at < cutoff
                and response_status_code != INCOMPLETE_RECEIPT_STATUS
            ]
            matched_total += len(eligible_ids)
            if delete and eligible_ids:
                IntegrationEventReceipt.objects.filter(pk__in=eligible_ids).delete()
            last_pk = rows[-1][0]
