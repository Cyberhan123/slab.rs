from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.chat_reasoning_effort import ChatReasoningEffort
from ..models.chat_thinking_type import ChatThinkingType
from ..models.chat_verbosity import ChatVerbosity
from ..types import UNSET, Unset

T = TypeVar("T", bound="ChatThinkingConfig")


@_attrs_define
class ChatThinkingConfig:
    """Thinking settings accepted by `POST /v1/chat/completions`.

    Attributes:
        type_ (ChatThinkingType): High-level thinking toggle used by chat clients.
        reasoning_effort (ChatReasoningEffort | None | Unset):
        verbosity (ChatVerbosity | None | Unset):
    """

    type_: ChatThinkingType
    reasoning_effort: ChatReasoningEffort | None | Unset = UNSET
    verbosity: ChatVerbosity | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        type_ = self.type_.value

        reasoning_effort: None | str | Unset
        if isinstance(self.reasoning_effort, Unset):
            reasoning_effort = UNSET
        elif isinstance(self.reasoning_effort, ChatReasoningEffort):
            reasoning_effort = self.reasoning_effort.value
        else:
            reasoning_effort = self.reasoning_effort

        verbosity: None | str | Unset
        if isinstance(self.verbosity, Unset):
            verbosity = UNSET
        elif isinstance(self.verbosity, ChatVerbosity):
            verbosity = self.verbosity.value
        else:
            verbosity = self.verbosity

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "type": type_,
            }
        )
        if reasoning_effort is not UNSET:
            field_dict["reasoning_effort"] = reasoning_effort
        if verbosity is not UNSET:
            field_dict["verbosity"] = verbosity

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        type_ = ChatThinkingType(d.pop("type"))

        def _parse_reasoning_effort(data: object) -> ChatReasoningEffort | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                reasoning_effort_type_1 = ChatReasoningEffort(data)

                return reasoning_effort_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ChatReasoningEffort | None | Unset, data)

        reasoning_effort = _parse_reasoning_effort(d.pop("reasoning_effort", UNSET))

        def _parse_verbosity(data: object) -> ChatVerbosity | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                verbosity_type_1 = ChatVerbosity(data)

                return verbosity_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ChatVerbosity | None | Unset, data)

        verbosity = _parse_verbosity(d.pop("verbosity", UNSET))

        chat_thinking_config = cls(
            type_=type_,
            reasoning_effort=reasoning_effort,
            verbosity=verbosity,
        )

        chat_thinking_config.additional_properties = d
        return chat_thinking_config

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
