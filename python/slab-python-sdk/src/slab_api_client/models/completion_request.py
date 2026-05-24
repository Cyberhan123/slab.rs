from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.chat_response_format import ChatResponseFormat


T = TypeVar("T", bound="CompletionRequest")


@_attrs_define
class CompletionRequest:
    """Request body for `POST /v1/completions`.

    Attributes:
        prompt (str): Raw prompt for completion-style generation.
        gbnf (None | str | Unset): Raw GBNF passed through to the local llama backend.
        json_schema (Any | Unset): Legacy llama.cpp-compatible top-level JSON schema field.
        max_tokens (int | None | Unset): Maximum tokens to generate.
        min_p (float | None | Unset): Min-p sampling threshold for local llama backends.
        model (str | Unset): Unified model identifier from `/v1/models`.
            When omitted, the first available chat-compatible model is used.
        n (int | None | Unset): Number of completions to generate.
        presence_penalty (float | None | Unset): Presence penalty for local llama backends.
        repetition_penalty (float | None | Unset): Repetition penalty for local llama backends.
        response_format (ChatResponseFormat | None | Unset):
        stop (list[str] | None | str | Unset):
        stream (bool | Unset): Stream the result using SSE.
        temperature (float | None | Unset): Sampling temperature in [0, 2].
        top_k (int | None | Unset): Top-k sampling limit for local llama backends.
        top_p (float | None | Unset): Nucleus sampling threshold in (0, 1].
    """

    prompt: str
    gbnf: None | str | Unset = UNSET
    json_schema: Any | Unset = UNSET
    max_tokens: int | None | Unset = UNSET
    min_p: float | None | Unset = UNSET
    model: str | Unset = UNSET
    n: int | None | Unset = UNSET
    presence_penalty: float | None | Unset = UNSET
    repetition_penalty: float | None | Unset = UNSET
    response_format: ChatResponseFormat | None | Unset = UNSET
    stop: list[str] | None | str | Unset = UNSET
    stream: bool | Unset = UNSET
    temperature: float | None | Unset = UNSET
    top_k: int | None | Unset = UNSET
    top_p: float | None | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        from ..models.chat_response_format import ChatResponseFormat

        prompt = self.prompt

        gbnf: None | str | Unset
        if isinstance(self.gbnf, Unset):
            gbnf = UNSET
        else:
            gbnf = self.gbnf

        json_schema = self.json_schema

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

        model = self.model

        n: int | None | Unset
        if isinstance(self.n, Unset):
            n = UNSET
        else:
            n = self.n

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

        response_format: dict[str, Any] | None | Unset
        if isinstance(self.response_format, Unset):
            response_format = UNSET
        elif isinstance(self.response_format, ChatResponseFormat):
            response_format = self.response_format.to_dict()
        else:
            response_format = self.response_format

        stop: list[str] | None | str | Unset
        if isinstance(self.stop, Unset):
            stop = UNSET
        elif isinstance(self.stop, list):
            stop = self.stop

        else:
            stop = self.stop

        stream = self.stream

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

        field_dict.update(
            {
                "prompt": prompt,
            }
        )
        if gbnf is not UNSET:
            field_dict["gbnf"] = gbnf
        if json_schema is not UNSET:
            field_dict["json_schema"] = json_schema
        if max_tokens is not UNSET:
            field_dict["max_tokens"] = max_tokens
        if min_p is not UNSET:
            field_dict["min_p"] = min_p
        if model is not UNSET:
            field_dict["model"] = model
        if n is not UNSET:
            field_dict["n"] = n
        if presence_penalty is not UNSET:
            field_dict["presence_penalty"] = presence_penalty
        if repetition_penalty is not UNSET:
            field_dict["repetition_penalty"] = repetition_penalty
        if response_format is not UNSET:
            field_dict["response_format"] = response_format
        if stop is not UNSET:
            field_dict["stop"] = stop
        if stream is not UNSET:
            field_dict["stream"] = stream
        if temperature is not UNSET:
            field_dict["temperature"] = temperature
        if top_k is not UNSET:
            field_dict["top_k"] = top_k
        if top_p is not UNSET:
            field_dict["top_p"] = top_p

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.chat_response_format import ChatResponseFormat

        d = dict(src_dict)
        prompt = d.pop("prompt")

        def _parse_gbnf(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        gbnf = _parse_gbnf(d.pop("gbnf", UNSET))

        json_schema = d.pop("json_schema", UNSET)

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

        model = d.pop("model", UNSET)

        def _parse_n(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        n = _parse_n(d.pop("n", UNSET))

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

        def _parse_response_format(data: object) -> ChatResponseFormat | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                response_format_type_1 = ChatResponseFormat.from_dict(data)

                return response_format_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ChatResponseFormat | None | Unset, data)

        response_format = _parse_response_format(d.pop("response_format", UNSET))

        def _parse_stop(data: object) -> list[str] | None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                componentsschemas_stop_sequences_type_1 = cast(list[str], data)

                return componentsschemas_stop_sequences_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[str] | None | str | Unset, data)

        stop = _parse_stop(d.pop("stop", UNSET))

        stream = d.pop("stream", UNSET)

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

        completion_request = cls(
            prompt=prompt,
            gbnf=gbnf,
            json_schema=json_schema,
            max_tokens=max_tokens,
            min_p=min_p,
            model=model,
            n=n,
            presence_penalty=presence_penalty,
            repetition_penalty=repetition_penalty,
            response_format=response_format,
            stop=stop,
            stream=stream,
            temperature=temperature,
            top_k=top_k,
            top_p=top_p,
        )

        return completion_request
