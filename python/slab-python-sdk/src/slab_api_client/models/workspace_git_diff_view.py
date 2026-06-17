from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="WorkspaceGitDiffView")


@_attrs_define
class WorkspaceGitDiffView:
    """
    Attributes:
        diff (str):
        path (str):
        staged (bool):
    """

    diff: str
    path: str
    staged: bool
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        diff = self.diff

        path = self.path

        staged = self.staged

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "diff": diff,
                "path": path,
                "staged": staged,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        diff = d.pop("diff")

        path = d.pop("path")

        staged = d.pop("staged")

        workspace_git_diff_view = cls(
            diff=diff,
            path=path,
            staged=staged,
        )

        workspace_git_diff_view.additional_properties = d
        return workspace_git_diff_view

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
