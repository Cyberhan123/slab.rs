from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.agent_tool_choice_input_type_3_type import AgentToolChoiceInputType3Type

T = TypeVar("T", bound="AgentToolChoiceInputType3")


@_attrs_define
class AgentToolChoiceInputType3:
    """
    Attributes:
        name (str):
        type_ (AgentToolChoiceInputType3Type):
    """

    name: str
    type_: AgentToolChoiceInputType3Type
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        name = self.name

        type_ = self.type_.value

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "name": name,
                "type": type_,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        name = d.pop("name")

        type_ = AgentToolChoiceInputType3Type(d.pop("type"))

        agent_tool_choice_input_type_3 = cls(
            name=name,
            type_=type_,
        )

        agent_tool_choice_input_type_3.additional_properties = d
        return agent_tool_choice_input_type_3

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
