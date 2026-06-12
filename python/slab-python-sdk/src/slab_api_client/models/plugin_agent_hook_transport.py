from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.plugin_agent_hook_runtime import PluginAgentHookRuntime

T = TypeVar("T", bound="PluginAgentHookTransport")


@_attrs_define
class PluginAgentHookTransport:
    """
    Attributes:
        function (str):
        runtime (PluginAgentHookRuntime):
    """

    function: str
    runtime: PluginAgentHookRuntime
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        function = self.function

        runtime = self.runtime.value

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "function": function,
                "runtime": runtime,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        function = d.pop("function")

        runtime = PluginAgentHookRuntime(d.pop("runtime"))

        plugin_agent_hook_transport = cls(
            function=function,
            runtime=runtime,
        )

        plugin_agent_hook_transport.additional_properties = d
        return plugin_agent_hook_transport

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
