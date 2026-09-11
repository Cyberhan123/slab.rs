from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

if TYPE_CHECKING:
    from ..models.rollout_timeline_response_thread import RolloutTimelineResponseThread


T = TypeVar("T", bound="RolloutTimelineResponse")


@_attrs_define
class RolloutTimelineResponse:
    """The rollout timeline projected into the harness `Thread` wire type (same
    projection `thread/resume` restores history with) plus every turn's final
    `TurnState` input messages — the exact prompt the model was sent.

        Attributes:
            thread (RolloutTimelineResponseThread):
            turn_prompts (list[Any]): Per turn (indexed): the persisted `TurnState.input_messages`, or `null`
                when the turn has no state record.
    """

    thread: RolloutTimelineResponseThread
    turn_prompts: list[Any]
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        thread = self.thread.to_dict()

        turn_prompts = self.turn_prompts

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "thread": thread,
                "turn_prompts": turn_prompts,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        from ..models.rollout_timeline_response_thread import (
            RolloutTimelineResponseThread,
        )

        d = dict(src_dict)
        thread = RolloutTimelineResponseThread.from_dict(d.pop("thread"))

        turn_prompts = cast(list[Any], d.pop("turn_prompts"))

        rollout_timeline_response = cls(
            thread=thread,
            turn_prompts=turn_prompts,
        )

        rollout_timeline_response.additional_properties = d
        return rollout_timeline_response

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
