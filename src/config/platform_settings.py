"""Optional first-party and delegated-authentication platform settings.

The base Floppy settings stay usable for offline and packaged deployments. This
module adds django-allauth's Headless and OIDC IdP applications so every normal
runtime uses one account platform while exposing the related network routes only
when an operator explicitly enables them.
"""

from decouple import Undefined
from django.core.exceptions import ImproperlyConfigured

from . import settings as base
from .settings import *  # noqa: F403

# Install the apps for one consistent schema/migration graph. Network surfaces
# remain disabled until config/urls.py sees the corresponding feature flag.
for _app in ("allauth.headless", "allauth.idp.oidc"):
    if _app not in INSTALLED_APPS:  # noqa: F405
        INSTALLED_APPS.append(_app)  # noqa: F405

FLOPPY_HEADLESS_ENABLED = base.config(
    "FLOPPY_HEADLESS_ENABLED",
    default=False,
    cast=bool,
)
FLOPPY_OIDC_IDP_ENABLED = base.config(
    "FLOPPY_OIDC_IDP_ENABLED",
    default=False,
    cast=bool,
)

# First-party packaged/mobile clients use allauth session tokens. Browser
# sessions continue to use the normal allauth account views and CSRF boundary.
HEADLESS_CLIENTS = ("app",)
HEADLESS_SERVE_SPECIFICATION = FLOPPY_HEADLESS_ENABLED
HEADLESS_TOKEN_STRATEGY = (
    "allauth.headless.tokens.strategies.sessions.SessionTokenStrategy"
)

# Delegated third-party access uses the allauth OIDC provider. Keep access tokens
# opaque so revocation, inactive-user checks, and scope changes are authoritative
# at the server on each request.
IDP_OIDC_ADAPTER = "integrations.oidc_adapter.FloppyOIDCAdapter"
IDP_OIDC_ACCESS_TOKEN_FORMAT = "opaque"
IDP_OIDC_ACCESS_TOKEN_EXPIRES_IN = base.config(
    "FLOPPY_OIDC_ACCESS_TOKEN_EXPIRES_IN",
    default=900,
    cast=int,
)
IDP_OIDC_AUTHORIZATION_CODE_EXPIRES_IN = 60
IDP_OIDC_DEVICE_CODE_EXPIRES_IN = base.config(
    "FLOPPY_OIDC_DEVICE_CODE_EXPIRES_IN",
    default=600,
    cast=int,
)
IDP_OIDC_DEVICE_CODE_INTERVAL = 5
IDP_OIDC_ROTATE_REFRESH_TOKEN = True

# 65.15 does not expose dynamic client registration. Newer allauth versions do;
# this setting is deliberately present now so the secure default survives the
# planned dependency upgrade.
IDP_OIDC_DCR_ENABLED = False
IDP_OIDC_DCR_REQUIRES_INITIAL_ACCESS_TOKEN = True

_oidc_private_key = base.config("FLOPPY_OIDC_PRIVATE_KEY", default=None)
if not _oidc_private_key:
    _oidc_private_key = base.secret("FLOPPY_OIDC_PRIVATE_KEY_FILE")
if isinstance(_oidc_private_key, Undefined):
    _oidc_private_key = None

if FLOPPY_OIDC_IDP_ENABLED and not _oidc_private_key:
    msg = (
        "FLOPPY_OIDC_IDP_ENABLED requires FLOPPY_OIDC_PRIVATE_KEY or "
        "FLOPPY_OIDC_PRIVATE_KEY_FILE."
    )
    raise ImproperlyConfigured(msg)
IDP_OIDC_PRIVATE_KEY = _oidc_private_key or ""

# Preserve the existing API credential classes. OIDC/Headless are additional
# transports into the same user and scope boundary, not replacements for the
# offline-compatible PAT contract.
