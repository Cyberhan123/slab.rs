from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.gpu_ledger_entry_response import GpuLedgerEntryResponse
    from ..models.gpu_ledger_gauge_response import GpuLedgerGaugeResponse


T = TypeVar("T", bound="GpuLedgerDeviceResponse")


@_attrs_define
class GpuLedgerDeviceResponse:
    """Per-device ledger: last-synced gauge + resident model entries.

    Attributes:
        resident (list[GpuLedgerEntryResponse]):
        uuid (str):
        gauge (GpuLedgerGaugeResponse | None | Unset):
    """

    resident: list[GpuLedgerEntryResponse]
    uuid: str
    gauge: GpuLedgerGaugeResponse | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.gpu_ledger_gauge_response import GpuLedgerGaugeResponse

        resident = []
        for resident_item_data in self.resident:
            resident_item = resident_item_data.to_dict()
            resident.append(resident_item)

        uuid = self.uuid

        gauge: dict[str, Any] | None | Unset
        if isinstance(self.gauge, Unset):
            gauge = UNSET
        elif isinstance(self.gauge, GpuLedgerGaugeResponse):
            gauge = self.gauge.to_dict()
        else:
            gauge = self.gauge

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "resident": resident,
                "uuid": uuid,
            }
        )
        if gauge is not UNSET:
            field_dict["gauge"] = gauge

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        from ..models.gpu_ledger_entry_response import GpuLedgerEntryResponse
        from ..models.gpu_ledger_gauge_response import GpuLedgerGaugeResponse

        d = dict(src_dict)
        resident = []
        _resident = d.pop("resident")
        for resident_item_data in _resident:
            resident_item = GpuLedgerEntryResponse.from_dict(resident_item_data)

            resident.append(resident_item)

        uuid = d.pop("uuid")

        def _parse_gauge(data: object) -> GpuLedgerGaugeResponse | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                gauge_type_1 = GpuLedgerGaugeResponse.from_dict(data)

                return gauge_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(GpuLedgerGaugeResponse | None | Unset, data)

        gauge = _parse_gauge(d.pop("gauge", UNSET))

        gpu_ledger_device_response = cls(
            resident=resident,
            uuid=uuid,
            gauge=gauge,
        )

        gpu_ledger_device_response.additional_properties = d
        return gpu_ledger_device_response

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
