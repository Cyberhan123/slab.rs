from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.open_ai_reasoning_input import OpenAIReasoningInput
    from ..models.open_ai_text_input import OpenAITextInput


T = TypeVar("T", bound="OpenAICreateRequest")


@_attrs_define
class OpenAICreateRequest:
    """`POST /v1/agents/responses` body as sent by the official `openai` SDK
    (`ResponseCreateParamsBase`). Slab translates `input` + a subset of config;
    unknown fields are ignored (no `deny_unknown_fields`) so future SDK fields
    don't break the server. `input` is held as a `serde_json::Value` (a string
    or an array of input items) so the type is `ToSchema`-derivable.

        Attributes:
            input_ (Any | Unset):
            instructions (None | str | Unset):
            max_output_tokens (int | None | Unset):
            model (None | str | Unset):
            previous_response_id (None | str | Unset):
            reasoning (None | OpenAIReasoningInput | Unset):
            stream (bool | None | Unset):
            temperature (float | None | Unset):
            text (None | OpenAITextInput | Unset):
            tool_choice (Any | Unset):
            tools (Any | Unset): OpenAI Responses `tools` array (function tool definitions). Held as a
                `serde_json::Value` — like `input`/`tool_choice` — so the struct stays
                `ToSchema`-derivable while accepting the canonical Responses shape
                (`[{"type":"function","name":...,"parameters":...}]`). Use
                [`OpenAICreateRequest::function_tools`] to extract the function tools.
            top_p (float | None | Unset):
    """

    input_: Any | Unset = UNSET
    instructions: None | str | Unset = UNSET
    max_output_tokens: int | None | Unset = UNSET
    model: None | str | Unset = UNSET
    previous_response_id: None | str | Unset = UNSET
    reasoning: None | OpenAIReasoningInput | Unset = UNSET
    stream: bool | None | Unset = UNSET
    temperature: float | None | Unset = UNSET
    text: None | OpenAITextInput | Unset = UNSET
    tool_choice: Any | Unset = UNSET
    tools: Any | Unset = UNSET
    top_p: float | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.open_ai_reasoning_input import OpenAIReasoningInput
        from ..models.open_ai_text_input import OpenAITextInput

        input_ = self.input_

        instructions: None | str | Unset
        if isinstance(self.instructions, Unset):
            instructions = UNSET
        else:
            instructions = self.instructions

        max_output_tokens: int | None | Unset
        if isinstance(self.max_output_tokens, Unset):
            max_output_tokens = UNSET
        else:
            max_output_tokens = self.max_output_tokens

        model: None | str | Unset
        if isinstance(self.model, Unset):
            model = UNSET
        else:
            model = self.model

        previous_response_id: None | str | Unset
        if isinstance(self.previous_response_id, Unset):
            previous_response_id = UNSET
        else:
            previous_response_id = self.previous_response_id

        reasoning: dict[str, Any] | None | Unset
        if isinstance(self.reasoning, Unset):
            reasoning = UNSET
        elif isinstance(self.reasoning, OpenAIReasoningInput):
            reasoning = self.reasoning.to_dict()
        else:
            reasoning = self.reasoning

        stream: bool | None | Unset
        if isinstance(self.stream, Unset):
            stream = UNSET
        else:
            stream = self.stream

        temperature: float | None | Unset
        if isinstance(self.temperature, Unset):
            temperature = UNSET
        else:
            temperature = self.temperature

        text: dict[str, Any] | None | Unset
        if isinstance(self.text, Unset):
            text = UNSET
        elif isinstance(self.text, OpenAITextInput):
            text = self.text.to_dict()
        else:
            text = self.text

        tool_choice = self.tool_choice

        tools = self.tools

        top_p: float | None | Unset
        if isinstance(self.top_p, Unset):
            top_p = UNSET
        else:
            top_p = self.top_p

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if input_ is not UNSET:
            field_dict["input"] = input_
        if instructions is not UNSET:
            field_dict["instructions"] = instructions
        if max_output_tokens is not UNSET:
            field_dict["max_output_tokens"] = max_output_tokens
        if model is not UNSET:
            field_dict["model"] = model
        if previous_response_id is not UNSET:
            field_dict["previous_response_id"] = previous_response_id
        if reasoning is not UNSET:
            field_dict["reasoning"] = reasoning
        if stream is not UNSET:
            field_dict["stream"] = stream
        if temperature is not UNSET:
            field_dict["temperature"] = temperature
        if text is not UNSET:
            field_dict["text"] = text
        if tool_choice is not UNSET:
            field_dict["tool_choice"] = tool_choice
        if tools is not UNSET:
            field_dict["tools"] = tools
        if top_p is not UNSET:
            field_dict["top_p"] = top_p

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        from ..models.open_ai_reasoning_input import OpenAIReasoningInput
        from ..models.open_ai_text_input import OpenAITextInput

        d = dict(src_dict)
        input_ = d.pop("input", UNSET)

        def _parse_instructions(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        instructions = _parse_instructions(d.pop("instructions", UNSET))

        def _parse_max_output_tokens(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        max_output_tokens = _parse_max_output_tokens(d.pop("max_output_tokens", UNSET))

        def _parse_model(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        model = _parse_model(d.pop("model", UNSET))

        def _parse_previous_response_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        previous_response_id = _parse_previous_response_id(
            d.pop("previous_response_id", UNSET)
        )

        def _parse_reasoning(data: object) -> None | OpenAIReasoningInput | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                reasoning_type_1 = OpenAIReasoningInput.from_dict(data)

                return reasoning_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | OpenAIReasoningInput | Unset, data)

        reasoning = _parse_reasoning(d.pop("reasoning", UNSET))

        def _parse_stream(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        stream = _parse_stream(d.pop("stream", UNSET))

        def _parse_temperature(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        temperature = _parse_temperature(d.pop("temperature", UNSET))

        def _parse_text(data: object) -> None | OpenAITextInput | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                text_type_1 = OpenAITextInput.from_dict(data)

                return text_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | OpenAITextInput | Unset, data)

        text = _parse_text(d.pop("text", UNSET))

        tool_choice = d.pop("tool_choice", UNSET)

        tools = d.pop("tools", UNSET)

        def _parse_top_p(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        top_p = _parse_top_p(d.pop("top_p", UNSET))

        open_ai_create_request = cls(
            input_=input_,
            instructions=instructions,
            max_output_tokens=max_output_tokens,
            model=model,
            previous_response_id=previous_response_id,
            reasoning=reasoning,
            stream=stream,
            temperature=temperature,
            text=text,
            tool_choice=tool_choice,
            tools=tools,
            top_p=top_p,
        )

        open_ai_create_request.additional_properties = d
        return open_ai_create_request

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
