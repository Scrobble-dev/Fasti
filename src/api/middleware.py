import json
import logging
from http import HTTPStatus as HTTP  # noqa: N814

from django.core.serializers.json import DjangoJSONEncoder
from django.db import IntegrityError
from django.http import JsonResponse
from django.template.response import ContentNotRenderedError, TemplateResponse

from app.playback_context import (
    reset_playback_source_client_id,
    set_playback_source_client_id,
)

logger = logging.getLogger(__name__)

_TRACKED_MEDIA_CONSTRAINT_SUFFIX = "_unique_item_user"
_TRACKED_MEDIA_UNIQUE_COLUMN_COUNT = 2
_MEDIA_COLLECTION_PATH_PART_COUNT = 4
_SENSITIVE_ERROR_KEYS = {
    "debug_html_snippet",
    "exception",
    "stack",
    "traceback",
}


def _is_sqlite_tracked_media_unique_error(exception):
    """Return whether SQLite reports the tracked-media user/item constraint."""
    message = str(exception).lower()
    marker = "unique constraint failed:"
    if marker not in message:
        return False

    raw_columns = message.split(marker, 1)[1].strip().split(",")
    if len(raw_columns) != _TRACKED_MEDIA_UNIQUE_COLUMN_COUNT:
        return False

    columns = []
    for raw_column in raw_columns:
        table, separator, column = raw_column.strip().rpartition(".")
        if not separator:
            return False
        columns.append((table, column))

    (first_table, first_column), (second_table, second_column) = columns
    return (
        first_table == second_table
        and first_table.startswith("app_")
        and {first_column, second_column} == {"user_id", "item_id"}
    )


def is_tracked_media_unique_error(exception):
    """Return whether an IntegrityError is the tracked-media duplicate constraint."""
    if not isinstance(exception, IntegrityError):
        return False

    cause = exception.__cause__ or exception
    diagnostic = getattr(cause, "diag", None)
    constraint_name = getattr(diagnostic, "constraint_name", "") or ""
    if constraint_name.endswith(_TRACKED_MEDIA_CONSTRAINT_SUFFIX):
        return True

    message = str(cause).lower()
    if "duplicate key" in message and _TRACKED_MEDIA_CONSTRAINT_SUFFIX in message:
        return True

    return _is_sqlite_tracked_media_unique_error(cause)


def _is_media_collection_post(request, path):
    """Return whether this is POST /api/v1/media/{media_type}."""
    if getattr(request, "method", "").upper() != "POST":
        return False
    parts = path.strip("/").split("/")
    return (
        len(parts) == _MEDIA_COLLECTION_PATH_PART_COUNT
        and parts[:3] == ["api", "v1", "media"]
    )


class ApiJsonErrorMiddleware:
    """Convert API failures to stable JSON and remove raw exception details."""

    def __init__(self, get_response):  # noqa: D107
        self.get_response = get_response

    def __call__(self, request):  # noqa: D102
        context_token = set_playback_source_client_id("")
        try:
            response = self.get_response(request)
            path = self._get_request_path(request)

            if path.startswith("/api/") and response is not None:
                response = self._handle_template_response(response, path)
                status = getattr(response, "status_code", HTTP.OK)
                content_type = self._get_content_type(response)

                if status >= HTTP.BAD_REQUEST and "application/json" in content_type:
                    response = self._sanitize_json_error_response(response, status, path)
                    content_type = self._get_content_type(response)

                if self._should_convert_to_json(status, content_type):
                    return self._build_json_error_response(status)

            return response
        finally:
            reset_playback_source_client_id(context_token)

    def _get_request_path(self, request):
        """Safely extract the request path."""
        try:
            return request.path or ""
        except Exception:
            return ""

    def _handle_template_response(self, response, path):
        """Render TemplateResponse if needed."""
        if isinstance(response, TemplateResponse):
            try:
                response = response.render()
            except ContentNotRenderedError:
                logger.exception(
                    "TemplateResponse could not be rendered for %s",
                    path,
                )
            except Exception:
                logger.exception(
                    "Error while rendering TemplateResponse for %s",
                    path,
                )
        return response

    def _get_content_type(self, response):
        """Extract content type from response."""
        if hasattr(response, "headers"):
            return response.headers.get("Content-Type", "")

        if hasattr(response, "get"):
            try:
                return response.get("Content-Type", "")
            except Exception:
                return getattr(response, "content_type", "")

        return getattr(response, "content_type", "")

    def _should_convert_to_json(self, status, content_type):
        """Determine if response should be converted to JSON."""
        if content_type and "application/json" in content_type:
            return False
        return status >= HTTP.BAD_REQUEST and (
            not content_type or "html" in content_type.lower()
        )

    def _sanitize_json_error_response(self, response, status, path):
        """Remove raw exception strings from rendered JSON error payloads."""
        try:
            payload = json.loads(response.content.decode("utf-8"))
        except (AttributeError, UnicodeDecodeError, json.JSONDecodeError):
            return response

        if not isinstance(payload, dict):
            return response

        removed = False
        errors = payload.get("errors")
        if isinstance(errors, str):
            payload.pop("errors", None)
            removed = True

        for key in _SENSITIVE_ERROR_KEYS:
            if key in payload:
                payload.pop(key, None)
                removed = True

        if not removed:
            return response

        logger.warning("Suppressed raw API error details for %s", path)
        if hasattr(response, "data"):
            response.data = payload
        response.content = json.dumps(payload, cls=DjangoJSONEncoder).encode("utf-8")
        response["Content-Length"] = str(len(response.content))
        response.status_code = status
        return response

    def _build_json_error_response(self, status):
        """Build a stable JSON error response without debug internals."""
        try:
            detail = HTTP(status).phrase
        except ValueError:
            detail = "Unknown status"

        return JsonResponse({"detail": detail}, status=status)

    def process_exception(self, request, exception):
        """Intercept unhandled API exceptions without exposing their text."""
        path = self._get_request_path(request)

        if not path.startswith("/api/"):
            return None

        if _is_media_collection_post(request, path) and is_tracked_media_unique_error(
            exception,
        ):
            logger.info("Tracked media create conflict: %s", path)
            return JsonResponse(
                {"detail": "Conflict. Media is already tracked."},
                status=HTTP.CONFLICT,
            )

        logger.exception("Unhandled exception during API request: %s", path)
        return JsonResponse(
            {"detail": "Internal server error."},
            status=HTTP.INTERNAL_SERVER_ERROR,
        )
