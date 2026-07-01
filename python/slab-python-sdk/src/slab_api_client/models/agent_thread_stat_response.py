from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="AgentThreadStatResponse")


@_attrs_define
class AgentThreadStatResponse:
    """One agent thread summary in the diagnostics snapshot (INFRA-08). No message
    content, config, or secret data is representable — the source whitelist type
    (`slab_utils::diagnostics::ThreadStat`) forbids them by construction.

        Attributes:
            depth (int):
            status (str):
            thread_id (str):
            turn_index (int):
            reason (None | str | Unset):
    """

    depth: int
    status: str
    thread_id: str
    turn_index: int
    reason: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        depth = self.depth

        status = self.status

        thread_id = self.thread_id

        turn_index = self.turn_index

        reason: None | str | Unset
        if isinstance(self.reason, Unset):
            reason = UNSET
        else:
            reason = self.reason

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "depth": depth,
                "status": status,
                "thread_id": thread_id,
                "turn_index": turn_index,
            }
        )
        if reason is not UNSET:
            field_dict["reason"] = reason

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        depth = d.pop("depth")

        status = d.pop("status")

        thread_id = d.pop("thread_id")

        turn_index = d.pop("turn_index")

        def _parse_reason(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        reason = _parse_reason(d.pop("reason", UNSET))

        agent_thread_stat_response = cls(
            depth=depth,
            status=status,
            thread_id=thread_id,
            turn_index=turn_index,
            reason=reason,
        )

        agent_thread_stat_response.additional_properties = d
        return agent_thread_stat_response

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
