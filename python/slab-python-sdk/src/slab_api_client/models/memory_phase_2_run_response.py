from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

from ..types import UNSET, Unset

T = TypeVar("T", bound="MemoryPhase2RunResponse")


@_attrs_define
class MemoryPhase2RunResponse:
    """One recent phase2 consolidation run.

    Attributes:
        id (str): Run id.
        project_key (str): Sanitized project key the run consolidated.
        started_at (str): Run start (RFC3339).
        status (str): Run status (running / succeeded / failed).
        claimed_watermark (None | str | Unset): Watermark claimed at run start.
        completed_at (None | str | Unset): Run completion (RFC3339), once terminal.
        completed_watermark (None | str | Unset): Watermark completed by the run (absent while running or on failure).
        error (None | str | Unset): Failure reason (truncated), for failed runs.
    """

    id: str
    project_key: str
    started_at: str
    status: str
    claimed_watermark: None | str | Unset = UNSET
    completed_at: None | str | Unset = UNSET
    completed_watermark: None | str | Unset = UNSET
    error: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        id = self.id

        project_key = self.project_key

        started_at = self.started_at

        status = self.status

        claimed_watermark: None | str | Unset
        if isinstance(self.claimed_watermark, Unset):
            claimed_watermark = UNSET
        else:
            claimed_watermark = self.claimed_watermark

        completed_at: None | str | Unset
        if isinstance(self.completed_at, Unset):
            completed_at = UNSET
        else:
            completed_at = self.completed_at

        completed_watermark: None | str | Unset
        if isinstance(self.completed_watermark, Unset):
            completed_watermark = UNSET
        else:
            completed_watermark = self.completed_watermark

        error: None | str | Unset
        if isinstance(self.error, Unset):
            error = UNSET
        else:
            error = self.error

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "id": id,
                "project_key": project_key,
                "started_at": started_at,
                "status": status,
            }
        )
        if claimed_watermark is not UNSET:
            field_dict["claimed_watermark"] = claimed_watermark
        if completed_at is not UNSET:
            field_dict["completed_at"] = completed_at
        if completed_watermark is not UNSET:
            field_dict["completed_watermark"] = completed_watermark
        if error is not UNSET:
            field_dict["error"] = error

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        d = dict(src_dict)
        id = d.pop("id")

        project_key = d.pop("project_key")

        started_at = d.pop("started_at")

        status = d.pop("status")

        def _parse_claimed_watermark(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        claimed_watermark = _parse_claimed_watermark(d.pop("claimed_watermark", UNSET))

        def _parse_completed_at(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        completed_at = _parse_completed_at(d.pop("completed_at", UNSET))

        def _parse_completed_watermark(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        completed_watermark = _parse_completed_watermark(
            d.pop("completed_watermark", UNSET)
        )

        def _parse_error(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        error = _parse_error(d.pop("error", UNSET))

        memory_phase_2_run_response = cls(
            id=id,
            project_key=project_key,
            started_at=started_at,
            status=status,
            claimed_watermark=claimed_watermark,
            completed_at=completed_at,
            completed_watermark=completed_watermark,
            error=error,
        )

        memory_phase_2_run_response.additional_properties = d
        return memory_phase_2_run_response

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
