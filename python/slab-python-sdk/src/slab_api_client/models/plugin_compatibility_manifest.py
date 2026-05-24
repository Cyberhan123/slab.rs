from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="PluginCompatibilityManifest")


@_attrs_define
class PluginCompatibilityManifest:
    """
    Attributes:
        plugin_api (None | str | Unset):
        slab (None | str | Unset):
    """

    plugin_api: None | str | Unset = UNSET
    slab: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        plugin_api: None | str | Unset
        if isinstance(self.plugin_api, Unset):
            plugin_api = UNSET
        else:
            plugin_api = self.plugin_api

        slab: None | str | Unset
        if isinstance(self.slab, Unset):
            slab = UNSET
        else:
            slab = self.slab

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if plugin_api is not UNSET:
            field_dict["pluginApi"] = plugin_api
        if slab is not UNSET:
            field_dict["slab"] = slab

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

        def _parse_plugin_api(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        plugin_api = _parse_plugin_api(d.pop("pluginApi", UNSET))

        def _parse_slab(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        slab = _parse_slab(d.pop("slab", UNSET))

        plugin_compatibility_manifest = cls(
            plugin_api=plugin_api,
            slab=slab,
        )

        plugin_compatibility_manifest.additional_properties = d
        return plugin_compatibility_manifest

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
