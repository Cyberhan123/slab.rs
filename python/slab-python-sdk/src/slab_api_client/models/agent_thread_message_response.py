from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.chat_tool_call import ChatToolCall


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
        tool_call_id (None | str | Unset):
        tool_calls (list[ChatToolCall] | Unset):
    """

    content: str
    created_at: str
    id: str
    role: str
    thread_id: str
    turn_index: int
    tool_call_id: None | str | Unset = UNSET
    tool_calls: list[ChatToolCall] | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        content = self.content

        created_at = self.created_at

        id = self.id

        role = self.role

        thread_id = self.thread_id

        turn_index = self.turn_index

        tool_call_id: None | str | Unset
        if isinstance(self.tool_call_id, Unset):
            tool_call_id = UNSET
        else:
            tool_call_id = self.tool_call_id

        tool_calls: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.tool_calls, Unset):
            tool_calls = []
            for tool_calls_item_data in self.tool_calls:
                tool_calls_item = tool_calls_item_data.to_dict()
                tool_calls.append(tool_calls_item)

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
        if tool_call_id is not UNSET:
            field_dict["tool_call_id"] = tool_call_id
        if tool_calls is not UNSET:
            field_dict["tool_calls"] = tool_calls

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.chat_tool_call import ChatToolCall

        d = dict(src_dict)
        content = d.pop("content")

        created_at = d.pop("created_at")

        id = d.pop("id")

        role = d.pop("role")

        thread_id = d.pop("thread_id")

        turn_index = d.pop("turn_index")

        def _parse_tool_call_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        tool_call_id = _parse_tool_call_id(d.pop("tool_call_id", UNSET))

        _tool_calls = d.pop("tool_calls", UNSET)
        tool_calls: list[ChatToolCall] | Unset = UNSET
        if _tool_calls is not UNSET:
            tool_calls = []
            for tool_calls_item_data in _tool_calls:
                tool_calls_item = ChatToolCall.from_dict(tool_calls_item_data)

                tool_calls.append(tool_calls_item)

        agent_thread_message_response = cls(
            content=content,
            created_at=created_at,
            id=id,
            role=role,
            thread_id=thread_id,
            turn_index=turn_index,
            tool_call_id=tool_call_id,
            tool_calls=tool_calls,
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
