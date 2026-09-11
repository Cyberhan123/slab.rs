from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

if TYPE_CHECKING:
    from ..models.rollout_trace_response_manifest import RolloutTraceResponseManifest


T = TypeVar("T", bound="RolloutTraceResponse")


@_attrs_define
class RolloutTraceResponse:
    """
    Attributes:
        conversation (list[Any]): The reducer's reconstruction of the conversation the model actually
            saw (L3 semantic replay). Empty on a reduction failure.
        events (list[Any]): Raw `trace.jsonl` events (paged, JSON passed through verbatim).
        manifest (RolloutTraceResponseManifest):
        total_events (int):
    """

    conversation: list[Any]
    events: list[Any]
    manifest: RolloutTraceResponseManifest
    total_events: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        conversation = self.conversation

        events = self.events

        manifest = self.manifest.to_dict()

        total_events = self.total_events

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "conversation": conversation,
                "events": events,
                "manifest": manifest,
                "total_events": total_events,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        from ..models.rollout_trace_response_manifest import (
            RolloutTraceResponseManifest,
        )

        d = dict(src_dict)
        conversation = cast(list[Any], d.pop("conversation"))

        events = cast(list[Any], d.pop("events"))

        manifest = RolloutTraceResponseManifest.from_dict(d.pop("manifest"))

        total_events = d.pop("total_events")

        rollout_trace_response = cls(
            conversation=conversation,
            events=events,
            manifest=manifest,
            total_events=total_events,
        )

        rollout_trace_response.additional_properties = d
        return rollout_trace_response

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
