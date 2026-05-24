from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="RenderSubtitleResponse")


@_attrs_define
class RenderSubtitleResponse:
    """
    Attributes:
        entry_count (int):
        format_ (str):
        output_path (str):
    """

    entry_count: int
    format_: str
    output_path: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        entry_count = self.entry_count

        format_ = self.format_

        output_path = self.output_path

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "entry_count": entry_count,
                "format": format_,
                "output_path": output_path,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        entry_count = d.pop("entry_count")

        format_ = d.pop("format")

        output_path = d.pop("output_path")

        render_subtitle_response = cls(
            entry_count=entry_count,
            format_=format_,
            output_path=output_path,
        )

        render_subtitle_response.additional_properties = d
        return render_subtitle_response

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
