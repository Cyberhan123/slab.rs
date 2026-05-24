from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="RuntimePresetsRequest")


@_attrs_define
class RuntimePresetsRequest:
    """Default runtime parameters (request).

    Attributes:
        max_tokens (int | None | Unset): Maximum tokens to generate by default.
        min_p (float | None | Unset): Min-p sampling threshold.
        presence_penalty (float | None | Unset): Presence penalty.
        repetition_penalty (float | None | Unset): Repetition penalty.
        temperature (float | None | Unset): Sampling temperature.
        top_k (int | None | Unset): Top-k sampling limit.
        top_p (float | None | Unset): Top-p nucleus sampling probability.
    """

    max_tokens: int | None | Unset = UNSET
    min_p: float | None | Unset = UNSET
    presence_penalty: float | None | Unset = UNSET
    repetition_penalty: float | None | Unset = UNSET
    temperature: float | None | Unset = UNSET
    top_k: int | None | Unset = UNSET
    top_p: float | None | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        max_tokens: int | None | Unset
        if isinstance(self.max_tokens, Unset):
            max_tokens = UNSET
        else:
            max_tokens = self.max_tokens

        min_p: float | None | Unset
        if isinstance(self.min_p, Unset):
            min_p = UNSET
        else:
            min_p = self.min_p

        presence_penalty: float | None | Unset
        if isinstance(self.presence_penalty, Unset):
            presence_penalty = UNSET
        else:
            presence_penalty = self.presence_penalty

        repetition_penalty: float | None | Unset
        if isinstance(self.repetition_penalty, Unset):
            repetition_penalty = UNSET
        else:
            repetition_penalty = self.repetition_penalty

        temperature: float | None | Unset
        if isinstance(self.temperature, Unset):
            temperature = UNSET
        else:
            temperature = self.temperature

        top_k: int | None | Unset
        if isinstance(self.top_k, Unset):
            top_k = UNSET
        else:
            top_k = self.top_k

        top_p: float | None | Unset
        if isinstance(self.top_p, Unset):
            top_p = UNSET
        else:
            top_p = self.top_p

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if max_tokens is not UNSET:
            field_dict["max_tokens"] = max_tokens
        if min_p is not UNSET:
            field_dict["min_p"] = min_p
        if presence_penalty is not UNSET:
            field_dict["presence_penalty"] = presence_penalty
        if repetition_penalty is not UNSET:
            field_dict["repetition_penalty"] = repetition_penalty
        if temperature is not UNSET:
            field_dict["temperature"] = temperature
        if top_k is not UNSET:
            field_dict["top_k"] = top_k
        if top_p is not UNSET:
            field_dict["top_p"] = top_p

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

        def _parse_max_tokens(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        max_tokens = _parse_max_tokens(d.pop("max_tokens", UNSET))

        def _parse_min_p(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        min_p = _parse_min_p(d.pop("min_p", UNSET))

        def _parse_presence_penalty(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        presence_penalty = _parse_presence_penalty(d.pop("presence_penalty", UNSET))

        def _parse_repetition_penalty(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        repetition_penalty = _parse_repetition_penalty(
            d.pop("repetition_penalty", UNSET)
        )

        def _parse_temperature(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        temperature = _parse_temperature(d.pop("temperature", UNSET))

        def _parse_top_k(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        top_k = _parse_top_k(d.pop("top_k", UNSET))

        def _parse_top_p(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        top_p = _parse_top_p(d.pop("top_p", UNSET))

        runtime_presets_request = cls(
            max_tokens=max_tokens,
            min_p=min_p,
            presence_penalty=presence_penalty,
            repetition_penalty=repetition_penalty,
            temperature=temperature,
            top_k=top_k,
            top_p=top_p,
        )

        return runtime_presets_request
