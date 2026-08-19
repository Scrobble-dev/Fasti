import hashlib

import django.db.models.deletion
from django.db import migrations, models

_CLIENT_NAMESPACE_DIGEST_LENGTH = 24
_LEGACY_NAMESPACE = "legacy"


def _client_namespace(token) -> str:
    """Return the stable opaque client namespace used by delivery receipts."""
    if token is None:
        return _LEGACY_NAMESPACE
    source = token.client_identifier.strip() or token.token_digest
    digest = hashlib.sha256(source.encode("utf-8")).hexdigest()
    return f"client-{digest[:_CLIENT_NAMESPACE_DIGEST_LENGTH]}"


def _backfill_client_namespaces(apps, schema_editor):
    """Backfill a namespace without rewriting historical public/storage keys."""
    receipt_model = apps.get_model("integrations", "IntegrationEventReceipt")

    for receipt in (
        receipt_model.objects.select_related("token").only(
            "id",
            "client_namespace",
            "token__client_identifier",
            "token__token_digest",
        )
    ).iterator(chunk_size=1000):
        namespace = _client_namespace(receipt.token)
        receipt_model.objects.filter(pk=receipt.pk).update(client_namespace=namespace)


def _restore_prefixed_storage_keys(apps, schema_editor):
    """Make receipt keys unique again before reversing to the legacy constraint."""
    receipt_model = apps.get_model("integrations", "IntegrationEventReceipt")

    for receipt in receipt_model.objects.only(
        "id",
        "client_namespace",
        "client_event_id",
    ).iterator(chunk_size=1000):
        if receipt.client_namespace == _LEGACY_NAMESPACE:
            continue
        prefix = f"{receipt.client_namespace}:"
        if receipt.client_event_id.startswith(prefix):
            continue
        receipt_model.objects.filter(pk=receipt.pk).update(
            client_event_id=f"{prefix}{receipt.client_event_id}"
        )


class Migration(migrations.Migration):
    """Scope delivery receipt uniqueness to a stable authenticated client."""

    dependencies = [
        ("integrations", "0026_psnaccount"),
    ]

    operations = [
        migrations.AddField(
            model_name="integrationeventreceipt",
            name="client_namespace",
            field=models.CharField(db_index=True, max_length=32, null=True),
        ),
        migrations.RemoveConstraint(
            model_name="integrationeventreceipt",
            name="unique_user_client_event_id",
        ),
        migrations.AlterField(
            model_name="integrationeventreceipt",
            name="token",
            field=models.ForeignKey(
                blank=True,
                null=True,
                on_delete=django.db.models.deletion.SET_NULL,
                related_name="event_receipts",
                to="integrations.integrationtoken",
            ),
        ),
        migrations.RunPython(
            _backfill_client_namespaces,
            _restore_prefixed_storage_keys,
        ),
        migrations.AlterField(
            model_name="integrationeventreceipt",
            name="client_namespace",
            field=models.CharField(db_index=True, max_length=32),
        ),
        migrations.AddConstraint(
            model_name="integrationeventreceipt",
            constraint=models.UniqueConstraint(
                fields=("user", "client_namespace", "client_event_id"),
                name="uniq_receipt_user_client_event",
            ),
        ),
    ]
