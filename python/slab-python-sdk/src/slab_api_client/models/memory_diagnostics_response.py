from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

if TYPE_CHECKING:
    from ..models.memory_phase_1_status_count_response import (
        MemoryPhase1StatusCountResponse,
    )
    from ..models.memory_phase_2_lock_response import MemoryPhase2LockResponse
    from ..models.memory_phase_2_run_response import MemoryPhase2RunResponse


T = TypeVar("T", bound="MemoryDiagnosticsResponse")


@_attrs_define
class MemoryDiagnosticsResponse:
    """Agent memory pipeline diagnostics exposed at
    `/v1/system/diagnostics/memories` (read-only; metadata only, no memory
    content).

        Attributes:
            phase1 (list[MemoryPhase1StatusCountResponse]): Phase1 output counts grouped by project and status.
            phase2_locks (list[MemoryPhase2LockResponse]): Per-project phase2 locks (watermarks + lease state).
            recent_runs (list[MemoryPhase2RunResponse]): Most recent phase2 consolidation runs, newest first.
    """

    phase1: list[MemoryPhase1StatusCountResponse]
    phase2_locks: list[MemoryPhase2LockResponse]
    recent_runs: list[MemoryPhase2RunResponse]
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        phase1 = []
        for phase1_item_data in self.phase1:
            phase1_item = phase1_item_data.to_dict()
            phase1.append(phase1_item)

        phase2_locks = []
        for phase2_locks_item_data in self.phase2_locks:
            phase2_locks_item = phase2_locks_item_data.to_dict()
            phase2_locks.append(phase2_locks_item)

        recent_runs = []
        for recent_runs_item_data in self.recent_runs:
            recent_runs_item = recent_runs_item_data.to_dict()
            recent_runs.append(recent_runs_item)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "phase1": phase1,
                "phase2_locks": phase2_locks,
                "recent_runs": recent_runs,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        from ..models.memory_phase_1_status_count_response import (
            MemoryPhase1StatusCountResponse,
        )
        from ..models.memory_phase_2_lock_response import MemoryPhase2LockResponse
        from ..models.memory_phase_2_run_response import MemoryPhase2RunResponse

        d = dict(src_dict)
        phase1 = []
        _phase1 = d.pop("phase1")
        for phase1_item_data in _phase1:
            phase1_item = MemoryPhase1StatusCountResponse.from_dict(phase1_item_data)

            phase1.append(phase1_item)

        phase2_locks = []
        _phase2_locks = d.pop("phase2_locks")
        for phase2_locks_item_data in _phase2_locks:
            phase2_locks_item = MemoryPhase2LockResponse.from_dict(
                phase2_locks_item_data
            )

            phase2_locks.append(phase2_locks_item)

        recent_runs = []
        _recent_runs = d.pop("recent_runs")
        for recent_runs_item_data in _recent_runs:
            recent_runs_item = MemoryPhase2RunResponse.from_dict(recent_runs_item_data)

            recent_runs.append(recent_runs_item)

        memory_diagnostics_response = cls(
            phase1=phase1,
            phase2_locks=phase2_locks,
            recent_runs=recent_runs,
        )

        memory_diagnostics_response.additional_properties = d
        return memory_diagnostics_response

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
