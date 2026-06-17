from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="RecentWorkspaceResponse")


@_attrs_define
class RecentWorkspaceResponse:
    """
    Attributes:
        last_opened_at (int):
        name (str):
        root_path (str):
    """

    last_opened_at: int
    name: str
    root_path: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        last_opened_at = self.last_opened_at

        name = self.name

        root_path = self.root_path

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "lastOpenedAt": last_opened_at,
                "name": name,
                "rootPath": root_path,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        last_opened_at = d.pop("lastOpenedAt")

        name = d.pop("name")

        root_path = d.pop("rootPath")

        recent_workspace_response = cls(
            last_opened_at=last_opened_at,
            name=name,
            root_path=root_path,
        )

        recent_workspace_response.additional_properties = d
        return recent_workspace_response

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
