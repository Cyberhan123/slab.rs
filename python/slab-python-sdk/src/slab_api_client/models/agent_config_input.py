from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.chat_reasoning_effort import ChatReasoningEffort
from ..models.chat_verbosity import ChatVerbosity
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.agent_structured_output_input_type_0 import (
        AgentStructuredOutputInputType0,
    )
    from ..models.agent_structured_output_input_type_1 import (
        AgentStructuredOutputInputType1,
    )
    from ..models.agent_tool_choice_input_type_0 import AgentToolChoiceInputType0
    from ..models.agent_tool_choice_input_type_1 import AgentToolChoiceInputType1
    from ..models.agent_tool_choice_input_type_2 import AgentToolChoiceInputType2
    from ..models.agent_tool_choice_input_type_3 import AgentToolChoiceInputType3


T = TypeVar("T", bound="AgentConfigInput")


@_attrs_define
class AgentConfigInput:
    """Agent configuration provided by the caller.

    Attributes:
        allowed_tools (list[str] | None | Unset):
        invalid_tool_call_retries (int | None | Unset):
        max_tokens (int | None | Unset):
        max_turns (int | None | Unset):
        min_p (float | None | Unset):
        model (None | str | Unset):
        presence_penalty (float | None | Unset):
        reasoning_effort (ChatReasoningEffort | None | Unset):
        repetition_penalty (float | None | Unset):
        structured_output (AgentStructuredOutputInputType0 | AgentStructuredOutputInputType1 | None | Unset):
        system_prompt (None | str | Unset):
        temperature (float | None | Unset):
        tool_choice (AgentToolChoiceInputType0 | AgentToolChoiceInputType1 | AgentToolChoiceInputType2 |
            AgentToolChoiceInputType3 | None | Unset):
        tool_concurrency (int | None | Unset):
        top_k (int | None | Unset):
        top_p (float | None | Unset):
        verbosity (ChatVerbosity | None | Unset):
    """

    allowed_tools: list[str] | None | Unset = UNSET
    invalid_tool_call_retries: int | None | Unset = UNSET
    max_tokens: int | None | Unset = UNSET
    max_turns: int | None | Unset = UNSET
    min_p: float | None | Unset = UNSET
    model: None | str | Unset = UNSET
    presence_penalty: float | None | Unset = UNSET
    reasoning_effort: ChatReasoningEffort | None | Unset = UNSET
    repetition_penalty: float | None | Unset = UNSET
    structured_output: (
        AgentStructuredOutputInputType0 | AgentStructuredOutputInputType1 | None | Unset
    ) = UNSET
    system_prompt: None | str | Unset = UNSET
    temperature: float | None | Unset = UNSET
    tool_choice: (
        AgentToolChoiceInputType0
        | AgentToolChoiceInputType1
        | AgentToolChoiceInputType2
        | AgentToolChoiceInputType3
        | None
        | Unset
    ) = UNSET
    tool_concurrency: int | None | Unset = UNSET
    top_k: int | None | Unset = UNSET
    top_p: float | None | Unset = UNSET
    verbosity: ChatVerbosity | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.agent_structured_output_input_type_0 import (
            AgentStructuredOutputInputType0,
        )
        from ..models.agent_structured_output_input_type_1 import (
            AgentStructuredOutputInputType1,
        )
        from ..models.agent_tool_choice_input_type_0 import AgentToolChoiceInputType0
        from ..models.agent_tool_choice_input_type_1 import AgentToolChoiceInputType1
        from ..models.agent_tool_choice_input_type_2 import AgentToolChoiceInputType2
        from ..models.agent_tool_choice_input_type_3 import AgentToolChoiceInputType3

        allowed_tools: list[str] | None | Unset
        if isinstance(self.allowed_tools, Unset):
            allowed_tools = UNSET
        elif isinstance(self.allowed_tools, list):
            allowed_tools = self.allowed_tools

        else:
            allowed_tools = self.allowed_tools

        invalid_tool_call_retries: int | None | Unset
        if isinstance(self.invalid_tool_call_retries, Unset):
            invalid_tool_call_retries = UNSET
        else:
            invalid_tool_call_retries = self.invalid_tool_call_retries

        max_tokens: int | None | Unset
        if isinstance(self.max_tokens, Unset):
            max_tokens = UNSET
        else:
            max_tokens = self.max_tokens

        max_turns: int | None | Unset
        if isinstance(self.max_turns, Unset):
            max_turns = UNSET
        else:
            max_turns = self.max_turns

        min_p: float | None | Unset
        if isinstance(self.min_p, Unset):
            min_p = UNSET
        else:
            min_p = self.min_p

        model: None | str | Unset
        if isinstance(self.model, Unset):
            model = UNSET
        else:
            model = self.model

        presence_penalty: float | None | Unset
        if isinstance(self.presence_penalty, Unset):
            presence_penalty = UNSET
        else:
            presence_penalty = self.presence_penalty

        reasoning_effort: None | str | Unset
        if isinstance(self.reasoning_effort, Unset):
            reasoning_effort = UNSET
        elif isinstance(self.reasoning_effort, ChatReasoningEffort):
            reasoning_effort = self.reasoning_effort.value
        else:
            reasoning_effort = self.reasoning_effort

        repetition_penalty: float | None | Unset
        if isinstance(self.repetition_penalty, Unset):
            repetition_penalty = UNSET
        else:
            repetition_penalty = self.repetition_penalty

        structured_output: dict[str, Any] | None | Unset
        if isinstance(self.structured_output, Unset):
            structured_output = UNSET
        elif isinstance(self.structured_output, AgentStructuredOutputInputType0):
            structured_output = self.structured_output.to_dict()
        elif isinstance(self.structured_output, AgentStructuredOutputInputType1):
            structured_output = self.structured_output.to_dict()
        else:
            structured_output = self.structured_output

        system_prompt: None | str | Unset
        if isinstance(self.system_prompt, Unset):
            system_prompt = UNSET
        else:
            system_prompt = self.system_prompt

        temperature: float | None | Unset
        if isinstance(self.temperature, Unset):
            temperature = UNSET
        else:
            temperature = self.temperature

        tool_choice: dict[str, Any] | None | Unset
        if isinstance(self.tool_choice, Unset):
            tool_choice = UNSET
        elif isinstance(self.tool_choice, AgentToolChoiceInputType0):
            tool_choice = self.tool_choice.to_dict()
        elif isinstance(self.tool_choice, AgentToolChoiceInputType1):
            tool_choice = self.tool_choice.to_dict()
        elif isinstance(self.tool_choice, AgentToolChoiceInputType2):
            tool_choice = self.tool_choice.to_dict()
        elif isinstance(self.tool_choice, AgentToolChoiceInputType3):
            tool_choice = self.tool_choice.to_dict()
        else:
            tool_choice = self.tool_choice

        tool_concurrency: int | None | Unset
        if isinstance(self.tool_concurrency, Unset):
            tool_concurrency = UNSET
        else:
            tool_concurrency = self.tool_concurrency

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

        verbosity: None | str | Unset
        if isinstance(self.verbosity, Unset):
            verbosity = UNSET
        elif isinstance(self.verbosity, ChatVerbosity):
            verbosity = self.verbosity.value
        else:
            verbosity = self.verbosity

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if allowed_tools is not UNSET:
            field_dict["allowed_tools"] = allowed_tools
        if invalid_tool_call_retries is not UNSET:
            field_dict["invalid_tool_call_retries"] = invalid_tool_call_retries
        if max_tokens is not UNSET:
            field_dict["max_tokens"] = max_tokens
        if max_turns is not UNSET:
            field_dict["max_turns"] = max_turns
        if min_p is not UNSET:
            field_dict["min_p"] = min_p
        if model is not UNSET:
            field_dict["model"] = model
        if presence_penalty is not UNSET:
            field_dict["presence_penalty"] = presence_penalty
        if reasoning_effort is not UNSET:
            field_dict["reasoning_effort"] = reasoning_effort
        if repetition_penalty is not UNSET:
            field_dict["repetition_penalty"] = repetition_penalty
        if structured_output is not UNSET:
            field_dict["structured_output"] = structured_output
        if system_prompt is not UNSET:
            field_dict["system_prompt"] = system_prompt
        if temperature is not UNSET:
            field_dict["temperature"] = temperature
        if tool_choice is not UNSET:
            field_dict["tool_choice"] = tool_choice
        if tool_concurrency is not UNSET:
            field_dict["tool_concurrency"] = tool_concurrency
        if top_k is not UNSET:
            field_dict["top_k"] = top_k
        if top_p is not UNSET:
            field_dict["top_p"] = top_p
        if verbosity is not UNSET:
            field_dict["verbosity"] = verbosity

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.agent_structured_output_input_type_0 import (
            AgentStructuredOutputInputType0,
        )
        from ..models.agent_structured_output_input_type_1 import (
            AgentStructuredOutputInputType1,
        )
        from ..models.agent_tool_choice_input_type_0 import AgentToolChoiceInputType0
        from ..models.agent_tool_choice_input_type_1 import AgentToolChoiceInputType1
        from ..models.agent_tool_choice_input_type_2 import AgentToolChoiceInputType2
        from ..models.agent_tool_choice_input_type_3 import AgentToolChoiceInputType3

        d = dict(src_dict)

        def _parse_allowed_tools(data: object) -> list[str] | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                allowed_tools_type_0 = cast(list[str], data)

                return allowed_tools_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[str] | None | Unset, data)

        allowed_tools = _parse_allowed_tools(d.pop("allowed_tools", UNSET))

        def _parse_invalid_tool_call_retries(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        invalid_tool_call_retries = _parse_invalid_tool_call_retries(
            d.pop("invalid_tool_call_retries", UNSET)
        )

        def _parse_max_tokens(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        max_tokens = _parse_max_tokens(d.pop("max_tokens", UNSET))

        def _parse_max_turns(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        max_turns = _parse_max_turns(d.pop("max_turns", UNSET))

        def _parse_min_p(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        min_p = _parse_min_p(d.pop("min_p", UNSET))

        def _parse_model(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        model = _parse_model(d.pop("model", UNSET))

        def _parse_presence_penalty(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        presence_penalty = _parse_presence_penalty(d.pop("presence_penalty", UNSET))

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

        def _parse_repetition_penalty(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        repetition_penalty = _parse_repetition_penalty(
            d.pop("repetition_penalty", UNSET)
        )

        def _parse_structured_output(
            data: object,
        ) -> (
            AgentStructuredOutputInputType0
            | AgentStructuredOutputInputType1
            | None
            | Unset
        ):
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_agent_structured_output_input_type_0 = (
                    AgentStructuredOutputInputType0.from_dict(data)
                )

                return componentsschemas_agent_structured_output_input_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_agent_structured_output_input_type_1 = (
                    AgentStructuredOutputInputType1.from_dict(data)
                )

                return componentsschemas_agent_structured_output_input_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(
                AgentStructuredOutputInputType0
                | AgentStructuredOutputInputType1
                | None
                | Unset,
                data,
            )

        structured_output = _parse_structured_output(d.pop("structured_output", UNSET))

        def _parse_system_prompt(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        system_prompt = _parse_system_prompt(d.pop("system_prompt", UNSET))

        def _parse_temperature(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        temperature = _parse_temperature(d.pop("temperature", UNSET))

        def _parse_tool_choice(
            data: object,
        ) -> (
            AgentToolChoiceInputType0
            | AgentToolChoiceInputType1
            | AgentToolChoiceInputType2
            | AgentToolChoiceInputType3
            | None
            | Unset
        ):
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_agent_tool_choice_input_type_0 = (
                    AgentToolChoiceInputType0.from_dict(data)
                )

                return componentsschemas_agent_tool_choice_input_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_agent_tool_choice_input_type_1 = (
                    AgentToolChoiceInputType1.from_dict(data)
                )

                return componentsschemas_agent_tool_choice_input_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_agent_tool_choice_input_type_2 = (
                    AgentToolChoiceInputType2.from_dict(data)
                )

                return componentsschemas_agent_tool_choice_input_type_2
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_agent_tool_choice_input_type_3 = (
                    AgentToolChoiceInputType3.from_dict(data)
                )

                return componentsschemas_agent_tool_choice_input_type_3
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(
                AgentToolChoiceInputType0
                | AgentToolChoiceInputType1
                | AgentToolChoiceInputType2
                | AgentToolChoiceInputType3
                | None
                | Unset,
                data,
            )

        tool_choice = _parse_tool_choice(d.pop("tool_choice", UNSET))

        def _parse_tool_concurrency(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        tool_concurrency = _parse_tool_concurrency(d.pop("tool_concurrency", UNSET))

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

        agent_config_input = cls(
            allowed_tools=allowed_tools,
            invalid_tool_call_retries=invalid_tool_call_retries,
            max_tokens=max_tokens,
            max_turns=max_turns,
            min_p=min_p,
            model=model,
            presence_penalty=presence_penalty,
            reasoning_effort=reasoning_effort,
            repetition_penalty=repetition_penalty,
            structured_output=structured_output,
            system_prompt=system_prompt,
            temperature=temperature,
            tool_choice=tool_choice,
            tool_concurrency=tool_concurrency,
            top_k=top_k,
            top_p=top_p,
            verbosity=verbosity,
        )

        agent_config_input.additional_properties = d
        return agent_config_input

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
