from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.plugin_capability_kind import PluginCapabilityKind
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.plugin_capability_transport import PluginCapabilityTransport


T = TypeVar("T", bound="PluginAgentCapabilityContribution")


@_attrs_define
class PluginAgentCapabilityContribution:
    """
    Attributes:
        id (str):
        kind (PluginCapabilityKind):
        transport (PluginCapabilityTransport):
        description (None | str | Unset):
        description_key (None | str | Unset):
        effects (list[str] | Unset):
        expose_as_mcp_tool (bool | Unset):
        input_schema (None | str | Unset):
        output_schema (None | str | Unset):
    """

    id: str
    kind: PluginCapabilityKind
    transport: PluginCapabilityTransport
    description: None | str | Unset = UNSET
    description_key: None | str | Unset = UNSET
    effects: list[str] | Unset = UNSET
    expose_as_mcp_tool: bool | Unset = UNSET
    input_schema: None | str | Unset = UNSET
    output_schema: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        id = self.id

        kind = self.kind.value

        transport = self.transport.to_dict()

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        description_key: None | str | Unset
        if isinstance(self.description_key, Unset):
            description_key = UNSET
        else:
            description_key = self.description_key

        effects: list[str] | Unset = UNSET
        if not isinstance(self.effects, Unset):
            effects = self.effects

        expose_as_mcp_tool = self.expose_as_mcp_tool

        input_schema: None | str | Unset
        if isinstance(self.input_schema, Unset):
            input_schema = UNSET
        else:
            input_schema = self.input_schema

        output_schema: None | str | Unset
        if isinstance(self.output_schema, Unset):
            output_schema = UNSET
        else:
            output_schema = self.output_schema

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "id": id,
                "kind": kind,
                "transport": transport,
            }
        )
        if description is not UNSET:
            field_dict["description"] = description
        if description_key is not UNSET:
            field_dict["descriptionKey"] = description_key
        if effects is not UNSET:
            field_dict["effects"] = effects
        if expose_as_mcp_tool is not UNSET:
            field_dict["exposeAsMcpTool"] = expose_as_mcp_tool
        if input_schema is not UNSET:
            field_dict["inputSchema"] = input_schema
        if output_schema is not UNSET:
            field_dict["outputSchema"] = output_schema

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.plugin_capability_transport import PluginCapabilityTransport

        d = dict(src_dict)
        id = d.pop("id")

        kind = PluginCapabilityKind(d.pop("kind"))

        transport = PluginCapabilityTransport.from_dict(d.pop("transport"))

        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))

        def _parse_description_key(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description_key = _parse_description_key(d.pop("descriptionKey", UNSET))

        effects = cast(list[str], d.pop("effects", UNSET))

        expose_as_mcp_tool = d.pop("exposeAsMcpTool", UNSET)

        def _parse_input_schema(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        input_schema = _parse_input_schema(d.pop("inputSchema", UNSET))

        def _parse_output_schema(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        output_schema = _parse_output_schema(d.pop("outputSchema", UNSET))

        plugin_agent_capability_contribution = cls(
            id=id,
            kind=kind,
            transport=transport,
            description=description,
            description_key=description_key,
            effects=effects,
            expose_as_mcp_tool=expose_as_mcp_tool,
            input_schema=input_schema,
            output_schema=output_schema,
        )

        plugin_agent_capability_contribution.additional_properties = d
        return plugin_agent_capability_contribution

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
