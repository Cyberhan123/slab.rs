from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

T = TypeVar("T", bound="MemoryPhase1StatusCountResponse")


@_attrs_define
class MemoryPhase1StatusCountResponse:
    """Agent-memory phase1 output count for one (project, status) pair — the
    backlog and failure surface per project memory store.

        Attributes:
            count (int): Row count for this (project, status) pair.
            project_key (str): Sanitized project key (empty string rows predate project sharding).
            status (str): Phase1 output status: pending / running / succeeded /
                succeeded_no_output / failed.
    """

    count: int
    project_key: str
    status: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        count = self.count

        project_key = self.project_key

        status = self.status

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "count": count,
                "project_key": project_key,
                "status": status,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        d = dict(src_dict)
        count = d.pop("count")

        project_key = d.pop("project_key")

        status = d.pop("status")

        memory_phase_1_status_count_response = cls(
            count=count,
            project_key=project_key,
            status=status,
        )

        memory_phase_1_status_count_response.additional_properties = d
        return memory_phase_1_status_count_response

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
