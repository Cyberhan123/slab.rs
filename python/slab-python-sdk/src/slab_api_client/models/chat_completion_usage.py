from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.chat_prompt_tokens_details import ChatPromptTokensDetails


T = TypeVar("T", bound="ChatCompletionUsage")


@_attrs_define
class ChatCompletionUsage:
    """
    Attributes:
        completion_tokens (int):
        prompt_tokens (int):
        prompt_tokens_details (ChatPromptTokensDetails):
        total_tokens (int):
        estimated (bool | Unset):
    """

    completion_tokens: int
    prompt_tokens: int
    prompt_tokens_details: ChatPromptTokensDetails
    total_tokens: int
    estimated: bool | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        completion_tokens = self.completion_tokens

        prompt_tokens = self.prompt_tokens

        prompt_tokens_details = self.prompt_tokens_details.to_dict()

        total_tokens = self.total_tokens

        estimated = self.estimated

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "completion_tokens": completion_tokens,
                "prompt_tokens": prompt_tokens,
                "prompt_tokens_details": prompt_tokens_details,
                "total_tokens": total_tokens,
            }
        )
        if estimated is not UNSET:
            field_dict["estimated"] = estimated

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.chat_prompt_tokens_details import ChatPromptTokensDetails

        d = dict(src_dict)
        completion_tokens = d.pop("completion_tokens")

        prompt_tokens = d.pop("prompt_tokens")

        prompt_tokens_details = ChatPromptTokensDetails.from_dict(
            d.pop("prompt_tokens_details")
        )

        total_tokens = d.pop("total_tokens")

        estimated = d.pop("estimated", UNSET)

        chat_completion_usage = cls(
            completion_tokens=completion_tokens,
            prompt_tokens=prompt_tokens,
            prompt_tokens_details=prompt_tokens_details,
            total_tokens=total_tokens,
            estimated=estimated,
        )

        chat_completion_usage.additional_properties = d
        return chat_completion_usage

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
