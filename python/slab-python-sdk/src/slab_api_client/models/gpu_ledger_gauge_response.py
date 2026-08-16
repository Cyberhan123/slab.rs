from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

T = TypeVar("T", bound="GpuLedgerGaugeResponse")


@_attrs_define
class GpuLedgerGaugeResponse:
    """Probe gauge folded into a ledger row. `free = total − used` (all-smi
    reports no free; the scheduler derives it).

        Attributes:
            free_bytes (int):
            total_bytes (int):
            used_bytes (int):
    """

    free_bytes: int
    total_bytes: int
    used_bytes: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        free_bytes = self.free_bytes

        total_bytes = self.total_bytes

        used_bytes = self.used_bytes

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "free_bytes": free_bytes,
                "total_bytes": total_bytes,
                "used_bytes": used_bytes,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        d = dict(src_dict)
        free_bytes = d.pop("free_bytes")

        total_bytes = d.pop("total_bytes")

        used_bytes = d.pop("used_bytes")

        gpu_ledger_gauge_response = cls(
            free_bytes=free_bytes,
            total_bytes=total_bytes,
            used_bytes=used_bytes,
        )

        gpu_ledger_gauge_response.additional_properties = d
        return gpu_ledger_gauge_response

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
