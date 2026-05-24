from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.plugin_compatibility_manifest import PluginCompatibilityManifest
    from ..models.plugin_contributes_manifest import PluginContributesManifest
    from ..models.plugin_permissions_manifest import PluginPermissionsManifest


T = TypeVar("T", bound="PluginResponse")


@_attrs_define
class PluginResponse:
    """
    Attributes:
        allow_hosts (list[str]):
        enabled (bool):
        has_wasm (bool):
        id (str):
        manifest_version (int):
        name (str):
        network_mode (str):
        removable (bool):
        runtime_status (str):
        source_kind (str):
        update_available (bool):
        valid (bool):
        version (str):
        available_version (None | str | Unset):
        compatibility (None | PluginCompatibilityManifest | Unset):
        contributions (None | PluginContributesManifest | Unset):
        error (None | str | Unset):
        install_root (None | str | Unset):
        installed_at (None | str | Unset):
        installed_version (None | str | Unset):
        last_error (None | str | Unset):
        last_seen_at (None | str | Unset):
        last_started_at (None | str | Unset):
        last_stopped_at (None | str | Unset):
        manifest_hash (None | str | Unset):
        permissions (None | PluginPermissionsManifest | Unset):
        source_ref (None | str | Unset):
        ui_entry (None | str | Unset):
        updated_at (None | str | Unset):
    """

    allow_hosts: list[str]
    enabled: bool
    has_wasm: bool
    id: str
    manifest_version: int
    name: str
    network_mode: str
    removable: bool
    runtime_status: str
    source_kind: str
    update_available: bool
    valid: bool
    version: str
    available_version: None | str | Unset = UNSET
    compatibility: None | PluginCompatibilityManifest | Unset = UNSET
    contributions: None | PluginContributesManifest | Unset = UNSET
    error: None | str | Unset = UNSET
    install_root: None | str | Unset = UNSET
    installed_at: None | str | Unset = UNSET
    installed_version: None | str | Unset = UNSET
    last_error: None | str | Unset = UNSET
    last_seen_at: None | str | Unset = UNSET
    last_started_at: None | str | Unset = UNSET
    last_stopped_at: None | str | Unset = UNSET
    manifest_hash: None | str | Unset = UNSET
    permissions: None | PluginPermissionsManifest | Unset = UNSET
    source_ref: None | str | Unset = UNSET
    ui_entry: None | str | Unset = UNSET
    updated_at: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.plugin_compatibility_manifest import PluginCompatibilityManifest
        from ..models.plugin_contributes_manifest import PluginContributesManifest
        from ..models.plugin_permissions_manifest import PluginPermissionsManifest

        allow_hosts = self.allow_hosts

        enabled = self.enabled

        has_wasm = self.has_wasm

        id = self.id

        manifest_version = self.manifest_version

        name = self.name

        network_mode = self.network_mode

        removable = self.removable

        runtime_status = self.runtime_status

        source_kind = self.source_kind

        update_available = self.update_available

        valid = self.valid

        version = self.version

        available_version: None | str | Unset
        if isinstance(self.available_version, Unset):
            available_version = UNSET
        else:
            available_version = self.available_version

        compatibility: dict[str, Any] | None | Unset
        if isinstance(self.compatibility, Unset):
            compatibility = UNSET
        elif isinstance(self.compatibility, PluginCompatibilityManifest):
            compatibility = self.compatibility.to_dict()
        else:
            compatibility = self.compatibility

        contributions: dict[str, Any] | None | Unset
        if isinstance(self.contributions, Unset):
            contributions = UNSET
        elif isinstance(self.contributions, PluginContributesManifest):
            contributions = self.contributions.to_dict()
        else:
            contributions = self.contributions

        error: None | str | Unset
        if isinstance(self.error, Unset):
            error = UNSET
        else:
            error = self.error

        install_root: None | str | Unset
        if isinstance(self.install_root, Unset):
            install_root = UNSET
        else:
            install_root = self.install_root

        installed_at: None | str | Unset
        if isinstance(self.installed_at, Unset):
            installed_at = UNSET
        else:
            installed_at = self.installed_at

        installed_version: None | str | Unset
        if isinstance(self.installed_version, Unset):
            installed_version = UNSET
        else:
            installed_version = self.installed_version

        last_error: None | str | Unset
        if isinstance(self.last_error, Unset):
            last_error = UNSET
        else:
            last_error = self.last_error

        last_seen_at: None | str | Unset
        if isinstance(self.last_seen_at, Unset):
            last_seen_at = UNSET
        else:
            last_seen_at = self.last_seen_at

        last_started_at: None | str | Unset
        if isinstance(self.last_started_at, Unset):
            last_started_at = UNSET
        else:
            last_started_at = self.last_started_at

        last_stopped_at: None | str | Unset
        if isinstance(self.last_stopped_at, Unset):
            last_stopped_at = UNSET
        else:
            last_stopped_at = self.last_stopped_at

        manifest_hash: None | str | Unset
        if isinstance(self.manifest_hash, Unset):
            manifest_hash = UNSET
        else:
            manifest_hash = self.manifest_hash

        permissions: dict[str, Any] | None | Unset
        if isinstance(self.permissions, Unset):
            permissions = UNSET
        elif isinstance(self.permissions, PluginPermissionsManifest):
            permissions = self.permissions.to_dict()
        else:
            permissions = self.permissions

        source_ref: None | str | Unset
        if isinstance(self.source_ref, Unset):
            source_ref = UNSET
        else:
            source_ref = self.source_ref

        ui_entry: None | str | Unset
        if isinstance(self.ui_entry, Unset):
            ui_entry = UNSET
        else:
            ui_entry = self.ui_entry

        updated_at: None | str | Unset
        if isinstance(self.updated_at, Unset):
            updated_at = UNSET
        else:
            updated_at = self.updated_at

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "allowHosts": allow_hosts,
                "enabled": enabled,
                "hasWasm": has_wasm,
                "id": id,
                "manifestVersion": manifest_version,
                "name": name,
                "networkMode": network_mode,
                "removable": removable,
                "runtimeStatus": runtime_status,
                "sourceKind": source_kind,
                "updateAvailable": update_available,
                "valid": valid,
                "version": version,
            }
        )
        if available_version is not UNSET:
            field_dict["availableVersion"] = available_version
        if compatibility is not UNSET:
            field_dict["compatibility"] = compatibility
        if contributions is not UNSET:
            field_dict["contributions"] = contributions
        if error is not UNSET:
            field_dict["error"] = error
        if install_root is not UNSET:
            field_dict["installRoot"] = install_root
        if installed_at is not UNSET:
            field_dict["installedAt"] = installed_at
        if installed_version is not UNSET:
            field_dict["installedVersion"] = installed_version
        if last_error is not UNSET:
            field_dict["lastError"] = last_error
        if last_seen_at is not UNSET:
            field_dict["lastSeenAt"] = last_seen_at
        if last_started_at is not UNSET:
            field_dict["lastStartedAt"] = last_started_at
        if last_stopped_at is not UNSET:
            field_dict["lastStoppedAt"] = last_stopped_at
        if manifest_hash is not UNSET:
            field_dict["manifestHash"] = manifest_hash
        if permissions is not UNSET:
            field_dict["permissions"] = permissions
        if source_ref is not UNSET:
            field_dict["sourceRef"] = source_ref
        if ui_entry is not UNSET:
            field_dict["uiEntry"] = ui_entry
        if updated_at is not UNSET:
            field_dict["updatedAt"] = updated_at

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.plugin_compatibility_manifest import PluginCompatibilityManifest
        from ..models.plugin_contributes_manifest import PluginContributesManifest
        from ..models.plugin_permissions_manifest import PluginPermissionsManifest

        d = dict(src_dict)
        allow_hosts = cast(list[str], d.pop("allowHosts"))

        enabled = d.pop("enabled")

        has_wasm = d.pop("hasWasm")

        id = d.pop("id")

        manifest_version = d.pop("manifestVersion")

        name = d.pop("name")

        network_mode = d.pop("networkMode")

        removable = d.pop("removable")

        runtime_status = d.pop("runtimeStatus")

        source_kind = d.pop("sourceKind")

        update_available = d.pop("updateAvailable")

        valid = d.pop("valid")

        version = d.pop("version")

        def _parse_available_version(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        available_version = _parse_available_version(d.pop("availableVersion", UNSET))

        def _parse_compatibility(
            data: object,
        ) -> None | PluginCompatibilityManifest | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                compatibility_type_1 = PluginCompatibilityManifest.from_dict(data)

                return compatibility_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | PluginCompatibilityManifest | Unset, data)

        compatibility = _parse_compatibility(d.pop("compatibility", UNSET))

        def _parse_contributions(
            data: object,
        ) -> None | PluginContributesManifest | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                contributions_type_1 = PluginContributesManifest.from_dict(data)

                return contributions_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | PluginContributesManifest | Unset, data)

        contributions = _parse_contributions(d.pop("contributions", UNSET))

        def _parse_error(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        error = _parse_error(d.pop("error", UNSET))

        def _parse_install_root(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        install_root = _parse_install_root(d.pop("installRoot", UNSET))

        def _parse_installed_at(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        installed_at = _parse_installed_at(d.pop("installedAt", UNSET))

        def _parse_installed_version(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        installed_version = _parse_installed_version(d.pop("installedVersion", UNSET))

        def _parse_last_error(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        last_error = _parse_last_error(d.pop("lastError", UNSET))

        def _parse_last_seen_at(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        last_seen_at = _parse_last_seen_at(d.pop("lastSeenAt", UNSET))

        def _parse_last_started_at(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        last_started_at = _parse_last_started_at(d.pop("lastStartedAt", UNSET))

        def _parse_last_stopped_at(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        last_stopped_at = _parse_last_stopped_at(d.pop("lastStoppedAt", UNSET))

        def _parse_manifest_hash(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        manifest_hash = _parse_manifest_hash(d.pop("manifestHash", UNSET))

        def _parse_permissions(
            data: object,
        ) -> None | PluginPermissionsManifest | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                permissions_type_1 = PluginPermissionsManifest.from_dict(data)

                return permissions_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | PluginPermissionsManifest | Unset, data)

        permissions = _parse_permissions(d.pop("permissions", UNSET))

        def _parse_source_ref(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        source_ref = _parse_source_ref(d.pop("sourceRef", UNSET))

        def _parse_ui_entry(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        ui_entry = _parse_ui_entry(d.pop("uiEntry", UNSET))

        def _parse_updated_at(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        updated_at = _parse_updated_at(d.pop("updatedAt", UNSET))

        plugin_response = cls(
            allow_hosts=allow_hosts,
            enabled=enabled,
            has_wasm=has_wasm,
            id=id,
            manifest_version=manifest_version,
            name=name,
            network_mode=network_mode,
            removable=removable,
            runtime_status=runtime_status,
            source_kind=source_kind,
            update_available=update_available,
            valid=valid,
            version=version,
            available_version=available_version,
            compatibility=compatibility,
            contributions=contributions,
            error=error,
            install_root=install_root,
            installed_at=installed_at,
            installed_version=installed_version,
            last_error=last_error,
            last_seen_at=last_seen_at,
            last_started_at=last_started_at,
            last_stopped_at=last_stopped_at,
            manifest_hash=manifest_hash,
            permissions=permissions,
            source_ref=source_ref,
            ui_entry=ui_entry,
            updated_at=updated_at,
        )

        plugin_response.additional_properties = d
        return plugin_response

    @property
    def additional_keys(self) -> list[str]:
        return list(self.additional_properties.keys())

    def __getitem__(self, key: str) -> Any:
        return self.additional_properties[key]

    def __setitem__(self, key: str, value: Any) -> None:
        self.additional_properties[key] = value

    def __delitem__(self, key: str) -> None:
        del self.additional_properties[key]

    def __contains__(self, key: str) -> bool:
        return key in self.additional_properties
