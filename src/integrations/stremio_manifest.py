"""Bounded declarative Stremio manifest parsing for local media integrations."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from typing import Any

from integrations.safe_fetch import SafeFetchError, fetch_json

_MAX_MANIFEST_ID_LENGTH = 255
_MAX_NAME_LENGTH = 255
_MAX_VERSION_LENGTH = 64
_MAX_DESCRIPTION_LENGTH = 4096
_MAX_TYPE_LENGTH = 64
_MAX_RESOURCE_LENGTH = 64
_MAX_CATALOGS = 256
_MAX_TYPES = 64
_MAX_RESOURCES = 64
_MAX_EXTRAS_PER_CATALOG = 32
_MAX_EXTRA_OPTIONS = 256
_CONSUMABLE_RESOURCES = frozenset({"catalog", "meta"})
_EXECUTABLE_OR_PLAYBACK_RESOURCES = frozenset({"stream", "subtitles"})


@dataclass(frozen=True, slots=True)
class ManifestValidationError(ValueError):
    """Stable validation detail for an untrusted declarative add-on manifest."""

    code: str
    param: str
    message: str

    def __str__(self) -> str:
        """Return the stable public-safe validation message."""
        return self.message


@dataclass(frozen=True, slots=True)
class CatalogExtra:
    """One bounded declarative Stremio catalog extra."""

    name: str
    required: bool
    options: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class CatalogDescriptor:
    """One declarative remote catalog exposed by a Stremio-compatible add-on."""

    content_type: str
    catalog_id: str
    name: str
    extras: tuple[CatalogExtra, ...]


@dataclass(frozen=True, slots=True)
class DeclarativeAddonManifest:
    """Safe capability projection of one Stremio-compatible manifest."""

    addon_id: str
    name: str
    version: str
    description: str
    raw_types: tuple[str, ...]
    declared_resources: tuple[str, ...]
    consumable_resources: tuple[str, ...]
    ignored_resources: tuple[str, ...]
    catalogs: tuple[CatalogDescriptor, ...]
    origin: str
    configuration_digest: str

    @property
    def provides_catalogs(self) -> bool:
        """Return whether Floppy can consume at least one declared catalog."""
        return "catalog" in self.consumable_resources and bool(self.catalogs)

    @property
    def provides_metadata(self) -> bool:
        """Return whether Floppy can consume declarative metadata."""
        return "meta" in self.consumable_resources


def _invalid(code: str, param: str, message: str):
    raise ManifestValidationError(code=code, param=param, message=message) from None


def _bounded_string(
    value: Any,
    *,
    param: str,
    max_length: int,
    required: bool = True,
) -> str:
    """Return one trimmed bounded scalar string."""
    if value is None and not required:
        return ""
    if not isinstance(value, str):
        _invalid("invalid_manifest", param, f"{param} must be a string.")
    value = value.strip()
    if required and not value:
        _invalid("invalid_manifest", param, f"{param} must not be empty.")
    if len(value) > max_length:
        _invalid(
            "manifest_value_too_long",
            param,
            f"{param} exceeds the supported length.",
        )
    return value


def _bounded_string_list(
    value: Any,
    *,
    param: str,
    max_items: int,
    max_length: int,
) -> tuple[str, ...]:
    """Normalize a bounded ordered unique list of strings."""
    if value is None:
        return ()
    if not isinstance(value, list):
        _invalid("invalid_manifest", param, f"{param} must be an array.")
    if len(value) > max_items:
        _invalid("manifest_too_large", param, f"{param} contains too many entries.")
    normalized = []
    for index, raw_value in enumerate(value):
        item = _bounded_string(
            raw_value,
            param=f"{param}[{index}]",
            max_length=max_length,
        )
        if item not in normalized:
            normalized.append(item)
    return tuple(normalized)


def _resource_name(value: Any, index: int) -> str:
    """Return the resource name from either Stremio string or object syntax."""
    if isinstance(value, str):
        return _bounded_string(
            value,
            param=f"resources[{index}]",
            max_length=_MAX_RESOURCE_LENGTH,
        )
    if isinstance(value, dict):
        return _bounded_string(
            value.get("name"),
            param=f"resources[{index}].name",
            max_length=_MAX_RESOURCE_LENGTH,
        )
    _invalid(
        "invalid_manifest",
        f"resources[{index}]",
        "Each resource must be a string or object.",
    )


def _parse_resources(value: Any) -> tuple[str, ...]:
    """Return bounded resource names while preserving declaration order."""
    if not isinstance(value, list) or not value:
        _invalid("invalid_manifest", "resources", "resources must be a non-empty array.")
    if len(value) > _MAX_RESOURCES:
        _invalid("manifest_too_large", "resources", "resources contains too many entries.")
    resources = []
    for index, raw_resource in enumerate(value):
        name = _resource_name(raw_resource, index).lower()
        if name not in resources:
            resources.append(name)
    return tuple(resources)


def _parse_extra(value: Any, *, catalog_index: int, extra_index: int) -> CatalogExtra:
    """Parse one bounded catalog extra descriptor."""
    param = f"catalogs[{catalog_index}].extra[{extra_index}]"
    if not isinstance(value, dict):
        _invalid("invalid_manifest", param, f"{param} must be an object.")
    name = _bounded_string(
        value.get("name"),
        param=f"{param}.name",
        max_length=_MAX_NAME_LENGTH,
    )
    required = value.get("isRequired", False)
    if not isinstance(required, bool):
        _invalid(
            "invalid_manifest",
            f"{param}.isRequired",
            f"{param}.isRequired must be true or false.",
        )
    options = _bounded_string_list(
        value.get("options"),
        param=f"{param}.options",
        max_items=_MAX_EXTRA_OPTIONS,
        max_length=_MAX_NAME_LENGTH,
    )
    return CatalogExtra(name=name, required=required, options=options)


def _parse_catalog(value: Any, index: int) -> CatalogDescriptor:
    """Parse one bounded Stremio catalog descriptor."""
    param = f"catalogs[{index}]"
    if not isinstance(value, dict):
        _invalid("invalid_manifest", param, f"{param} must be an object.")
    content_type = _bounded_string(
        value.get("type"),
        param=f"{param}.type",
        max_length=_MAX_TYPE_LENGTH,
    )
    catalog_id = _bounded_string(
        value.get("id"),
        param=f"{param}.id",
        max_length=_MAX_MANIFEST_ID_LENGTH,
    )
    name = _bounded_string(
        value.get("name") or catalog_id,
        param=f"{param}.name",
        max_length=_MAX_NAME_LENGTH,
    )
    raw_extras = value.get("extra") or []
    if not isinstance(raw_extras, list):
        _invalid("invalid_manifest", f"{param}.extra", f"{param}.extra must be an array.")
    if len(raw_extras) > _MAX_EXTRAS_PER_CATALOG:
        _invalid(
            "manifest_too_large",
            f"{param}.extra",
            f"{param}.extra contains too many entries.",
        )
    extras = tuple(
        _parse_extra(extra, catalog_index=index, extra_index=extra_index)
        for extra_index, extra in enumerate(raw_extras)
    )
    return CatalogDescriptor(
        content_type=content_type,
        catalog_id=catalog_id,
        name=name,
        extras=extras,
    )


def parse_manifest(
    payload: Any,
    *,
    origin: str,
    configuration_digest: str,
) -> DeclarativeAddonManifest:
    """Parse one manifest into the safe Floppy capability projection."""
    if not isinstance(payload, dict):
        _invalid("invalid_manifest", "body", "Manifest body must be an object.")

    addon_id = _bounded_string(
        payload.get("id"),
        param="id",
        max_length=_MAX_MANIFEST_ID_LENGTH,
    )
    name = _bounded_string(
        payload.get("name"),
        param="name",
        max_length=_MAX_NAME_LENGTH,
    )
    version = _bounded_string(
        payload.get("version"),
        param="version",
        max_length=_MAX_VERSION_LENGTH,
    )
    description = _bounded_string(
        payload.get("description"),
        param="description",
        max_length=_MAX_DESCRIPTION_LENGTH,
        required=False,
    )
    raw_types = _bounded_string_list(
        payload.get("types"),
        param="types",
        max_items=_MAX_TYPES,
        max_length=_MAX_TYPE_LENGTH,
    )
    if not raw_types:
        _invalid("invalid_manifest", "types", "types must include at least one content type.")
    resources = _parse_resources(payload.get("resources"))

    raw_catalogs = payload.get("catalogs") or []
    if not isinstance(raw_catalogs, list):
        _invalid("invalid_manifest", "catalogs", "catalogs must be an array.")
    if len(raw_catalogs) > _MAX_CATALOGS:
        _invalid("manifest_too_large", "catalogs", "catalogs contains too many entries.")
    catalogs = tuple(_parse_catalog(value, index) for index, value in enumerate(raw_catalogs))

    consumable = tuple(
        resource for resource in resources if resource in _CONSUMABLE_RESOURCES
    )
    ignored = tuple(resource for resource in resources if resource not in consumable)
    if not consumable:
        _invalid(
            "unsupported_manifest",
            "resources",
            "Manifest does not expose a catalog or metadata resource Floppy can consume.",
        )
    if any(resource in _EXECUTABLE_OR_PLAYBACK_RESOURCES for resource in resources):
        # Playback/executable-adjacent resources remain visible as ignored capability
        # data but never become executable Floppy behavior.
        ignored = tuple(dict.fromkeys((*ignored, *(_EXECUTABLE_OR_PLAYBACK_RESOURCES & set(resources)))))

    return DeclarativeAddonManifest(
        addon_id=addon_id,
        name=name,
        version=version,
        description=description,
        raw_types=raw_types,
        declared_resources=resources,
        consumable_resources=consumable,
        ignored_resources=ignored,
        catalogs=catalogs,
        origin=origin,
        configuration_digest=configuration_digest,
    )


def load_manifest(raw_url: str) -> DeclarativeAddonManifest:
    """Fetch and parse one public manifest without persisting or exposing its URL."""
    payload, result = fetch_json(raw_url)
    digest = hashlib.sha256(raw_url.encode("utf-8")).hexdigest()
    try:
        return parse_manifest(
            payload,
            origin=result.origin,
            configuration_digest=digest,
        )
    except ManifestValidationError:
        raise
    except (TypeError, ValueError) as exc:
        raise ManifestValidationError(
            code="invalid_manifest",
            param="body",
            message="Remote manifest is invalid.",
        ) from exc


__all__ = [
    "CatalogDescriptor",
    "CatalogExtra",
    "DeclarativeAddonManifest",
    "ManifestValidationError",
    "SafeFetchError",
    "load_manifest",
    "parse_manifest",
]
