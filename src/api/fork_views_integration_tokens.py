"""Account-session-only lifecycle API for scoped integration credentials."""

from http import HTTPStatus as HTTP  # noqa: N814

from django.utils import timezone
from drf_spectacular.utils import OpenApiParameter, extend_schema
from rest_framework import permissions
from rest_framework.authentication import SessionAuthentication
from rest_framework import views as drf_views
from rest_framework.response import Response

from integrations.models import IntegrationToken
from integrations.scopes import (
    INTEGRATION_SCOPE_DESCRIPTIONS,
    INTEGRATION_SCOPES,
    NUVIO_RECOMMENDED_SCOPES,
)

_MAX_EXPIRY_DAYS = 3650

_TOKEN_CREATE_SCHEMA = {
    "type": "object",
    "required": ["name", "scopes"],
    "properties": {
        "name": {"type": "string", "minLength": 1, "maxLength": 255},
        "client_identifier": {"type": "string", "maxLength": 255},
        "scopes": {
            "type": "array",
            "items": {"type": "string", "enum": sorted(INTEGRATION_SCOPES)},
            "uniqueItems": True,
            "minItems": 1,
        },
        "expires_in_days": {
            "type": "integer",
            "minimum": 1,
            "maximum": _MAX_EXPIRY_DAYS,
            "nullable": True,
        },
    },
}


def _error(code: str, message: str, *, param: str | None = None, status=HTTP.BAD_REQUEST):
    """Return a stable token-management validation error."""
    return Response(
        {
            "error": {
                "type": "invalid_request_error",
                "code": code,
                "message": message,
                "param": param,
            },
        },
        status=status,
    )


def _token_state(token: IntegrationToken) -> str:
    """Return the user-facing lifecycle state for one scoped token."""
    if token.revoked_at is not None:
        return "revoked"
    if token.expires_at is not None and token.expires_at <= timezone.now():
        return "expired"
    return "active"


def _serialize_token(token: IntegrationToken) -> dict:
    """Serialize token metadata without returning a usable credential or digest."""
    return {
        "id": token.pk,
        "name": token.name,
        "client_identifier": token.client_identifier,
        "token_prefix": token.token_prefix,
        "scopes": token.scopes or [],
        "state": _token_state(token),
        "created_at": token.created_at,
        "last_used_at": token.last_used_at,
        "expires_at": token.expires_at,
        "revoked_at": token.revoked_at,
    }


def _parse_create_payload(data):
    """Return validated token fields or a public validation response."""
    if not isinstance(data, dict):
        return None, _error(
            "invalid_body",
            "Request body must be a JSON object.",
        )

    name = str(data.get("name", "")).strip()
    if not name or len(name) > 255:
        return None, _error(
            "invalid_name",
            "name must contain 1 to 255 characters.",
            param="name",
        )

    client_identifier = str(data.get("client_identifier", "")).strip()
    if len(client_identifier) > 255:
        return None, _error(
            "invalid_client_identifier",
            "client_identifier must be 255 characters or fewer.",
            param="client_identifier",
        )

    raw_scopes = data.get("scopes")
    if not isinstance(raw_scopes, list) or not raw_scopes:
        return None, _error(
            "invalid_scopes",
            "scopes must be a non-empty array.",
            param="scopes",
        )
    if any(not isinstance(scope, str) for scope in raw_scopes):
        return None, _error(
            "invalid_scopes",
            "Every scope must be a string.",
            param="scopes",
        )
    scopes = list(dict.fromkeys(raw_scopes))
    unsupported = sorted(set(scopes) - INTEGRATION_SCOPES)
    if unsupported:
        return None, _error(
            "invalid_scopes",
            f"Unsupported scope: {unsupported[0]}.",
            param="scopes",
        )

    expires_in_days = data.get("expires_in_days")
    expires_at = None
    if expires_in_days is not None:
        if isinstance(expires_in_days, bool):
            return None, _error(
                "invalid_expiry",
                f"expires_in_days must be between 1 and {_MAX_EXPIRY_DAYS}.",
                param="expires_in_days",
            )
        try:
            expires_in_days = int(expires_in_days)
        except (TypeError, ValueError):
            expires_in_days = 0
        if not 1 <= expires_in_days <= _MAX_EXPIRY_DAYS:
            return None, _error(
                "invalid_expiry",
                f"expires_in_days must be between 1 and {_MAX_EXPIRY_DAYS}.",
                param="expires_in_days",
            )
        expires_at = timezone.now() + timezone.timedelta(days=expires_in_days)

    return {
        "name": name,
        "client_identifier": client_identifier,
        "scopes": scopes,
        "expires_at": expires_at,
    }, None


