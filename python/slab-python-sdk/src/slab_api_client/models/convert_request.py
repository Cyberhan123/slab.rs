from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="ConvertRequest")


@_attrs_define
class ConvertRequest:
    """
    Attributes:
        output_format (str): Desired output format (e.g. `"mp3"`, `"wav"`, `"mp4"`).
        source_path (str): Absolute path to the source file.
        output_path (None | str | Unset): Optional output path; defaults to source path with new extension.
    """

    output_format: str
    source_path: str
    output_path: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        output_format = self.output_format

        source_path = self.source_path

        output_path: None | str | Unset
        if isinstance(self.output_path, Unset):
            output_path = UNSET
        else:
            output_path = self.output_path

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "output_format": output_format,
                "source_path": source_path,
            }
        )
        if output_path is not UNSET:
            field_dict["output_path"] = output_path

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        output_format = d.pop("output_format")

        source_path = d.pop("source_path")

        def _parse_output_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        output_path = _parse_output_path(d.pop("output_path", UNSET))

        convert_request = cls(
            output_format=output_format,
            source_path=source_path,
            output_path=output_path,
        )

        convert_request.additional_properties = d
        return convert_request

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
