"""Versioned reference-only contract for the Local Shared Media Workspace."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, NoReturn

from integrations.content_classification import (
    ClassifiedContentType,
    classify_external_content_type,
)

_SCHEMA_VERSION = "1.0"
_MAX_COLLECTION_ID_LENGTH = 255
_MAX_TITLE_LENGTH = 255
_MAX_FOLDER_ID_LENGTH = 255
_MAX_ADDON_ID_LENGTH = 255
_MAX_CATALOG_ID_LENGTH = 255
_MAX_GENRE_LENGTH = 255
_MAX_FOLDERS = 128
_MAX_SOURCES_PER_FOLDER = 128
_ADDON_CATALOG_KIND = "addon-catalog"


@dataclass(frozen=True, slots=True)
class WorkspaceValidationError(ValueError):
    """Stable validation result for a portable workspace descriptor."""

    code: str
    param: str
    message: str

    def __str__(self) -> str:
        """Return the stable public-safe message."""
        return self.message


@dataclass(frozen=True, slots=True)
class AddonCatalogSource:
    """Portable source reference compatible with Nuvio's add-on catalog source."""

    addon_id: str
    content_type: ClassifiedContentType
    catalog_id: str
    genre: str | None = None

    def as_dict(self) -> dict[str, Any]:
        """Return the versioned wire representation without local implementation state."""
        payload = {
            "kind": _ADDON_CATALOG_KIND,
            "addonId": self.addon_id,
            "type": self.content_type.api_type,
            "catalogId": self.catalog_id,
        }
        if self.genre:
            payload["genre"] = self.genre
        return payload


@dataclass(frozen=True, slots=True)
class WorkspaceFolder:
    """Portable folder identity and source references only."""

    folder_id: str
    title: str
    sources: tuple[AddonCatalogSource, ...]

    def as_dict(self) -> dict[str, Any]:
        """Return the folder wire representation."""
        return {
            "id": self.folder_id,
            "title": self.title,
            "sources": [source.as_dict() for source in self.sources],
        }


@dataclass(frozen=True, slots=True)
class WorkspaceCollection:
    """Portable Collection descriptor that does not own Nuvio presentation state."""

    collection_id: str
    title: str
    folders: tuple[WorkspaceFolder, ...]
    schema_version: str = _SCHEMA_VERSION

    def as_dict(self) -> dict[str, Any]:
        """Return the stable versioned wire representation."""
        return {
            "schemaVersion": self.schema_version,
            "id": self.collection_id,
            "title": self.title,
            "folders": [folder.as_dict() for folder in self.folders],
        }


def _invalid(code: str, param: str, message: str) -> NoReturn:
    raise WorkspaceValidationError(code=code, param=param, message=message) from None


def _string(
    value: Any,
    *,
    param: str,
    max_length: int,
    required: bool = True,
) -> str:
    if value is None and not required:
        return ""
    if not isinstance(value, str):
        _invalid("invalid_workspace_descriptor", param, f"{param} must be a string.")
    value = value.strip()
    if required and not value:
        _invalid("invalid_workspace_descriptor", param, f"{param} must not be empty.")
    if len(value) > max_length:
        _invalid(
            "workspace_value_too_long",
            param,
            f"{param} exceeds the supported length.",
        )
    return value


def _parse_source(value: Any, *, folder_index: int, source_index: int) -> AddonCatalogSource:
    param = f"folders[{folder_index}].sources[{source_index}]"
    if not isinstance(value, dict):
        _invalid("invalid_workspace_descriptor", param, f"{param} must be an object.")
    kind = _string(
        value.get("kind"),
        param=f"{param}.kind",
        max_length=64,
    )
    if kind != _ADDON_CATALOG_KIND:
        _invalid(
            "unsupported_workspace_source",
            f"{param}.kind",
            "Only declarative add-on catalog references are supported.",
        )
    disallowed = {
        "url",
        "configuredUrl",
        "token",
        "password",
        "plugin",
        "code",
        "items",
        "media",
    }
    if disallowed.intersection(value):
        _invalid(
            "unsafe_workspace_source",
            param,
            "Workspace source contains data that must remain outside the portable descriptor.",
        )
    addon_id = _string(
        value.get("addonId"),
        param=f"{param}.addonId",
        max_length=_MAX_ADDON_ID_LENGTH,
    )
    raw_type = _string(
        value.get("type"),
        param=f"{param}.type",
        max_length=64,
    )
    catalog_id = _string(
        value.get("catalogId"),
        param=f"{param}.catalogId",
        max_length=_MAX_CATALOG_ID_LENGTH,
    )
    genre = _string(
        value.get("genre"),
        param=f"{param}.genre",
        max_length=_MAX_GENRE_LENGTH,
        required=False,
    ) or None
    return AddonCatalogSource(
        addon_id=addon_id,
        content_type=classify_external_content_type(raw_type),
        catalog_id=catalog_id,
        genre=genre,
    )


def _parse_folder(value: Any, index: int) -> WorkspaceFolder:
    param = f"folders[{index}]"
    if not isinstance(value, dict):
        _invalid("invalid_workspace_descriptor", param, f"{param} must be an object.")
    folder_id = _string(
        value.get("id"),
        param=f"{param}.id",
        max_length=_MAX_FOLDER_ID_LENGTH,
    )
    title = _string(
        value.get("title"),
        param=f"{param}.title",
        max_length=_MAX_TITLE_LENGTH,
    )
    raw_sources = value.get("sources") or []
    if not isinstance(raw_sources, list):
        _invalid(
            "invalid_workspace_descriptor",
            f"{param}.sources",
            f"{param}.sources must be an array.",
        )
    if len(raw_sources) > _MAX_SOURCES_PER_FOLDER:
        _invalid(
            "workspace_too_large",
            f"{param}.sources",
            f"{param}.sources contains too many entries.",
        )
    sources = tuple(
        _parse_source(source, folder_index=index, source_index=source_index)
        for source_index, source in enumerate(raw_sources)
    )
    return WorkspaceFolder(folder_id=folder_id, title=title, sources=sources)


def parse_workspace_collection(payload: Any) -> WorkspaceCollection:
    """Parse a bounded, reference-only portable collection descriptor."""
    if not isinstance(payload, dict):
        _invalid(
            "invalid_workspace_descriptor",
            "body",
            "Workspace descriptor must be an object.",
        )
    schema_version = _string(
        payload.get("schemaVersion"),
        param="schemaVersion",
        max_length=32,
    )
    if schema_version != _SCHEMA_VERSION:
        _invalid(
            "unsupported_workspace_version",
            "schemaVersion",
            f"Supported workspace schema version is {_SCHEMA_VERSION}.",
        )
    collection_id = _string(
        payload.get("id"),
        param="id",
        max_length=_MAX_COLLECTION_ID_LENGTH,
    )
    title = _string(
        payload.get("title"),
        param="title",
        max_length=_MAX_TITLE_LENGTH,
    )
    raw_folders = payload.get("folders") or []
    if not isinstance(raw_folders, list):
        _invalid(
            "invalid_workspace_descriptor",
            "folders",
            "folders must be an array.",
        )
    if len(raw_folders) > _MAX_FOLDERS:
        _invalid("workspace_too_large", "folders", "folders contains too many entries.")
    folders = tuple(_parse_folder(folder, index) for index, folder in enumerate(raw_folders))
    return WorkspaceCollection(
        collection_id=collection_id,
        title=title,
        folders=folders,
        schema_version=schema_version,
    )
