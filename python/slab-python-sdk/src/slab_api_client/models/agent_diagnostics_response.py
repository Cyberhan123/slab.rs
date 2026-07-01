from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.agent_thread_stat_response import AgentThreadStatResponse
    from ..models.failed_tool_call_response import FailedToolCallResponse


T = TypeVar("T", bound="AgentDiagnosticsResponse")


@_attrs_define
class AgentDiagnosticsResponse:
    """Aggregated agent diagnostics: recent thread stats + recent failed tool calls.

    Attributes:
        failed_tool_calls (list[FailedToolCallResponse]):
        threads (list[AgentThreadStatResponse]):
    """

    failed_tool_calls: list[FailedToolCallResponse]
    threads: list[AgentThreadStatResponse]
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        failed_tool_calls = []
        for failed_tool_calls_item_data in self.failed_tool_calls:
            failed_tool_calls_item = failed_tool_calls_item_data.to_dict()
            failed_tool_calls.append(failed_tool_calls_item)

        threads = []
        for threads_item_data in self.threads:
            threads_item = threads_item_data.to_dict()
            threads.append(threads_item)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "failed_tool_calls": failed_tool_calls,
                "threads": threads,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.agent_thread_stat_response import AgentThreadStatResponse
        from ..models.failed_tool_call_response import FailedToolCallResponse

        d = dict(src_dict)
        failed_tool_calls = []
        _failed_tool_calls = d.pop("failed_tool_calls")
        for failed_tool_calls_item_data in _failed_tool_calls:
            failed_tool_calls_item = FailedToolCallResponse.from_dict(
                failed_tool_calls_item_data
            )

            failed_tool_calls.append(failed_tool_calls_item)

        threads = []
        _threads = d.pop("threads")
        for threads_item_data in _threads:
            threads_item = AgentThreadStatResponse.from_dict(threads_item_data)

            threads.append(threads_item)

        agent_diagnostics_response = cls(
            failed_tool_calls=failed_tool_calls,
            threads=threads,
        )

        agent_diagnostics_response.additional_properties = d
        return agent_diagnostics_response

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
