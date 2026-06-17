from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="WorkspaceWriteFileView")


@_attrs_define
class WorkspaceWriteFileView:
    """
    Attributes:
        content_hash (str):
        relative_path (str):
        size_bytes (int):
    """

    content_hash: str
    relative_path: str
    size_bytes: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        content_hash = self.content_hash

        relative_path = self.relative_path

        size_bytes = self.size_bytes

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "contentHash": content_hash,
                "relativePath": relative_path,
                "sizeBytes": size_bytes,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        content_hash = d.pop("contentHash")

        relative_path = d.pop("relativePath")

        size_bytes = d.pop("sizeBytes")

        workspace_write_file_view = cls(
            content_hash=content_hash,
            relative_path=relative_path,
            size_bytes=size_bytes,
        )

        workspace_write_file_view.additional_properties = d
        return workspace_write_file_view

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
