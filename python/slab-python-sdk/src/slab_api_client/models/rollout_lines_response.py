from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

if TYPE_CHECKING:
    from ..models.rollout_line_entry import RolloutLineEntry


T = TypeVar("T", bound="RolloutLinesResponse")


@_attrs_define
class RolloutLinesResponse:
    """
    Attributes:
        limit (int):
        lines (list[RolloutLineEntry]):
        offset (int):
        total (int):
        truncated (bool): Whether more lines exist past the returned window.
    """

    limit: int
    lines: list[RolloutLineEntry]
    offset: int
    total: int
    truncated: bool
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        limit = self.limit

        lines = []
        for lines_item_data in self.lines:
            lines_item = lines_item_data.to_dict()
            lines.append(lines_item)

        offset = self.offset

        total = self.total

        truncated = self.truncated

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "limit": limit,
                "lines": lines,
                "offset": offset,
                "total": total,
                "truncated": truncated,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        from ..models.rollout_line_entry import RolloutLineEntry

        d = dict(src_dict)
        limit = d.pop("limit")

        lines = []
        _lines = d.pop("lines")
        for lines_item_data in _lines:
            lines_item = RolloutLineEntry.from_dict(lines_item_data)

            lines.append(lines_item)

        offset = d.pop("offset")

        total = d.pop("total")

        truncated = d.pop("truncated")

        rollout_lines_response = cls(
            limit=limit,
            lines=lines,
            offset=offset,
            total=total,
            truncated=truncated,
        )

        rollout_lines_response.additional_properties = d
        return rollout_lines_response

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
