from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="SettingValidationErrorData")


@_attrs_define
class SettingValidationErrorData:
    """
    Attributes:
        message (str):
        path (str):
        pmid (str):
        type_ (str):
    """

    message: str
    path: str
    pmid: str
    type_: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        message = self.message

        path = self.path

        pmid = self.pmid

        type_ = self.type_

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "message": message,
                "path": path,
                "pmid": pmid,
                "type": type_,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        message = d.pop("message")

        path = d.pop("path")

        pmid = d.pop("pmid")

        type_ = d.pop("type")

        setting_validation_error_data = cls(
            message=message,
            path=path,
            pmid=pmid,
            type_=type_,
        )

        setting_validation_error_data.additional_properties = d
        return setting_validation_error_data

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
