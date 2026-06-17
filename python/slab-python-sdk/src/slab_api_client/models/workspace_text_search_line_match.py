from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="WorkspaceTextSearchLineMatch")


@_attrs_define
class WorkspaceTextSearchLineMatch:
    """
    Attributes:
        line_number (int):
        line_text (str):
        match_end (int):
        match_start (int):
    """

    line_number: int
    line_text: str
    match_end: int
    match_start: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        line_number = self.line_number

        line_text = self.line_text

        match_end = self.match_end

        match_start = self.match_start

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "lineNumber": line_number,
                "lineText": line_text,
                "matchEnd": match_end,
                "matchStart": match_start,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        line_number = d.pop("lineNumber")

        line_text = d.pop("lineText")

        match_end = d.pop("matchEnd")

        match_start = d.pop("matchStart")

        workspace_text_search_line_match = cls(
            line_number=line_number,
            line_text=line_text,
            match_end=match_end,
            match_start=match_start,
        )

        workspace_text_search_line_match.additional_properties = d
        return workspace_text_search_line_match

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
