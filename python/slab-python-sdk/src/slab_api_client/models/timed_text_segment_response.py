from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="TimedTextSegmentResponse")


@_attrs_define
class TimedTextSegmentResponse:
    """
    Attributes:
        end_ms (int | None | Unset):
        start_ms (int | None | Unset):
        text (None | str | Unset):
    """

    end_ms: int | None | Unset = UNSET
    start_ms: int | None | Unset = UNSET
    text: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        end_ms: int | None | Unset
        if isinstance(self.end_ms, Unset):
            end_ms = UNSET
        else:
            end_ms = self.end_ms

        start_ms: int | None | Unset
        if isinstance(self.start_ms, Unset):
            start_ms = UNSET
        else:
            start_ms = self.start_ms

        text: None | str | Unset
        if isinstance(self.text, Unset):
            text = UNSET
        else:
            text = self.text

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if end_ms is not UNSET:
            field_dict["end_ms"] = end_ms
        if start_ms is not UNSET:
            field_dict["start_ms"] = start_ms
        if text is not UNSET:
            field_dict["text"] = text

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

        def _parse_end_ms(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        end_ms = _parse_end_ms(d.pop("end_ms", UNSET))

        def _parse_start_ms(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        start_ms = _parse_start_ms(d.pop("start_ms", UNSET))

        def _parse_text(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        text = _parse_text(d.pop("text", UNSET))

        timed_text_segment_response = cls(
            end_ms=end_ms,
            start_ms=start_ms,
            text=text,
        )

        timed_text_segment_response.additional_properties = d
        return timed_text_segment_response

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
