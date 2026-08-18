"""django-allauth OIDC adapter for delegated Floppy integrations."""

from urllib.parse import urlparse

from allauth.idp.oidc.adapter import DefaultOIDCAdapter
from django.core.exceptions import ValidationError

from integrations.scopes import INTEGRATION_SCOPE_DESCRIPTIONS


class FloppyOIDCAdapter(DefaultOIDCAdapter):
    """Expose Floppy's canonical resource scopes through allauth OIDC."""

    scope_display = {
        **DefaultOIDCAdapter.scope_display,
        **INTEGRATION_SCOPE_DESCRIPTIONS,
    }

    def validate_resource_uris(self, *, uris: list[str], **kwargs) -> None:
        """Restrict RFC 8707 resource indicators to this Floppy origin."""
        issuer = urlparse(self.get_issuer())
        for uri in uris:
            resource = urlparse(uri)
            if (
                resource.scheme not in {"http", "https"}
                or resource.scheme != issuer.scheme
                or resource.netloc != issuer.netloc
            ):
                msg = "OAuth resource targets must use this Floppy server origin."
                raise ValidationError(msg)
