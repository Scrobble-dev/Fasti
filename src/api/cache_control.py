"""Cache-control helpers for private API responses."""


def private_no_store(response):
    """Mark an authenticated response private and non-cacheable."""
    response["Cache-Control"] = "private, no-store"
    response["Pragma"] = "no-cache"
    return response
