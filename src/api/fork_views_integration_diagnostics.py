"""Local integration diagnostics for the signed-in Floppy user."""

from http import HTTPStatus as HTTP  # noqa: N814

from django.db.models import Max, Q
from django.utils import timezone
from drf_spectacular.utils import extend_schema
from rest_framework import permissions, views as drf_views
from rest_framework.authentication import SessionAuthentication
from rest_framework.response import Response

from app.models import (
    Episode,
    Movie,
    PlaybackProgress,
    PlaybackProgressChange,
    SavedMediaChange,
    SavedMediaMembership,
    Status,
)
from integrations.models import IntegrationEventReceipt, IntegrationToken

_RESERVED_RECEIPT_STATUS = 0


def _latest_change(queryset):
    """Return non-sensitive freshness data for one ordered change feed."""
    latest = queryset.order_by("-sequence_id").values("sequence_id", "created_at").first()
    if latest is None:
        return {"latest_sequence": 0, "latest_change_at": None}
    return {
        "latest_sequence": latest["sequence_id"],
        "latest_change_at": latest["created_at"],
    }


class IntegrationDiagnosticsView(drf_views.APIView):
    """Report local integration health without contacting external services."""

    authentication_classes = [SessionAuthentication]
    permission_classes = [permissions.IsAuthenticated]

    @extend_schema(
        description=(
            "Return local integration diagnostics for the signed-in user. The endpoint "
            "does not call Nuvio or any metadata provider and never returns token secrets, "
            "idempotency keys, or raw client payloads."
        ),
        responses={200: {"type": "object"}},
    )
    def get(self, request):
        """Return bounded local health and recovery information."""
        now = timezone.now()
        tokens = IntegrationToken.objects.filter(user=request.user)
        active_tokens = tokens.filter(revoked_at__isnull=True).filter(
            Q(expires_at__isnull=True) | Q(expires_at__gt=now)
        )
        revoked_count = tokens.filter(revoked_at__isnull=False).count()
        expired_count = tokens.filter(
            revoked_at__isnull=True,
            expires_at__isnull=False,
            expires_at__lte=now,
        ).count()

        receipts = IntegrationEventReceipt.objects.filter(user=request.user)
        incomplete_receipts = receipts.filter(
            response_status_code=_RESERVED_RECEIPT_STATUS
        ).count()
        latest_token_use = tokens.aggregate(value=Max("last_used_at"))["value"]

        progress_changes = PlaybackProgressChange.objects.filter(user=request.user)
        saved_changes = SavedMediaChange.objects.filter(user=request.user)
        watched_movies = (
            Movie.objects.filter(user=request.user)
            .filter(
                Q(status=Status.COMPLETED.value)
                | Q(end_date__isnull=False)
                | Q(plays__isnull=False)
            )
            .distinct()
            .count()
        )
        watched_episodes = (
            Episode.objects.filter(
                related_season__user=request.user,
                status=Status.COMPLETED.value,
                item__isnull=False,
            )
            .values("item_id")
            .distinct()
            .count()
        )

        recovery = []
        if incomplete_receipts:
            recovery.append(
                {
                    "code": "incomplete_delivery_receipts",
                    "count": incomplete_receipts,
                    "action": (
                        "Check current media state before the affected client retries with "
                        "a new idempotency key."
                    ),
                }
            )

        return Response(
            {
                "state": "needs_attention" if recovery else "ready",
                "tokens": {
                    "active": active_tokens.count(),
                    "revoked": revoked_count,
                    "expired": expired_count,
                    "last_used_at": latest_token_use,
                },
                "delivery": {
                    "receipt_count": receipts.count(),
                    "incomplete_receipt_count": incomplete_receipts,
                },
                "progress": {
                    "current_count": PlaybackProgress.objects.filter(
                        user=request.user
                    ).count(),
                    **_latest_change(progress_changes),
                },
                "saved_media": {
                    "current_count": SavedMediaMembership.objects.filter(
                        user=request.user
                    ).count(),
                    **_latest_change(saved_changes),
                },
                "watched_state": {
                    "movie_count": watched_movies,
                    "episode_count": watched_episodes,
                },
                "external_client_sync": {
                    "state": "not_reported",
                    "detail": (
                        "Floppy can report its local durable state, but it cannot claim that "
                        "a Nuvio client completed synchronization unless that client reports "
                        "or reconciles its remote state."
                    ),
                },
                "offline": {
                    "diagnostics_require_external_network": False,
                    "existing_identity_progress_writes_require_external_network": False,
                    "existing_identity_saved_media_writes_require_external_network": False,
                    "new_unresolved_saved_media_may_require_metadata_resolution": True,
                },
                "recovery": recovery,
            },
            status=HTTP.OK,
        )
