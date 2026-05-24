from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="InstallPluginRequest")


@_attrs_define
class InstallPluginRequest:
    """
    Attributes:
        plugin_id (str):
        package_sha_256 (None | str | Unset):
        package_url (None | str | Unset):
        source_id (None | str | Unset):
        version (None | str | Unset):
    """

    plugin_id: str
    package_sha_256: None | str | Unset = UNSET
    package_url: None | str | Unset = UNSET
    source_id: None | str | Unset = UNSET
    version: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        plugin_id = self.plugin_id

        package_sha_256: None | str | Unset
        if isinstance(self.package_sha_256, Unset):
            package_sha_256 = UNSET
        else:
            package_sha_256 = self.package_sha_256

        package_url: None | str | Unset
        if isinstance(self.package_url, Unset):
            package_url = UNSET
        else:
            package_url = self.package_url

        source_id: None | str | Unset
        if isinstance(self.source_id, Unset):
            source_id = UNSET
        else:
            source_id = self.source_id

        version: None | str | Unset
        if isinstance(self.version, Unset):
            version = UNSET
        else:
            version = self.version

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "pluginId": plugin_id,
            }
        )
        if package_sha_256 is not UNSET:
            field_dict["packageSha256"] = package_sha_256
        if package_url is not UNSET:
            field_dict["packageUrl"] = package_url
        if source_id is not UNSET:
            field_dict["sourceId"] = source_id
        if version is not UNSET:
            field_dict["version"] = version

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        plugin_id = d.pop("pluginId")

        def _parse_package_sha_256(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        package_sha_256 = _parse_package_sha_256(d.pop("packageSha256", UNSET))

        def _parse_package_url(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        package_url = _parse_package_url(d.pop("packageUrl", UNSET))

        def _parse_source_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        source_id = _parse_source_id(d.pop("sourceId", UNSET))

        def _parse_version(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        version = _parse_version(d.pop("version", UNSET))

        install_plugin_request = cls(
            plugin_id=plugin_id,
            package_sha_256=package_sha_256,
            package_url=package_url,
            source_id=source_id,
            version=version,
        )

        install_plugin_request.additional_properties = d
        return install_plugin_request

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
