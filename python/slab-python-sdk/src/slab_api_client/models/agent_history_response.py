from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.agent_thread_message_response import AgentThreadMessageResponse
    from ..models.agent_thread_response import AgentThreadResponse


T = TypeVar("T", bound="AgentHistoryResponse")


@_attrs_define
class AgentHistoryResponse:
    """Agent history restored through the sessions API.

    Attributes:
        messages (list[AgentThreadMessageResponse]):
        session_id (str):
        responses (list[Any] | Unset):
        thread (AgentThreadResponse | None | Unset):
    """

    messages: list[AgentThreadMessageResponse]
    session_id: str
    responses: list[Any] | Unset = UNSET
    thread: AgentThreadResponse | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.agent_thread_response import AgentThreadResponse

        messages = []
        for messages_item_data in self.messages:
            messages_item = messages_item_data.to_dict()
            messages.append(messages_item)

        session_id = self.session_id

        responses: list[Any] | Unset = UNSET
        if not isinstance(self.responses, Unset):
            responses = self.responses

        thread: dict[str, Any] | None | Unset
        if isinstance(self.thread, Unset):
            thread = UNSET
        elif isinstance(self.thread, AgentThreadResponse):
            thread = self.thread.to_dict()
        else:
            thread = self.thread

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "messages": messages,
                "session_id": session_id,
            }
        )
        if responses is not UNSET:
            field_dict["responses"] = responses
        if thread is not UNSET:
            field_dict["thread"] = thread

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        from ..models.agent_thread_message_response import AgentThreadMessageResponse
        from ..models.agent_thread_response import AgentThreadResponse

        d = dict(src_dict)
        messages = []
        _messages = d.pop("messages")
        for messages_item_data in _messages:
            messages_item = AgentThreadMessageResponse.from_dict(messages_item_data)

            messages.append(messages_item)

        session_id = d.pop("session_id")

        responses = cast(list[Any], d.pop("responses", UNSET))

        def _parse_thread(data: object) -> AgentThreadResponse | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                thread_type_1 = AgentThreadResponse.from_dict(data)

                return thread_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(AgentThreadResponse | None | Unset, data)

        thread = _parse_thread(d.pop("thread", UNSET))

        agent_history_response = cls(
            messages=messages,
            session_id=session_id,
            responses=responses,
            thread=thread,
        )

        agent_history_response.additional_properties = d
        return agent_history_response

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
