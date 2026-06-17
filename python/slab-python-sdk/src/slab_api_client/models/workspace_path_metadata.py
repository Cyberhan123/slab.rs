from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.workspace_file_kind import WorkspaceFileKind

T = TypeVar("T", bound="WorkspacePathMetadata")


@_attrs_define
class WorkspacePathMetadata:
    """
    Attributes:
        created_at (int):
        kind (WorkspaceFileKind):
        modified_at (int):
        relative_path (str):
        size_bytes (int):
    """

    created_at: int
    kind: WorkspaceFileKind
    modified_at: int
    relative_path: str
    size_bytes: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        created_at = self.created_at

        kind = self.kind.value

        modified_at = self.modified_at

        relative_path = self.relative_path

        size_bytes = self.size_bytes

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "createdAt": created_at,
                "kind": kind,
                "modifiedAt": modified_at,
                "relativePath": relative_path,
                "sizeBytes": size_bytes,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        created_at = d.pop("createdAt")

        kind = WorkspaceFileKind(d.pop("kind"))

        modified_at = d.pop("modifiedAt")

        relative_path = d.pop("relativePath")

        size_bytes = d.pop("sizeBytes")

        workspace_path_metadata = cls(
            created_at=created_at,
            kind=kind,
            modified_at=modified_at,
            relative_path=relative_path,
            size_bytes=size_bytes,
        )

        workspace_path_metadata.additional_properties = d
        return workspace_path_metadata

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
