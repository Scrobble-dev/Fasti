"""Authentication classes for API requests."""

import hashlib
from datetime import timedelta

from django.db.models import Q
from django.utils import timezone
from rest_framework.authentication import BaseAuthentication
from rest_framework.exceptions import AuthenticationFailed
from rest_framework.permissions import BasePermission

from app.playback_context import set_playback_source_client_id
from integrations.models import IntegrationToken
from users.models import User


_INTEGRATION_SCOPE_RULES = {
    "api_scrobble": {
        "POST": "scrobble:write",
    },
    "api_playback_progress": {
        "GET": "progress:read",
        "HEAD": "progress:read",
        "PUT": "progress:write",
        "DELETE": "progress:write",
    },
    "api_playback_progress_snapshot": {
        "GET": "progress:read",
        "HEAD": "progress:read",
    },
    "api_playback_progress_changes": {
        "GET": "progress:read",
        "HEAD": "progress:read",
    },
    "api_watchlist": {
        "GET": "watchlist:read",
        "HEAD": "watchlist:read",
        "PUT": "watchlist:write",
        "DELETE": "watchlist:write",
    },
    "api_watchlist_snapshot": {
        "GET": "watchlist:read",
        "HEAD": "watchlist:read",
    },
    "api_watchlist_changes": {
        "GET": "watchlist:read",
        "HEAD": "watchlist:read",
    },
    "api_watched_state": {
        "GET": "watched:read",
        "HEAD": "watched:read",
        "PUT": "watched:write",
        "DELETE": "watched:write",
    },
    "api_watched_state_snapshot": {
        "GET": "watched:read",
        "HEAD": "watched:read",
    },
    "api_watched_state_changes": {
        "GET": "watched:read",
        "HEAD": "watched:read",
    },
}
_LAST_USED_WRITE_INTERVAL = timedelta(minutes=15)
_CLIENT_ID_DIGEST_LENGTH = 24


def _scope_for_request(request):
    """Return (declared, scope) for an IntegrationToken request."""
    if request is None:
        return True, None

    resolver_match = getattr(request, "resolver_match", None)
    url_name = getattr(resolver_match, "url_name", None)
    method_scopes = _INTEGRATION_SCOPE_RULES.get(url_name)
    if method_scopes is None:
        return False, None

    method = request.method.upper()
    if method == "OPTIONS":
        return True, None

    scope = method_scopes.get(method)
    return scope is not None, scope


def _record_token_use(integration_token):
    """Update last-used metadata at most once per interval."""
    now = timezone.now()
    cutoff = now - _LAST_USED_WRITE_INTERVAL
    IntegrationToken.objects.filter(pk=integration_token.pk).filter(
        Q(last_used_at__isnull=True) | Q(last_used_at__lt=cutoff)
    ).update(last_used_at=now)


def _integration_client_id(integration_token: IntegrationToken) -> str:
    """Return a stable opaque identifier that is safe to expose as an origin."""
    return f"token:{integration_token.token_digest[:_CLIENT_ID_DIGEST_LENGTH]}"


def authenticate_token(raw_token: str, request=None, required_scope=None):
    """Authenticate an integration token or fall back to the legacy user token."""
    if not raw_token:
        msg = "Invalid token"
        raise AuthenticationFailed(msg)

    token_digest = hashlib.sha256(raw_token.encode("utf-8")).hexdigest()
    try:
        integration_token = IntegrationToken.objects.select_related("user").get(
            token_digest=token_digest
        )
    except IntegrationToken.DoesNotExist:
        pass
    else:
        if not integration_token.is_valid():
            msg = "Invalid token"
            raise AuthenticationFailed(msg)

        if required_scope is None:
            declared, required_scope = _scope_for_request(request)
            if not declared:
                msg = "Integration token is not permitted for this endpoint"
                raise AuthenticationFailed(msg)

        if required_scope and not integration_token.has_scope(required_scope):
            msg = "Integration token does not have the required scope"
            raise AuthenticationFailed(msg)

        _record_token_use(integration_token)
        if request is not None:
            set_playback_source_client_id(_integration_client_id(integration_token))
        return (integration_token.user, integration_token)

    try:
        user = User.objects.get(token=raw_token)
    except User.DoesNotExist:
        msg = "Invalid token"
        raise AuthenticationFailed(msg) from None
    if request is not None:
        set_playback_source_client_id("legacy")
    return (user, None)


class BearerAuthentication(BaseAuthentication):
    """Bearer or Token Authorization header authentication."""

    keywords = ("bearer", "token")

    def authenticate(self, request):
        """Authenticate the user with Bearer or Token header."""
        auth = request.headers.get("Authorization")
        if not auth:
            return None
        parts = auth.split()
        if len(parts) != 2 or parts[0].lower() not in self.keywords:  # noqa: PLR2004
            return None
        token = parts[1]
        return authenticate_token(token, request=request)


class ListenBrainzTokenAuthentication(BaseAuthentication):
    """ListenBrainz-style `Authorization: Token <token>` authentication.

    Exists so ListenBrainz-compatible scrobble clients (Multi-Scrobbler,
    Navidrome, Pano Scrobbler, ...) can authenticate against the ingest
    endpoints. Supports both IntegrationToken and legacy User.token.
    """

    keyword = "Token"

    def authenticate_header(self, request):
        """Return the WWW-Authenticate value so DRF answers 401, not 403.

        The ListenBrainz protocol specifies 401 for a missing or invalid token,
        and clients branch on it.
        """
        return self.keyword

    def authenticate(self, request):
        """Authenticate the user with a ListenBrainz-style token."""
        auth = request.headers.get("Authorization")
        if not auth:
            return None
        parts = auth.split()
        if len(parts) != 2 or parts[0].lower() != self.keyword.lower():  # noqa: PLR2004
            return None
        token = parts[1]
        return authenticate_token(
            token,
            request=request,
            required_scope="scrobble:write",
        )


class APIKeyAuthentication(BaseAuthentication):
    """API Key Authentication via X-API-Key header."""

    def authenticate(self, request):
        """Authenticate the user with API Key."""
        auth = request.headers.get("X-API-Key")
        if not auth:
            return None
        return authenticate_token(auth.strip(), request=request)


class HasScope(BasePermission):
    """Permission class to check whether request.auth grants a required scope.

    Legacy tokens (request.auth is None) have full access to all scopes.
    Integration tokens fail closed when a view does not declare a scope.
    """

    required_scope = None

    def __init__(self, required_scope=None):
        """Initialize permission class with an optional required scope."""
        if required_scope is not None:
            self.required_scope = required_scope

    def has_permission(self, request, view):
        """Return True if the request user and token scopes satisfy requirements."""
        if not request.user or not request.user.is_authenticated:
            return False
        if request.auth is None:
            return True
        if hasattr(request.auth, "has_scope"):
            scope = getattr(view, "required_scope", self.required_scope)
            if not scope:
                return False
            return request.auth.has_scope(scope)
        return False
