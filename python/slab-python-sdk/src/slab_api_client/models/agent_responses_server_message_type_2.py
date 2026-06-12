from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.agent_responses_server_message_type_2_type import (
    AgentResponsesServerMessageType2Type,
)
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.i18n_payload import I18NPayload


T = TypeVar("T", bound="AgentResponsesServerMessageType2")


@_attrs_define
class AgentResponsesServerMessageType2:
    """
    Attributes:
        code (str):
        message (str):
        type_ (AgentResponsesServerMessageType2Type):
        i18n (I18NPayload | None | Unset):
        request_id (None | str | Unset):
        thread_id (None | str | Unset):
    """

    code: str
    message: str
    type_: AgentResponsesServerMessageType2Type
    i18n: I18NPayload | None | Unset = UNSET
    request_id: None | str | Unset = UNSET
    thread_id: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.i18n_payload import I18NPayload

        code = self.code

        message = self.message

        type_ = self.type_.value

        i18n: dict[str, Any] | None | Unset
        if isinstance(self.i18n, Unset):
            i18n = UNSET
        elif isinstance(self.i18n, I18NPayload):
            i18n = self.i18n.to_dict()
        else:
            i18n = self.i18n

        request_id: None | str | Unset
        if isinstance(self.request_id, Unset):
            request_id = UNSET
        else:
            request_id = self.request_id

        thread_id: None | str | Unset
        if isinstance(self.thread_id, Unset):
            thread_id = UNSET
        else:
            thread_id = self.thread_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "code": code,
                "message": message,
                "type": type_,
            }
        )
        if i18n is not UNSET:
            field_dict["i18n"] = i18n
        if request_id is not UNSET:
            field_dict["request_id"] = request_id
        if thread_id is not UNSET:
            field_dict["thread_id"] = thread_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.i18n_payload import I18NPayload

        d = dict(src_dict)
        code = d.pop("code")

        message = d.pop("message")

        type_ = AgentResponsesServerMessageType2Type(d.pop("type"))

        def _parse_i18n(data: object) -> I18NPayload | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                i18n_type_1 = I18NPayload.from_dict(data)

                return i18n_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(I18NPayload | None | Unset, data)

        i18n = _parse_i18n(d.pop("i18n", UNSET))

        def _parse_request_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        request_id = _parse_request_id(d.pop("request_id", UNSET))

        def _parse_thread_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        thread_id = _parse_thread_id(d.pop("thread_id", UNSET))

        agent_responses_server_message_type_2 = cls(
            code=code,
            message=message,
            type_=type_,
            i18n=i18n,
            request_id=request_id,
            thread_id=thread_id,
        )

        agent_responses_server_message_type_2.additional_properties = d
        return agent_responses_server_message_type_2

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
