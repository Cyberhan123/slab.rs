from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="AgentInterruptResponse")


@_attrs_define
class AgentInterruptResponse:
    """Response body for `POST /v1/agents/{id}/interrupt`.

    Attributes:
        interrupted (bool):
        thread_id (str):
    """

    interrupted: bool
    thread_id: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        interrupted = self.interrupted

        thread_id = self.thread_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "interrupted": interrupted,
                "thread_id": thread_id,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        interrupted = d.pop("interrupted")

        thread_id = d.pop("thread_id")

        agent_interrupt_response = cls(
            interrupted=interrupted,
            thread_id=thread_id,
        )

        agent_interrupt_response.additional_properties = d
        return agent_interrupt_response

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
