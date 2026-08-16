from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

if TYPE_CHECKING:
    from ..models.gpu_ledger_device_response import GpuLedgerDeviceResponse


T = TypeVar("T", bound="GpuLedgerResponse")


@_attrs_define
class GpuLedgerResponse:
    """Resident-model memory ledger exposed at `/v1/system/gpu/ledger`
    (diagnostics-only; the `/v1/system/gpu` response shape stays frozen).

        Attributes:
            devices (list[GpuLedgerDeviceResponse]):
    """

    devices: list[GpuLedgerDeviceResponse]
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        devices = []
        for devices_item_data in self.devices:
            devices_item = devices_item_data.to_dict()
            devices.append(devices_item)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "devices": devices,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        from ..models.gpu_ledger_device_response import GpuLedgerDeviceResponse

        d = dict(src_dict)
        devices = []
        _devices = d.pop("devices")
        for devices_item_data in _devices:
            devices_item = GpuLedgerDeviceResponse.from_dict(devices_item_data)

            devices.append(devices_item)

        gpu_ledger_response = cls(
            devices=devices,
        )

        gpu_ledger_response.additional_properties = d
        return gpu_ledger_response

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
