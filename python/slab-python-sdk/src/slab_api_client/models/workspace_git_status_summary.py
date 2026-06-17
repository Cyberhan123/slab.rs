from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="WorkspaceGitStatusSummary")


@_attrs_define
class WorkspaceGitStatusSummary:
    """
    Attributes:
        added (int):
        conflicted (int):
        copied (int):
        deleted (int):
        modified (int):
        renamed (int):
        untracked (int):
    """

    added: int
    conflicted: int
    copied: int
    deleted: int
    modified: int
    renamed: int
    untracked: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        added = self.added

        conflicted = self.conflicted

        copied = self.copied

        deleted = self.deleted

        modified = self.modified

        renamed = self.renamed

        untracked = self.untracked

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "added": added,
                "conflicted": conflicted,
                "copied": copied,
                "deleted": deleted,
                "modified": modified,
                "renamed": renamed,
                "untracked": untracked,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        added = d.pop("added")

        conflicted = d.pop("conflicted")

        copied = d.pop("copied")

        deleted = d.pop("deleted")

        modified = d.pop("modified")

        renamed = d.pop("renamed")

        untracked = d.pop("untracked")

        workspace_git_status_summary = cls(
            added=added,
            conflicted=conflicted,
            copied=copied,
            deleted=deleted,
            modified=modified,
            renamed=renamed,
            untracked=untracked,
        )

        workspace_git_status_summary.additional_properties = d
        return workspace_git_status_summary

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
