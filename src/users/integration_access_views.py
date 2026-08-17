"""Settings surface for scoped third-party integration access."""

from django.contrib.auth.decorators import login_required
from django.shortcuts import render
from django.views.decorators.http import require_GET


@login_required
@require_GET
def integration_access(request):
    """Render the scoped integration credential management page."""
    return render(request, "users/integration_access.html")
