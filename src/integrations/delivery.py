"""Deduplication and delivery receipt gateway for integration events."""

import hashlib
import json
import logging
from collections.abc import Callable
from http import HTTPStatus as HTTP  # noqa: N814
from typing import Any

from django.core.serializers.json import DjangoJSONEncoder
from django.db import IntegrityError, transaction
from rest_framework.response import Response

from .models import IntegrationEventReceipt, IntegrationToken

logger = logging.getLogger(__name__)

_MAX_IDEMPOTENCY_KEY_LENGTH = 128
_RESERVED_RESPONSE_STATUS = 0
_ASCII_VISIBLE_MIN = 0x21
_ASCII_VISIBLE_MAX = 0x7E
_MAX_RECEIPT_RESERVATION_ATTEMPTS = 2
_CLIENT_NAMESPACE_DIGEST_LENGTH = 24


def calculate_payload_digest(payload: Any) -> str:
    """Calculate a deterministic SHA-256 digest for a request payload."""
    if isinstance(payload, (dict, list)):
        json_str = json.dumps(payload, sort_keys=True, separators=(",", ":"), default=str)
    elif isinstance(payload, str):
        try:
            parsed = json.loads(payload)
            if isinstance(parsed, (dict, list)):
                json_str = json.dumps(parsed, sort_keys=True, separators=(",", ":"), default=str)
            else:
                json_str = payload
        except (ValueError, TypeError):
            json_str = payload
    elif isinstance(payload, bytes):
        try:
            parsed = json.loads(payload.decode("utf-8"))
            if isinstance(parsed, (dict, list)):
                json_str = json.dumps(parsed, sort_keys=True, separators=(",", ":"), default=str)
            else:
                json_str = payload.decode("utf-8", errors="replace")
        except (ValueError, TypeError, UnicodeDecodeError):
            json_str = payload.decode("utf-8", errors="replace")
    else:
        try:
            json_str = json.dumps(payload, sort_keys=True, separators=(",", ":"), default=str)
        except (TypeError, ValueError):
            json_str = str(payload)

    return hashlib.sha256(json_str.encode("utf-8")).hexdigest()


def _error_response(client_event_id: str, code: str, message: str, status: int):
    """Return the stable error envelope used by delivery failures."""
    return Response(
        {
            "error": {
                "type": "conflict_error" if status == HTTP.CONFLICT else "invalid_request_error",
                "code": code,
                "message": message,
                "param": "Idempotency-Key",
                "correlation_id": f"rec_{client_event_id}",
            },
        },
        status=status,
    )


def _validate_client_event_id(client_event_id: str):
    """Reject empty, oversized, whitespace, and control-character keys."""
    if not client_event_id:
        return _error_response(
            client_event_id,
            "invalid_idempotency_key",
            "Idempotency-Key must not be empty.",
            HTTP.BAD_REQUEST,
        )
    if len(client_event_id) > _MAX_IDEMPOTENCY_KEY_LENGTH:
        return _error_response(
            client_event_id[:_MAX_IDEMPOTENCY_KEY_LENGTH],
            "invalid_idempotency_key",
            f"Idempotency-Key must be {_MAX_IDEMPOTENCY_KEY_LENGTH} characters or fewer.",
            HTTP.BAD_REQUEST,
        )
    if any(
        ord(char) < _ASCII_VISIBLE_MIN or ord(char) > _ASCII_VISIBLE_MAX
        for char in client_event_id
    ):
        return _error_response(
            "invalid",
            "invalid_idempotency_key",
            "Idempotency-Key must contain visible ASCII characters without spaces.",
            HTTP.BAD_REQUEST,
        )
    return None


def _receipt_result(receipt, digest: str, client_event_id: str):
    """Return a replay/conflict result for an existing receipt."""
    if receipt.payload_digest != digest:
        return (
            _error_response(
                client_event_id,
                "idempotency_conflict",
                "The provided Idempotency-Key has already been used with a different request payload.",
                HTTP.CONFLICT,
            ),
            False,
        )

    if receipt.response_status_code == _RESERVED_RESPONSE_STATUS:
        return (
            _error_response(
                client_event_id,
                "idempotency_incomplete",
                (
                    "The earlier request with this Idempotency-Key did not record a final "
                    "response. Check current state before retrying with a new key."
                ),
                HTTP.CONFLICT,
            ),
            True,
        )

    replay_body = None if receipt.response_status_code == HTTP.NO_CONTENT else receipt.response_body
    return (Response(replay_body, status=receipt.response_status_code), True)


def _client_namespace(token: IntegrationToken | None) -> str:
    """Return an opaque stable namespace for one authenticated client.

    A configured client identifier survives token rotation. Tokens without one
    remain isolated by their digest. The raw client identifier and token digest
    are never stored in receipt keys.
    """
    if token is None:
        return "legacy"
    source = token.client_identifier.strip() or token.token_digest
    digest = hashlib.sha256(source.encode("utf-8")).hexdigest()
    return f"client-{digest[:_CLIENT_NAMESPACE_DIGEST_LENGTH]}"


