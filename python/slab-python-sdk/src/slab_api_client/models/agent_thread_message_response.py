from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="AgentThreadMessageResponse")


@_attrs_define
class AgentThreadMessageResponse:
    """Persisted agent thread message.

    Attributes:
        content (str):
        created_at (str):
        id (str):
        role (str):
        thread_id (str):
        turn_index (int):
    """

    content: str
    created_at: str
    id: str
    role: str
    thread_id: str
    turn_index: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        content = self.content

        created_at = self.created_at

        id = self.id

        role = self.role

        thread_id = self.thread_id

        turn_index = self.turn_index

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "content": content,
                "created_at": created_at,
                "id": id,
                "role": role,
                "thread_id": thread_id,
                "turn_index": turn_index,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        content = d.pop("content")

        created_at = d.pop("created_at")

        id = d.pop("id")

        role = d.pop("role")

        thread_id = d.pop("thread_id")

        turn_index = d.pop("turn_index")

        agent_thread_message_response = cls(
            content=content,
            created_at=created_at,
            id=id,
            role=role,
            thread_id=thread_id,
            turn_index=turn_index,
        )

        agent_thread_message_response.additional_properties = d
        return agent_thread_message_response

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
