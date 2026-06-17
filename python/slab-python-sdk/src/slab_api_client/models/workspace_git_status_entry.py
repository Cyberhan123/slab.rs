from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.workspace_git_file_status import WorkspaceGitFileStatus
from ..types import UNSET, Unset

T = TypeVar("T", bound="WorkspaceGitStatusEntry")


@_attrs_define
class WorkspaceGitStatusEntry:
    """
    Attributes:
        path (str):
        staged (bool):
        status (WorkspaceGitFileStatus):
        original_path (None | str | Unset):
    """

    path: str
    staged: bool
    status: WorkspaceGitFileStatus
    original_path: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        path = self.path

        staged = self.staged

        status = self.status.value

        original_path: None | str | Unset
        if isinstance(self.original_path, Unset):
            original_path = UNSET
        else:
            original_path = self.original_path

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "path": path,
                "staged": staged,
                "status": status,
            }
        )
        if original_path is not UNSET:
            field_dict["originalPath"] = original_path

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        path = d.pop("path")

        staged = d.pop("staged")

        status = WorkspaceGitFileStatus(d.pop("status"))

        def _parse_original_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        original_path = _parse_original_path(d.pop("originalPath", UNSET))

        workspace_git_status_entry = cls(
            path=path,
            staged=staged,
            status=status,
            original_path=original_path,
        )

        workspace_git_status_entry.additional_properties = d
        return workspace_git_status_entry

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
