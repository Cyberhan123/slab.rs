from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.agent_responses_client_message_type_1_type import (
    AgentResponsesClientMessageType1Type,
)
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.agent_config_input import AgentConfigInput
    from ..models.message_input import MessageInput


T = TypeVar("T", bound="AgentResponsesClientMessageType1")


@_attrs_define
class AgentResponsesClientMessageType1:
    """
    Attributes:
        session_id (str):
        type_ (AgentResponsesClientMessageType1Type):
        config (AgentConfigInput | Unset): Agent configuration provided by the caller.
        messages (list[MessageInput] | Unset):
        request_id (None | str | Unset):
    """

    session_id: str
    type_: AgentResponsesClientMessageType1Type
    config: AgentConfigInput | Unset = UNSET
    messages: list[MessageInput] | Unset = UNSET
    request_id: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        session_id = self.session_id

        type_ = self.type_.value

        config: dict[str, Any] | Unset = UNSET
        if not isinstance(self.config, Unset):
            config = self.config.to_dict()

        messages: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.messages, Unset):
            messages = []
            for messages_item_data in self.messages:
                messages_item = messages_item_data.to_dict()
                messages.append(messages_item)

        request_id: None | str | Unset
        if isinstance(self.request_id, Unset):
            request_id = UNSET
        else:
            request_id = self.request_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "session_id": session_id,
                "type": type_,
            }
        )
        if config is not UNSET:
            field_dict["config"] = config
        if messages is not UNSET:
            field_dict["messages"] = messages
        if request_id is not UNSET:
            field_dict["request_id"] = request_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.agent_config_input import AgentConfigInput
        from ..models.message_input import MessageInput

        d = dict(src_dict)
        session_id = d.pop("session_id")

        type_ = AgentResponsesClientMessageType1Type(d.pop("type"))

        _config = d.pop("config", UNSET)
        config: AgentConfigInput | Unset
        if isinstance(_config, Unset):
            config = UNSET
        else:
            config = AgentConfigInput.from_dict(_config)

        _messages = d.pop("messages", UNSET)
        messages: list[MessageInput] | Unset = UNSET
        if _messages is not UNSET:
            messages = []
            for messages_item_data in _messages:
                messages_item = MessageInput.from_dict(messages_item_data)

                messages.append(messages_item)

        def _parse_request_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        request_id = _parse_request_id(d.pop("request_id", UNSET))

        agent_responses_client_message_type_1 = cls(
            session_id=session_id,
            type_=type_,
            config=config,
            messages=messages,
            request_id=request_id,
        )

        agent_responses_client_message_type_1.additional_properties = d
        return agent_responses_client_message_type_1

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
