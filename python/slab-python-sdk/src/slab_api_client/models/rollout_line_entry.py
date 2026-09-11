from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

if TYPE_CHECKING:
    from ..models.rollout_line_entry_item import RolloutLineEntryItem


T = TypeVar("T", bound="RolloutLineEntry")


@_attrs_define
class RolloutLineEntry:
    """One raw rollout line, item JSON passed through verbatim.

    Attributes:
        index (int): 0-based position of the line in the file.
        item (RolloutLineEntryItem):
        rollout_type (str): The `rolloutType` discriminant (`sessionMeta` / `turnItem` / `eventMsg`
            / `compacted` / `turnContext`).
        timestamp (str):
    """

    index: int
    item: RolloutLineEntryItem
    rollout_type: str
    timestamp: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        index = self.index

        item = self.item.to_dict()

        rollout_type = self.rollout_type

        timestamp = self.timestamp

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "index": index,
                "item": item,
                "rollout_type": rollout_type,
                "timestamp": timestamp,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        from ..models.rollout_line_entry_item import RolloutLineEntryItem

        d = dict(src_dict)
        index = d.pop("index")

        item = RolloutLineEntryItem.from_dict(d.pop("item"))

        rollout_type = d.pop("rollout_type")

        timestamp = d.pop("timestamp")

        rollout_line_entry = cls(
            index=index,
            item=item,
            rollout_type=rollout_type,
            timestamp=timestamp,
        )

        rollout_line_entry.additional_properties = d
        return rollout_line_entry

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
