"""django-allauth OIDC integration policy for Floppy delegated clients."""

from allauth.idp.oidc.adapter import DefaultOIDCAdapter

from .scopes import INTEGRATION_SCOPE_DESCRIPTIONS


class FloppyOIDCAdapter(DefaultOIDCAdapter):
    """Expose Floppy resource permissions through allauth's OIDC provider.

    Account identity remains owned by django-allauth. Media identity, matching,
    classification, and synchronization semantics remain owned by Floppy's
    integration domain and are intentionally absent from OIDC claims.
    """

    scope_display = {
        **DefaultOIDCAdapter.scope_display,
        **INTEGRATION_SCOPE_DESCRIPTIONS,
    }

    def populate_access_token(self, access_token, *, client, scopes, user, **kwargs):
        """Attach only the stable client identifier needed for origin diagnostics.

        Opaque access tokens remain the preferred format. The client identifier
        is useful if an operator deliberately enables JWT access tokens later,
        while no media/provider/user-profile data is exposed as a token claim.
        """
        access_token["floppy_client_id"] = str(client.pk)
