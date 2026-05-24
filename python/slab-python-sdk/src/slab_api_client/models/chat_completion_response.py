from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.chat_choice import ChatChoice
    from ..models.chat_completion_usage import ChatCompletionUsage


T = TypeVar("T", bound="ChatCompletionResponse")


@_attrs_define
class ChatCompletionResponse:
    """Response body for `POST /v1/chat/completions`.

    Attributes:
        choices (list[ChatChoice]): Generated choices.
        created (int): Unix timestamp of when the response was created.
        id (str): Unique identifier for this completion.
        model (str): Model that produced the completion.
        object_ (str): Always `"chat.completion"`.
        system_fingerprint (str): Backend/system fingerprint for compatibility with OpenAI clients.
        usage (ChatCompletionUsage | None | Unset):
    """

    choices: list[ChatChoice]
    created: int
    id: str
    model: str
    object_: str
    system_fingerprint: str
    usage: ChatCompletionUsage | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.chat_completion_usage import ChatCompletionUsage

        choices = []
        for choices_item_data in self.choices:
            choices_item = choices_item_data.to_dict()
            choices.append(choices_item)

        created = self.created

        id = self.id

        model = self.model

        object_ = self.object_

        system_fingerprint = self.system_fingerprint

        usage: dict[str, Any] | None | Unset
        if isinstance(self.usage, Unset):
            usage = UNSET
        elif isinstance(self.usage, ChatCompletionUsage):
            usage = self.usage.to_dict()
        else:
            usage = self.usage

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "choices": choices,
                "created": created,
                "id": id,
                "model": model,
                "object": object_,
                "system_fingerprint": system_fingerprint,
            }
        )
        if usage is not UNSET:
            field_dict["usage"] = usage

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.chat_choice import ChatChoice
        from ..models.chat_completion_usage import ChatCompletionUsage

        d = dict(src_dict)
        choices = []
        _choices = d.pop("choices")
        for choices_item_data in _choices:
            choices_item = ChatChoice.from_dict(choices_item_data)

            choices.append(choices_item)

        created = d.pop("created")

        id = d.pop("id")

        model = d.pop("model")

        object_ = d.pop("object")

        system_fingerprint = d.pop("system_fingerprint")

        def _parse_usage(data: object) -> ChatCompletionUsage | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                usage_type_1 = ChatCompletionUsage.from_dict(data)

                return usage_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ChatCompletionUsage | None | Unset, data)

        usage = _parse_usage(d.pop("usage", UNSET))

        chat_completion_response = cls(
            choices=choices,
            created=created,
            id=id,
            model=model,
            object_=object_,
            system_fingerprint=system_fingerprint,
            usage=usage,
        )

        chat_completion_response.additional_properties = d
        return chat_completion_response

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
