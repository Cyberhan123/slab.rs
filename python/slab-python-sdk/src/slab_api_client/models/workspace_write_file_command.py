from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="WorkspaceWriteFileCommand")


@_attrs_define
class WorkspaceWriteFileCommand:
    """
    Attributes:
        content (str):
        relative_path (str):
        expected_hash (None | str | Unset):
    """

    content: str
    relative_path: str
    expected_hash: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        content = self.content

        relative_path = self.relative_path

        expected_hash: None | str | Unset
        if isinstance(self.expected_hash, Unset):
            expected_hash = UNSET
        else:
            expected_hash = self.expected_hash

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "content": content,
                "relativePath": relative_path,
            }
        )
        if expected_hash is not UNSET:
            field_dict["expectedHash"] = expected_hash

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        content = d.pop("content")

        relative_path = d.pop("relativePath")

        def _parse_expected_hash(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        expected_hash = _parse_expected_hash(d.pop("expectedHash", UNSET))

        workspace_write_file_command = cls(
            content=content,
            relative_path=relative_path,
            expected_hash=expected_hash,
        )

        workspace_write_file_command.additional_properties = d
        return workspace_write_file_command

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
