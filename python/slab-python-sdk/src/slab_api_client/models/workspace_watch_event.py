from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.workspace_watch_entry_kind import WorkspaceWatchEntryKind
from ..models.workspace_watch_event_type import WorkspaceWatchEventType

T = TypeVar("T", bound="WorkspaceWatchEvent")


@_attrs_define
class WorkspaceWatchEvent:
    """
    Attributes:
        kind (WorkspaceWatchEntryKind):
        relative_path (str):
        sequence_number (int):
        type_ (WorkspaceWatchEventType):
    """

    kind: WorkspaceWatchEntryKind
    relative_path: str
    sequence_number: int
    type_: WorkspaceWatchEventType
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        kind = self.kind.value

        relative_path = self.relative_path

        sequence_number = self.sequence_number

        type_ = self.type_.value

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "kind": kind,
                "relativePath": relative_path,
                "sequenceNumber": sequence_number,
                "type": type_,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        kind = WorkspaceWatchEntryKind(d.pop("kind"))

        relative_path = d.pop("relativePath")

        sequence_number = d.pop("sequenceNumber")

        type_ = WorkspaceWatchEventType(d.pop("type"))

        workspace_watch_event = cls(
            kind=kind,
            relative_path=relative_path,
            sequence_number=sequence_number,
            type_=type_,
        )

        workspace_watch_event.additional_properties = d
        return workspace_watch_event

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
