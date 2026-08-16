from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

T = TypeVar("T", bound="WorkspaceMigrationSummary")


@_attrs_define
class WorkspaceMigrationSummary:
    """Summary of the migration folded into a [`WorkspaceStateResponse`] when a
    workspace switch interrupted + snapshotted the originating workspace.

        Attributes:
            project_id (str):
            suspended_count (int):
    """

    project_id: str
    suspended_count: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        project_id = self.project_id

        suspended_count = self.suspended_count

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "projectId": project_id,
                "suspendedCount": suspended_count,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        d = dict(src_dict)
        project_id = d.pop("projectId")

        suspended_count = d.pop("suspendedCount")

        workspace_migration_summary = cls(
            project_id=project_id,
            suspended_count=suspended_count,
        )

        workspace_migration_summary.additional_properties = d
        return workspace_migration_summary

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