def _receipt_belongs_to_client(receipt, token: IntegrationToken | None) -> bool:
    """Return True when a receipt belongs to the authenticated client namespace."""
    if receipt.token_id is None or token is None:
        return receipt.token_id is None and token is None
    return _client_namespace(receipt.token) == _client_namespace(token)


def _client_receipt_id(client_event_id: str, token: IntegrationToken | None) -> str:
    """Return the canonical internal storage key for one delivery client."""
    if token is None:
        return client_event_id
    return f"{_client_namespace(token)}:{client_event_id}"


def _legacy_token_receipt_id(client_event_id: str, token: IntegrationToken | None) -> str | None:
    """Return the pre-stable-namespace storage key for compatibility reads."""
    if token is None:
        return None
    namespace = f"token-{token.token_digest[:_CLIENT_NAMESPACE_DIGEST_LENGTH]}"
    return f"{namespace}:{client_event_id}"


def _find_receipt_or_slot(user, client_event_id: str, token: IntegrationToken | None):
    """Return this client's receipt or its deterministic storage key.

    New scoped-token receipts always use an opaque client namespace, so two
    clients can safely reuse the same public Idempotency-Key without first
    colliding on the user-level unique constraint. Historical public-key and
    token-digest-prefixed receipts remain readable during the compatibility
    period.
    """
    storage_id = _client_receipt_id(client_event_id, token)
    candidate_ids = [storage_id]
    if token is not None:
        candidate_ids.append(client_event_id)
        legacy_id = _legacy_token_receipt_id(client_event_id, token)
        if legacy_id is not None:
            candidate_ids.append(legacy_id)

    for candidate_id in dict.fromkeys(candidate_ids):
        receipt = (
            IntegrationEventReceipt.objects.select_related("token")
            .filter(user=user, client_event_id=candidate_id)
            .first()
        )
        if receipt is not None and _receipt_belongs_to_client(receipt, token):
            return receipt, candidate_id

    return None, storage_id


def _reserve_receipt(
    user,
    client_event_id: str,
    digest: str,
    token: IntegrationToken | None,
):
    """Reserve one client delivery key, retrying only for a concurrent collision."""
    receipt, storage_id = _find_receipt_or_slot(user, client_event_id, token)
    if receipt is not None:
        return receipt, False

    for _attempt in range(_MAX_RECEIPT_RESERVATION_ATTEMPTS):
        try:
            with transaction.atomic():
                receipt = IntegrationEventReceipt.objects.create(
                    user=user,
                    token=token,
                    client_event_id=storage_id,
                    payload_digest=digest,
                    response_status_code=_RESERVED_RESPONSE_STATUS,
                    response_body={},
                )
        except IntegrityError:
            receipt, next_storage_id = _find_receipt_or_slot(
                user,
                client_event_id,
                token,
            )
            if receipt is not None:
                return receipt, False
            if next_storage_id == storage_id:
                raise
            storage_id = next_storage_id
        else:
            return receipt, True

    msg = "Could not reserve an idempotency receipt after a concurrent collision."
    raise IntegrityError(msg)


def get_or_record_receipt(
    user: Any,
    client_event_id: str,
    payload: Any,
    execute_fn: Callable[[], Response],
    token: IntegrationToken | None = None,
) -> tuple[Response, bool]:
    """Reserve one delivery key, execute once, and persist the final response.

    The reservation commits before the protected operation runs. A concurrent
    duplicate therefore cannot execute the same mutation before the unique receipt
    exists. If the process stops after the mutation but before the final response is
    recorded, the reservation remains and future retries fail closed instead of
    applying the mutation again.
    """
    client_event_id = str(client_event_id)
    validation_error = _validate_client_event_id(client_event_id)
    if validation_error is not None:
        return (validation_error, False)

    digest = calculate_payload_digest(payload)
    receipt, created = _reserve_receipt(
        user,
        client_event_id,
        digest,
        token,
    )
    if not created:
        return _receipt_result(receipt, digest, client_event_id)

    # Keep provider/network work outside the reservation transaction. If execution
    # raises after a mutation has landed, retain the reservation so a retry cannot
    # unknowingly apply the mutation again.
    response = execute_fn()

    if response.data is not None:
        try:
            response_data = json.loads(json.dumps(response.data, cls=DjangoJSONEncoder))
        except (TypeError, ValueError):
            response_data = {"detail": "Response was not serializable for replay."}
    else:
        response_data = {}

    IntegrationEventReceipt.objects.filter(
        pk=receipt.pk,
        response_status_code=_RESERVED_RESPONSE_STATUS,
    ).update(
        response_status_code=response.status_code,
        response_body=response_data,
    )

    return (response, False)
