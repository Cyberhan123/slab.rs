from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.workspace_git_status_entry import WorkspaceGitStatusEntry
    from ..models.workspace_git_status_summary import WorkspaceGitStatusSummary


T = TypeVar("T", bound="WorkspaceGitStatusView")


@_attrs_define
class WorkspaceGitStatusView:
    """
    Attributes:
        available (bool):
        entries (list[WorkspaceGitStatusEntry]):
        is_repository (bool):
        summary (WorkspaceGitStatusSummary):
        branch (None | str | Unset):
        message (None | str | Unset):
        repository_root (None | str | Unset):
    """

    available: bool
    entries: list[WorkspaceGitStatusEntry]
    is_repository: bool
    summary: WorkspaceGitStatusSummary
    branch: None | str | Unset = UNSET
    message: None | str | Unset = UNSET
    repository_root: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        available = self.available

        entries = []
        for entries_item_data in self.entries:
            entries_item = entries_item_data.to_dict()
            entries.append(entries_item)

        is_repository = self.is_repository

        summary = self.summary.to_dict()

        branch: None | str | Unset
        if isinstance(self.branch, Unset):
            branch = UNSET
        else:
            branch = self.branch

        message: None | str | Unset
        if isinstance(self.message, Unset):
            message = UNSET
        else:
            message = self.message

        repository_root: None | str | Unset
        if isinstance(self.repository_root, Unset):
            repository_root = UNSET
        else:
            repository_root = self.repository_root

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "available": available,
                "entries": entries,
                "isRepository": is_repository,
                "summary": summary,
            }
        )
        if branch is not UNSET:
            field_dict["branch"] = branch
        if message is not UNSET:
            field_dict["message"] = message
        if repository_root is not UNSET:
            field_dict["repositoryRoot"] = repository_root

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.workspace_git_status_entry import WorkspaceGitStatusEntry
        from ..models.workspace_git_status_summary import WorkspaceGitStatusSummary

        d = dict(src_dict)
        available = d.pop("available")

        entries = []
        _entries = d.pop("entries")
        for entries_item_data in _entries:
            entries_item = WorkspaceGitStatusEntry.from_dict(entries_item_data)

            entries.append(entries_item)

        is_repository = d.pop("isRepository")

        summary = WorkspaceGitStatusSummary.from_dict(d.pop("summary"))

        def _parse_branch(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        branch = _parse_branch(d.pop("branch", UNSET))

        def _parse_message(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        message = _parse_message(d.pop("message", UNSET))

        def _parse_repository_root(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        repository_root = _parse_repository_root(d.pop("repositoryRoot", UNSET))

        workspace_git_status_view = cls(
            available=available,
            entries=entries,
            is_repository=is_repository,
            summary=summary,
            branch=branch,
            message=message,
            repository_root=repository_root,
        )

        workspace_git_status_view.additional_properties = d
        return workspace_git_status_view

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
