"""Request-local playback origin context for durable progress changes."""

from contextvars import ContextVar, Token

_playback_source_client_id = ContextVar("playback_source_client_id", default="")


def set_playback_source_client_id(value: str | None) -> Token:
    """Set the current playback source client and return a reset token."""
    return _playback_source_client_id.set(value or "")


def reset_playback_source_client_id(token: Token) -> None:
    """Restore the playback source client value that preceded ``token``."""
    _playback_source_client_id.reset(token)


def get_playback_source_client_id() -> str:
    """Return the current server-derived playback source client identifier."""
    return _playback_source_client_id.get()
