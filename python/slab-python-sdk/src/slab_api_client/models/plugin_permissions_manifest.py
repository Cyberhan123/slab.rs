from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.plugin_file_permissions import PluginFilePermissions
    from ..models.plugin_network_manifest import PluginNetworkManifest


T = TypeVar("T", bound="PluginPermissionsManifest")


@_attrs_define
class PluginPermissionsManifest:
    """
    Attributes:
        agent (list[str] | Unset):
        files (PluginFilePermissions | Unset):
        lsp (list[str] | Unset):
        network (PluginNetworkManifest | Unset):
        slab_api (list[str] | Unset):
        ui (list[str] | Unset):
    """

    agent: list[str] | Unset = UNSET
    files: PluginFilePermissions | Unset = UNSET
    lsp: list[str] | Unset = UNSET
    network: PluginNetworkManifest | Unset = UNSET
    slab_api: list[str] | Unset = UNSET
    ui: list[str] | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        agent: list[str] | Unset = UNSET
        if not isinstance(self.agent, Unset):
            agent = self.agent

        files: dict[str, Any] | Unset = UNSET
        if not isinstance(self.files, Unset):
            files = self.files.to_dict()

        lsp: list[str] | Unset = UNSET
        if not isinstance(self.lsp, Unset):
            lsp = self.lsp

        network: dict[str, Any] | Unset = UNSET
        if not isinstance(self.network, Unset):
            network = self.network.to_dict()

        slab_api: list[str] | Unset = UNSET
        if not isinstance(self.slab_api, Unset):
            slab_api = self.slab_api

        ui: list[str] | Unset = UNSET
        if not isinstance(self.ui, Unset):
            ui = self.ui

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if agent is not UNSET:
            field_dict["agent"] = agent
        if files is not UNSET:
            field_dict["files"] = files
        if lsp is not UNSET:
            field_dict["lsp"] = lsp
        if network is not UNSET:
            field_dict["network"] = network
        if slab_api is not UNSET:
            field_dict["slabApi"] = slab_api
        if ui is not UNSET:
            field_dict["ui"] = ui

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.plugin_file_permissions import PluginFilePermissions
        from ..models.plugin_network_manifest import PluginNetworkManifest

        d = dict(src_dict)
        agent = cast(list[str], d.pop("agent", UNSET))

        _files = d.pop("files", UNSET)
        files: PluginFilePermissions | Unset
        if isinstance(_files, Unset):
            files = UNSET
        else:
            files = PluginFilePermissions.from_dict(_files)

        lsp = cast(list[str], d.pop("lsp", UNSET))

        _network = d.pop("network", UNSET)
        network: PluginNetworkManifest | Unset
        if isinstance(_network, Unset):
            network = UNSET
        else:
            network = PluginNetworkManifest.from_dict(_network)

        slab_api = cast(list[str], d.pop("slabApi", UNSET))

        ui = cast(list[str], d.pop("ui", UNSET))

        plugin_permissions_manifest = cls(
            agent=agent,
            files=files,
            lsp=lsp,
            network=network,
            slab_api=slab_api,
            ui=ui,
        )

        plugin_permissions_manifest.additional_properties = d
        return plugin_permissions_manifest

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
