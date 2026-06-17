from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.workspace_text_search_line_match import WorkspaceTextSearchLineMatch


T = TypeVar("T", bound="WorkspaceTextSearchFileMatch")


@_attrs_define
class WorkspaceTextSearchFileMatch:
    """
    Attributes:
        line_matches (list[WorkspaceTextSearchLineMatch]):
        name (str):
        relative_path (str):
    """

    line_matches: list[WorkspaceTextSearchLineMatch]
    name: str
    relative_path: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        line_matches = []
        for line_matches_item_data in self.line_matches:
            line_matches_item = line_matches_item_data.to_dict()
            line_matches.append(line_matches_item)

        name = self.name

        relative_path = self.relative_path

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "lineMatches": line_matches,
                "name": name,
                "relativePath": relative_path,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.workspace_text_search_line_match import (
            WorkspaceTextSearchLineMatch,
        )

        d = dict(src_dict)
        line_matches = []
        _line_matches = d.pop("lineMatches")
        for line_matches_item_data in _line_matches:
            line_matches_item = WorkspaceTextSearchLineMatch.from_dict(
                line_matches_item_data
            )

            line_matches.append(line_matches_item)

        name = d.pop("name")

        relative_path = d.pop("relativePath")

        workspace_text_search_file_match = cls(
            line_matches=line_matches,
            name=name,
            relative_path=relative_path,
        )

        workspace_text_search_file_match.additional_properties = d
        return workspace_text_search_file_match

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
