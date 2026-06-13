from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="SystemDiagnosticPathResponse")


@_attrs_define
class SystemDiagnosticPathResponse:
    """One local filesystem path included in the diagnostics snapshot.

    Attributes:
        exists (bool):
        label (str):
        path (str):
    """

    exists: bool
    label: str
    path: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        exists = self.exists

        label = self.label

        path = self.path

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "exists": exists,
                "label": label,
                "path": path,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        exists = d.pop("exists")

        label = d.pop("label")

        path = d.pop("path")

        system_diagnostic_path_response = cls(
            exists=exists,
            label=label,
            path=path,
        )

        system_diagnostic_path_response.additional_properties = d
        return system_diagnostic_path_response

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
