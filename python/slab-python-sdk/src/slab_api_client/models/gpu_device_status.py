from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="GpuDeviceStatus")


@_attrs_define
class GpuDeviceStatus:
    """Per-GPU snapshot from `all-smi`.

    Attributes:
        device_type (str): Device class from all-smi, e.g. GPU / NPU.
        id (int): Stable GPU index in current snapshot.
        memory_usage_percent (float): Derived VRAM usage percentage (0-100).
        name (str): Human-readable GPU model name.
        power_draw_watts (float): Current power draw in watts.
        temperature_celsius (int): Reported temperature in Celsius.
        total_memory_bytes (int): Total VRAM bytes.
        used_memory_bytes (int): Current used VRAM bytes.
        utilization_percent (float): Core utilization in percentage (0-100).
    """

    device_type: str
    id: int
    memory_usage_percent: float
    name: str
    power_draw_watts: float
    temperature_celsius: int
    total_memory_bytes: int
    used_memory_bytes: int
    utilization_percent: float
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        device_type = self.device_type

        id = self.id

        memory_usage_percent = self.memory_usage_percent

        name = self.name

        power_draw_watts = self.power_draw_watts

        temperature_celsius = self.temperature_celsius

        total_memory_bytes = self.total_memory_bytes

        used_memory_bytes = self.used_memory_bytes

        utilization_percent = self.utilization_percent

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "device_type": device_type,
                "id": id,
                "memory_usage_percent": memory_usage_percent,
                "name": name,
                "power_draw_watts": power_draw_watts,
                "temperature_celsius": temperature_celsius,
                "total_memory_bytes": total_memory_bytes,
                "used_memory_bytes": used_memory_bytes,
                "utilization_percent": utilization_percent,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        device_type = d.pop("device_type")

        id = d.pop("id")

        memory_usage_percent = d.pop("memory_usage_percent")

        name = d.pop("name")

        power_draw_watts = d.pop("power_draw_watts")

        temperature_celsius = d.pop("temperature_celsius")

        total_memory_bytes = d.pop("total_memory_bytes")

        used_memory_bytes = d.pop("used_memory_bytes")

        utilization_percent = d.pop("utilization_percent")

        gpu_device_status = cls(
            device_type=device_type,
            id=id,
            memory_usage_percent=memory_usage_percent,
            name=name,
            power_draw_watts=power_draw_watts,
            temperature_celsius=temperature_celsius,
            total_memory_bytes=total_memory_bytes,
            used_memory_bytes=used_memory_bytes,
            utilization_percent=utilization_percent,
        )

        gpu_device_status.additional_properties = d
        return gpu_device_status

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