class IntegrationTokenManagementView(drf_views.APIView):
    """List and create scoped integration tokens from an authenticated browser session."""

    authentication_classes = [SessionAuthentication]
    permission_classes = [permissions.IsAuthenticated]

    @extend_schema(
        description=(
            "List the current user's scoped integration credentials. This account-management "
            "surface requires the signed-in browser session and never returns token digests."
        ),
        responses={200: {"type": "object"}},
    )
    def get(self, request):
        """List token metadata for the signed-in user."""
        tokens = IntegrationToken.objects.filter(user=request.user).order_by("-created_at", "-id")
        return Response(
            {
                "results": [_serialize_token(token) for token in tokens],
                "available_scopes": [
                    {
                        "scope": scope,
                        "description": INTEGRATION_SCOPE_DESCRIPTIONS[scope],
                    }
                    for scope in sorted(INTEGRATION_SCOPES)
                ],
                "recommended_nuvio_scopes": list(NUVIO_RECOMMENDED_SCOPES),
            },
            status=HTTP.OK,
        )

    @extend_schema(
        description=(
            "Create one named scoped credential. The full secret is returned once in this "
            "response and is not stored in recoverable form."
        ),
        request={"application/json": _TOKEN_CREATE_SCHEMA},
        responses={201: {"type": "object"}, 400: {"type": "object"}},
    )
    def post(self, request):
        """Create a scoped token and return its secret once."""
        if request.user.is_demo:
            return _error(
                "demo_read_only",
                "Token creation is disabled for demo accounts.",
                status=HTTP.FORBIDDEN,
            )

        fields, error = _parse_create_payload(request.data)
        if error:
            return error
        token, raw_token = IntegrationToken.generate(
            user=request.user,
            name=fields["name"],
            scopes=fields["scopes"],
            client_identifier=fields["client_identifier"],
            expires_at=fields["expires_at"],
        )
        response = Response(
            {
                **_serialize_token(token),
                "token": raw_token,
                "secret_shown_once": True,
            },
            status=HTTP.CREATED,
        )
        response["Cache-Control"] = "no-store"
        response["Pragma"] = "no-cache"
        return response


class IntegrationTokenDetailView(drf_views.APIView):
    """Revoke one scoped integration token without erasing audit history."""

    authentication_classes = [SessionAuthentication]
    permission_classes = [permissions.IsAuthenticated]

    @extend_schema(
        description=(
            "Revoke one scoped credential owned by the signed-in user. Revocation is "
            "idempotent and does not physically delete delivery receipts."
        ),
        parameters=[
            OpenApiParameter(
                name="token_id",
                type=int,
                location=OpenApiParameter.PATH,
                required=True,
            )
        ],
        responses={204: None, 404: {"type": "object"}},
    )
    def delete(self, request, token_id):
        """Revoke one user-owned token."""
        if request.user.is_demo:
            return _error(
                "demo_read_only",
                "Token revocation is disabled for demo accounts.",
                status=HTTP.FORBIDDEN,
            )
        token = IntegrationToken.objects.filter(user=request.user, pk=token_id).first()
        if token is None:
            return _error(
                "token_not_found",
                "Integration token not found.",
                status=HTTP.NOT_FOUND,
            )
        if token.revoked_at is None:
            IntegrationToken.objects.filter(pk=token.pk, revoked_at__isnull=True).update(
                revoked_at=timezone.now()
            )
        return Response(status=HTTP.NO_CONTENT)
