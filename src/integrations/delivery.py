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
    """Return an opaque stable namespace for one authenticated client."""
    if token is None:
        return "legacy"
    source = token.client_identifier.strip() or token.token_digest
    digest = hashlib.sha256(source.encode("utf-8")).hexdigest()
    return f"client-{digest[:_CLIENT_NAMESPACE_DIGEST_LENGTH]}"


def _receipt_belongs_to_client(receipt, token: IntegrationToken | None) -> bool:
    """Return True when a receipt belongs to the authenticated client namespace."""
    return receipt.client_namespace == _client_namespace(token)


def _legacy_client_receipt_id(
    client_event_id: str,
    token: IntegrationToken | None,
) -> str | None:
    """Return the pre-schema client-prefixed storage key for compatibility reads."""
    if token is None:
        return None
    return f"{_client_namespace(token)}:{client_event_id}"


def _legacy_token_receipt_id(
    client_event_id: str,
    token: IntegrationToken | None,
) -> str | None:
    """Return the older token-digest-prefixed storage key for compatibility reads."""
    if token is None:
        return None
    namespace = f"token-{token.token_digest[:_CLIENT_NAMESPACE_DIGEST_LENGTH]}"
    return f"{namespace}:{client_event_id}"


def _find_receipt(user, client_event_id: str, token: IntegrationToken | None):
    """Return this client's current or historical receipt for one public key."""
    namespace = _client_namespace(token)
    candidate_ids = [client_event_id]
    if token is not None:
        candidate_ids.extend(
            filter(
                None,
                (
                    _legacy_client_receipt_id(client_event_id, token),
                    _legacy_token_receipt_id(client_event_id, token),
                ),
            )
        )

    for candidate_id in dict.fromkeys(candidate_ids):
        receipt = (
            IntegrationEventReceipt.objects.filter(
                user=user,
                client_namespace=namespace,
                client_event_id=candidate_id,
            )
            .select_related("token")
            .first()
        )
        if receipt is not None and _receipt_belongs_to_client(receipt, token):
            return receipt
    return None


def _reserve_receipt(
    user,
    client_event_id: str,
    digest: str,
    token: IntegrationToken | None,
):
    """Reserve one client delivery key and resolve a concurrent duplicate safely."""
    receipt = _find_receipt(user, client_event_id, token)
    if receipt is not None:
        return receipt, False

    try:
        with transaction.atomic():
            receipt = IntegrationEventReceipt.objects.create(
                user=user,
                token=token,
                client_namespace=_client_namespace(token),
                client_event_id=client_event_id,
                payload_digest=digest,
                response_status_code=_RESERVED_RESPONSE_STATUS,
                response_body={},
            )
    except IntegrityError:
        receipt = _find_receipt(user, client_event_id, token)
        if receipt is None:
            raise
        return receipt, False
    return receipt, True


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
