from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.workspace_file_kind import WorkspaceFileKind
from ..types import UNSET, Unset

T = TypeVar("T", bound="WorkspaceFileEntry")


@_attrs_define
class WorkspaceFileEntry:
    """
    Attributes:
        has_children (bool):
        id (str):
        kind (WorkspaceFileKind):
        name (str):
        relative_path (str):
        created_at (int | None | Unset):
        modified_at (int | None | Unset):
        size_bytes (int | None | Unset):
    """

    has_children: bool
    id: str
    kind: WorkspaceFileKind
    name: str
    relative_path: str
    created_at: int | None | Unset = UNSET
    modified_at: int | None | Unset = UNSET
    size_bytes: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        has_children = self.has_children

        id = self.id

        kind = self.kind.value

        name = self.name

        relative_path = self.relative_path

        created_at: int | None | Unset
        if isinstance(self.created_at, Unset):
            created_at = UNSET
        else:
            created_at = self.created_at

        modified_at: int | None | Unset
        if isinstance(self.modified_at, Unset):
            modified_at = UNSET
        else:
            modified_at = self.modified_at

        size_bytes: int | None | Unset
        if isinstance(self.size_bytes, Unset):
            size_bytes = UNSET
        else:
            size_bytes = self.size_bytes

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "hasChildren": has_children,
                "id": id,
                "kind": kind,
                "name": name,
                "relativePath": relative_path,
            }
        )
        if created_at is not UNSET:
            field_dict["createdAt"] = created_at
        if modified_at is not UNSET:
            field_dict["modifiedAt"] = modified_at
        if size_bytes is not UNSET:
            field_dict["sizeBytes"] = size_bytes

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        has_children = d.pop("hasChildren")

        id = d.pop("id")

        kind = WorkspaceFileKind(d.pop("kind"))

        name = d.pop("name")

        relative_path = d.pop("relativePath")

        def _parse_created_at(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        created_at = _parse_created_at(d.pop("createdAt", UNSET))

        def _parse_modified_at(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        modified_at = _parse_modified_at(d.pop("modifiedAt", UNSET))

        def _parse_size_bytes(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        size_bytes = _parse_size_bytes(d.pop("sizeBytes", UNSET))

        workspace_file_entry = cls(
            has_children=has_children,
            id=id,
            kind=kind,
            name=name,
            relative_path=relative_path,
            created_at=created_at,
            modified_at=modified_at,
            size_bytes=size_bytes,
        )

        workspace_file_entry.additional_properties = d
        return workspace_file_entry

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
