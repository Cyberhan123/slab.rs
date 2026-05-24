from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.agent_config_input import AgentConfigInput
    from ..models.message_input import MessageInput


T = TypeVar("T", bound="SpawnAgentRequest")


@_attrs_define
class SpawnAgentRequest:
    """Request body for `POST /v1/agents/spawn`.

    Attributes:
        session_id (str): Chat session ID that backs this agent thread.
        config (AgentConfigInput | Unset): Agent configuration provided by the caller.
        messages (list[MessageInput] | Unset): Initial messages to seed the agent's conversation.
    """

    session_id: str
    config: AgentConfigInput | Unset = UNSET
    messages: list[MessageInput] | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        session_id = self.session_id

        config: dict[str, Any] | Unset = UNSET
        if not isinstance(self.config, Unset):
            config = self.config.to_dict()

        messages: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.messages, Unset):
            messages = []
            for messages_item_data in self.messages:
                messages_item = messages_item_data.to_dict()
                messages.append(messages_item)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "session_id": session_id,
            }
        )
        if config is not UNSET:
            field_dict["config"] = config
        if messages is not UNSET:
            field_dict["messages"] = messages

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.agent_config_input import AgentConfigInput
        from ..models.message_input import MessageInput

        d = dict(src_dict)
        session_id = d.pop("session_id")

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

        spawn_agent_request = cls(
            session_id=session_id,
            config=config,
            messages=messages,
        )

        spawn_agent_request.additional_properties = d
        return spawn_agent_request

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
