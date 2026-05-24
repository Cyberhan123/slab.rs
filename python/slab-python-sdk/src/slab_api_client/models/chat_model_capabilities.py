from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="ChatModelCapabilities")


@_attrs_define
class ChatModelCapabilities:
    """
    Attributes:
        raw_gbnf (bool):
        reasoning_controls (bool):
        structured_output (bool):
    """

    raw_gbnf: bool
    reasoning_controls: bool
    structured_output: bool
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        raw_gbnf = self.raw_gbnf

        reasoning_controls = self.reasoning_controls

        structured_output = self.structured_output

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "raw_gbnf": raw_gbnf,
                "reasoning_controls": reasoning_controls,
                "structured_output": structured_output,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        raw_gbnf = d.pop("raw_gbnf")

        reasoning_controls = d.pop("reasoning_controls")

        structured_output = d.pop("structured_output")

        chat_model_capabilities = cls(
            raw_gbnf=raw_gbnf,
            reasoning_controls=reasoning_controls,
            structured_output=structured_output,
        )

        chat_model_capabilities.additional_properties = d
        return chat_model_capabilities

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
