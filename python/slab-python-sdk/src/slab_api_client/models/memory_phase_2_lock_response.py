from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

from ..types import UNSET, Unset

T = TypeVar("T", bound="MemoryPhase2LockResponse")


@_attrs_define
class MemoryPhase2LockResponse:
    """One per-project phase2 consolidation lock: watermarks plus lease state.

    Attributes:
        job_key (str): Lock key — the sanitized project key.
        status (str): Lock status (idle / running).
        updated_at (str): Last lock update (RFC3339).
        claimed_watermark (None | str | Unset): `MAX(source_updated_at)` snapshot when the current claim was made.
        completed_watermark (None | str | Unset): Watermark of the last completed consolidation for the project.
        lease_owner (None | str | Unset): Current lease owner, while a consolidation holds the lock.
        lease_until (None | str | Unset): Lease expiry (RFC3339), while held.
    """

    job_key: str
    status: str
    updated_at: str
    claimed_watermark: None | str | Unset = UNSET
    completed_watermark: None | str | Unset = UNSET
    lease_owner: None | str | Unset = UNSET
    lease_until: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        job_key = self.job_key

        status = self.status

        updated_at = self.updated_at

        claimed_watermark: None | str | Unset
        if isinstance(self.claimed_watermark, Unset):
            claimed_watermark = UNSET
        else:
            claimed_watermark = self.claimed_watermark

        completed_watermark: None | str | Unset
        if isinstance(self.completed_watermark, Unset):
            completed_watermark = UNSET
        else:
            completed_watermark = self.completed_watermark

        lease_owner: None | str | Unset
        if isinstance(self.lease_owner, Unset):
            lease_owner = UNSET
        else:
            lease_owner = self.lease_owner

        lease_until: None | str | Unset
        if isinstance(self.lease_until, Unset):
            lease_until = UNSET
        else:
            lease_until = self.lease_until

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "job_key": job_key,
                "status": status,
                "updated_at": updated_at,
            }
        )
        if claimed_watermark is not UNSET:
            field_dict["claimed_watermark"] = claimed_watermark
        if completed_watermark is not UNSET:
            field_dict["completed_watermark"] = completed_watermark
        if lease_owner is not UNSET:
            field_dict["lease_owner"] = lease_owner
        if lease_until is not UNSET:
            field_dict["lease_until"] = lease_until

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        d = dict(src_dict)
        job_key = d.pop("job_key")

        status = d.pop("status")

        updated_at = d.pop("updated_at")

        def _parse_claimed_watermark(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        claimed_watermark = _parse_claimed_watermark(d.pop("claimed_watermark", UNSET))

        def _parse_completed_watermark(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        completed_watermark = _parse_completed_watermark(
            d.pop("completed_watermark", UNSET)
        )

        def _parse_lease_owner(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        lease_owner = _parse_lease_owner(d.pop("lease_owner", UNSET))

        def _parse_lease_until(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        lease_until = _parse_lease_until(d.pop("lease_until", UNSET))

        memory_phase_2_lock_response = cls(
            job_key=job_key,
            status=status,
            updated_at=updated_at,
            claimed_watermark=claimed_watermark,
            completed_watermark=completed_watermark,
            lease_owner=lease_owner,
            lease_until=lease_until,
        )

        memory_phase_2_lock_response.additional_properties = d
        return memory_phase_2_lock_response

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
