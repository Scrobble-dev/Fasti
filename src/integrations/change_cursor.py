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


class ChangeCursorExpired(ChangeCursorError):  # noqa: N818
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
        message = "sequence_id must be a non-negative integer"
        raise ValueError(message)
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


def _load_cursor_payload(cursor: str, legacy_salts: tuple[str, ...]) -> object:
    """Load a canonical cursor, accepting known legacy salts during migration."""
    expired_error = None
    for salt in (_CURSOR_SALT, *legacy_salts):
        try:
            return signing.loads(
                cursor,
                salt=salt,
                max_age=CURSOR_MAX_AGE_SECONDS,
            )
        except SignatureExpired as error:
            expired_error = error
        except (BadSignature, ValueError, TypeError):
            continue
    if expired_error is not None:
        raise ChangeCursorExpired from expired_error
    raise ChangeCursorError


def decode_change_cursor(
    cursor: str,
    *,
    resource: str,
    user_id: int,
    client_id: str,
    legacy_salts: tuple[str, ...] = (),
) -> ChangeCursorState:
    """Validate one ordered change cursor and return its sequence position."""
    if not isinstance(cursor, str) or not cursor or len(cursor) > CURSOR_MAX_LENGTH:
        raise ChangeCursorError

    payload = _load_cursor_payload(cursor, legacy_salts)
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
