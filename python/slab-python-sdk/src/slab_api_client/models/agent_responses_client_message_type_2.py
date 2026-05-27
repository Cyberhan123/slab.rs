from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.agent_responses_client_message_type_2_type import (
    AgentResponsesClientMessageType2Type,
)
from ..types import UNSET, Unset

T = TypeVar("T", bound="AgentResponsesClientMessageType2")


@_attrs_define
class AgentResponsesClientMessageType2:
    """
    Attributes:
        content (str):
        thread_id (str):
        type_ (AgentResponsesClientMessageType2Type):
        request_id (None | str | Unset):
    """

    content: str
    thread_id: str
    type_: AgentResponsesClientMessageType2Type
    request_id: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        content = self.content

        thread_id = self.thread_id

        type_ = self.type_.value

        request_id: None | str | Unset
        if isinstance(self.request_id, Unset):
            request_id = UNSET
        else:
            request_id = self.request_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "content": content,
                "thread_id": thread_id,
                "type": type_,
            }
        )
        if request_id is not UNSET:
            field_dict["request_id"] = request_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        content = d.pop("content")

        thread_id = d.pop("thread_id")

        type_ = AgentResponsesClientMessageType2Type(d.pop("type"))

        def _parse_request_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        request_id = _parse_request_id(d.pop("request_id", UNSET))

        agent_responses_client_message_type_2 = cls(
            content=content,
            thread_id=thread_id,
            type_=type_,
            request_id=request_id,
        )

        agent_responses_client_message_type_2.additional_properties = d
        return agent_responses_client_message_type_2

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
