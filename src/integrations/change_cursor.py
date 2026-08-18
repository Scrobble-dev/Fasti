"""Opaque client-bound cursors for ordered synchronization change feeds."""

from dataclasses import dataclass

from django.core import signing
from django.core.signing import BadSignature, SignatureExpired

from integrations.sync_policy import (
    CURSOR_MAX_AGE_SECONDS,
    CURSOR_MAX_LENGTH,
    CURSOR_VERSION,
)

_CURSOR_SALT = "integrations.ordered-change-feed"


@dataclass(frozen=True)
class ChangeCursorState:
    """Validated ordered position for one synchronization change feed."""

    sequence_id: int


class ChangeCursorError(ValueError):
    """Base error for invalid ordered change cursors."""

    code = "invalid_cursor"


class ChangeCursorExpired(ChangeCursorError):
    """Raised when an ordered change cursor is outside the recovery window."""

    code = "cursor_expired"


def encode_change_cursor(
    *,
    resource: str,
    user_id: int,
    client_id: str,
    sequence_id: int,
) -> str:
    """Sign one ordered position for an exact user, client, and resource."""
    if not isinstance(sequence_id, int) or isinstance(sequence_id, bool) or sequence_id < 0:
        raise ValueError("sequence_id must be a non-negative integer")
    return signing.dumps(
        {
            "v": CURSOR_VERSION,
            "resource": resource,
            "user": user_id,
            "client": client_id,
            "sequence": sequence_id,
        },
        salt=_CURSOR_SALT,
        compress=True,
    )


def decode_change_cursor(
    cursor: str,
    *,
    resource: str,
    user_id: int,
    client_id: str,
) -> ChangeCursorState:
    """Validate one ordered change cursor and return its sequence position."""
    if not isinstance(cursor, str) or not cursor or len(cursor) > CURSOR_MAX_LENGTH:
        raise ChangeCursorError

    try:
        payload = signing.loads(
            cursor,
            salt=_CURSOR_SALT,
            max_age=CURSOR_MAX_AGE_SECONDS,
        )
    except SignatureExpired as error:
        raise ChangeCursorExpired from error
    except (BadSignature, ValueError, TypeError) as error:
        raise ChangeCursorError from error

    if not isinstance(payload, dict):
        raise ChangeCursorError
    if (
        payload.get("v") != CURSOR_VERSION
        or payload.get("resource") != resource
        or payload.get("user") != user_id
        or payload.get("client") != client_id
    ):
        raise ChangeCursorError

    sequence_id = payload.get("sequence")
    if not isinstance(sequence_id, int) or isinstance(sequence_id, bool) or sequence_id < 0:
        raise ChangeCursorError

    return ChangeCursorState(sequence_id=sequence_id)
