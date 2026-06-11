from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.agent_structured_output_input_type_1_type import (
    AgentStructuredOutputInputType1Type,
)
from ..types import UNSET, Unset

T = TypeVar("T", bound="AgentStructuredOutputInputType1")


@_attrs_define
class AgentStructuredOutputInputType1:
    """
    Attributes:
        schema (Any):
        type_ (AgentStructuredOutputInputType1Type):
        description (None | str | Unset):
        name (None | str | Unset):
        strict (bool | None | Unset):
    """

    schema: Any
    type_: AgentStructuredOutputInputType1Type
    description: None | str | Unset = UNSET
    name: None | str | Unset = UNSET
    strict: bool | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        schema = self.schema

        type_ = self.type_.value

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        name: None | str | Unset
        if isinstance(self.name, Unset):
            name = UNSET
        else:
            name = self.name

        strict: bool | None | Unset
        if isinstance(self.strict, Unset):
            strict = UNSET
        else:
            strict = self.strict

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "schema": schema,
                "type": type_,
            }
        )
        if description is not UNSET:
            field_dict["description"] = description
        if name is not UNSET:
            field_dict["name"] = name
        if strict is not UNSET:
            field_dict["strict"] = strict

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        schema = d.pop("schema")

        type_ = AgentStructuredOutputInputType1Type(d.pop("type"))

        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))

        def _parse_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        name = _parse_name(d.pop("name", UNSET))

        def _parse_strict(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        strict = _parse_strict(d.pop("strict", UNSET))

        agent_structured_output_input_type_1 = cls(
            schema=schema,
            type_=type_,
            description=description,
            name=name,
            strict=strict,
        )

        agent_structured_output_input_type_1.additional_properties = d
        return agent_structured_output_input_type_1

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
