from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.plugin_language_server_transport_type_1_type import (
    PluginLanguageServerTransportType1Type,
)

T = TypeVar("T", bound="PluginLanguageServerTransportType1")


@_attrs_define
class PluginLanguageServerTransportType1:
    """
    Attributes:
        type_ (PluginLanguageServerTransportType1Type):
        url (str):
    """

    type_: PluginLanguageServerTransportType1Type
    url: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        type_ = self.type_.value

        url = self.url

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "type": type_,
                "url": url,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        type_ = PluginLanguageServerTransportType1Type(d.pop("type"))

        url = d.pop("url")

        plugin_language_server_transport_type_1 = cls(
            type_=type_,
            url=url,
        )

        plugin_language_server_transport_type_1.additional_properties = d
        return plugin_language_server_transport_type_1

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
