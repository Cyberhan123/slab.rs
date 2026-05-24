from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.plugin_capability_transport_type import PluginCapabilityTransportType

T = TypeVar("T", bound="PluginCapabilityTransport")


@_attrs_define
class PluginCapabilityTransport:
    """
    Attributes:
        function (str):
        type_ (PluginCapabilityTransportType):
    """

    function: str
    type_: PluginCapabilityTransportType
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        function = self.function

        type_ = self.type_.value

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "function": function,
                "type": type_,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        function = d.pop("function")

        type_ = PluginCapabilityTransportType(d.pop("type"))

        plugin_capability_transport = cls(
            function=function,
            type_=type_,
        )

        plugin_capability_transport.additional_properties = d
        return plugin_capability_transport

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
