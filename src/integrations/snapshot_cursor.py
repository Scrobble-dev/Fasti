"""Opaque client-bound cursors for stable synchronization snapshots."""

from dataclasses import dataclass

from django.core import signing
from django.core.signing import BadSignature, SignatureExpired

from integrations.sync_policy import (
    CURSOR_MAX_AGE_SECONDS,
    CURSOR_MAX_LENGTH,
    SNAPSHOT_CURSOR_VERSION,
)

_CURSOR_SALT = "integrations.stable-snapshot"


@dataclass(frozen=True)
class SnapshotCursorState:
    """Validated position and immutable filters for one stable snapshot."""

    upper_id: int
    after_id: int
    filters: dict[str, object]


class SnapshotCursorError(ValueError):
    """Base error for invalid or expired stable snapshot cursors."""

    code = "invalid_snapshot_cursor"


class SnapshotCursorExpired(SnapshotCursorError):  # noqa: N818
    """Raised when a stable snapshot cursor is older than the supported window."""

    code = "snapshot_cursor_expired"


def encode_snapshot_cursor(
    *,
    resource: str,
    user_id: int,
    client_id: str,
    upper_id: int,
    after_id: int,
    filters: dict[str, object],
) -> str:
    """Sign one snapshot position for an exact user, client, and resource."""
    return signing.dumps(
        {
            "v": SNAPSHOT_CURSOR_VERSION,
            "resource": resource,
            "user": user_id,
            "client": client_id,
            "upper": upper_id,
            "after": after_id,
            "filters": filters,
        },
        salt=_CURSOR_SALT,
        compress=True,
    )


def decode_snapshot_cursor(
    cursor: str,
    *,
    resource: str,
    user_id: int,
    client_id: str,
) -> SnapshotCursorState:
    """Validate a stable snapshot cursor and return its immutable state."""
    if not isinstance(cursor, str) or not cursor or len(cursor) > CURSOR_MAX_LENGTH:
        raise SnapshotCursorError

    try:
        payload = signing.loads(
            cursor,
            salt=_CURSOR_SALT,
            max_age=CURSOR_MAX_AGE_SECONDS,
        )
    except SignatureExpired as error:
        raise SnapshotCursorExpired from error
    except (BadSignature, ValueError, TypeError) as error:
        raise SnapshotCursorError from error

    if not isinstance(payload, dict):
        raise SnapshotCursorError
    if (
        payload.get("v") != SNAPSHOT_CURSOR_VERSION
        or payload.get("resource") != resource
        or payload.get("user") != user_id
        or payload.get("client") != client_id
    ):
        raise SnapshotCursorError

    upper_id = payload.get("upper")
    after_id = payload.get("after")
    filters = payload.get("filters")
    if (
        not isinstance(upper_id, int)
        or isinstance(upper_id, bool)
        or upper_id < 0
        or not isinstance(after_id, int)
        or isinstance(after_id, bool)
        or after_id < 0
        or after_id > upper_id
        or not isinstance(filters, dict)
    ):
        raise SnapshotCursorError

    return SnapshotCursorState(
        upper_id=upper_id,
        after_id=after_id,
        filters=filters,
    )
