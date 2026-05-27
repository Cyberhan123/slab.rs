from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.agent_responses_action import AgentResponsesAction
from ..models.agent_responses_server_message_type_0_type import (
    AgentResponsesServerMessageType0Type,
)
from ..models.agent_status_value import AgentStatusValue
from ..types import UNSET, Unset

T = TypeVar("T", bound="AgentResponsesServerMessageType0")


@_attrs_define
class AgentResponsesServerMessageType0:
    """
    Attributes:
        accepted (bool):
        action (AgentResponsesAction): Client action acknowledged by `/v1/agents/responses`.
        type_ (AgentResponsesServerMessageType0Type):
        delivered (bool | None | Unset):
        request_id (None | str | Unset):
        status (AgentStatusValue | None | Unset):
        thread_id (None | str | Unset):
    """

    accepted: bool
    action: AgentResponsesAction
    type_: AgentResponsesServerMessageType0Type
    delivered: bool | None | Unset = UNSET
    request_id: None | str | Unset = UNSET
    status: AgentStatusValue | None | Unset = UNSET
    thread_id: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        accepted = self.accepted

        action = self.action.value

        type_ = self.type_.value

        delivered: bool | None | Unset
        if isinstance(self.delivered, Unset):
            delivered = UNSET
        else:
            delivered = self.delivered

        request_id: None | str | Unset
        if isinstance(self.request_id, Unset):
            request_id = UNSET
        else:
            request_id = self.request_id

        status: None | str | Unset
        if isinstance(self.status, Unset):
            status = UNSET
        elif isinstance(self.status, AgentStatusValue):
            status = self.status.value
        else:
            status = self.status

        thread_id: None | str | Unset
        if isinstance(self.thread_id, Unset):
            thread_id = UNSET
        else:
            thread_id = self.thread_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "accepted": accepted,
                "action": action,
                "type": type_,
            }
        )
        if delivered is not UNSET:
            field_dict["delivered"] = delivered
        if request_id is not UNSET:
            field_dict["request_id"] = request_id
        if status is not UNSET:
            field_dict["status"] = status
        if thread_id is not UNSET:
            field_dict["thread_id"] = thread_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        accepted = d.pop("accepted")

        action = AgentResponsesAction(d.pop("action"))

        type_ = AgentResponsesServerMessageType0Type(d.pop("type"))

        def _parse_delivered(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        delivered = _parse_delivered(d.pop("delivered", UNSET))

        def _parse_request_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        request_id = _parse_request_id(d.pop("request_id", UNSET))

        def _parse_status(data: object) -> AgentStatusValue | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                status_type_1 = AgentStatusValue(data)

                return status_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(AgentStatusValue | None | Unset, data)

        status = _parse_status(d.pop("status", UNSET))

        def _parse_thread_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        thread_id = _parse_thread_id(d.pop("thread_id", UNSET))

        agent_responses_server_message_type_0 = cls(
            accepted=accepted,
            action=action,
            type_=type_,
            delivered=delivered,
            request_id=request_id,
            status=status,
            thread_id=thread_id,
        )

        agent_responses_server_message_type_0.additional_properties = d
        return agent_responses_server_message_type_0

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
