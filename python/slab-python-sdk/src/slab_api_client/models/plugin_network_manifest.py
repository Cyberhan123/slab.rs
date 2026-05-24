from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.plugin_network_mode import PluginNetworkMode
from ..types import UNSET, Unset

T = TypeVar("T", bound="PluginNetworkManifest")


@_attrs_define
class PluginNetworkManifest:
    """
    Attributes:
        mode (PluginNetworkMode):
        allow_hosts (list[str] | Unset):
    """

    mode: PluginNetworkMode
    allow_hosts: list[str] | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        mode = self.mode.value

        allow_hosts: list[str] | Unset = UNSET
        if not isinstance(self.allow_hosts, Unset):
            allow_hosts = self.allow_hosts

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "mode": mode,
            }
        )
        if allow_hosts is not UNSET:
            field_dict["allowHosts"] = allow_hosts

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        mode = PluginNetworkMode(d.pop("mode"))

        allow_hosts = cast(list[str], d.pop("allowHosts", UNSET))

        plugin_network_manifest = cls(
            mode=mode,
            allow_hosts=allow_hosts,
        )

        plugin_network_manifest.additional_properties = d
        return plugin_network_manifest

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
