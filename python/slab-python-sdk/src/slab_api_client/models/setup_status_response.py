from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.component_status_response import ComponentStatusResponse


T = TypeVar("T", bound="SetupStatusResponse")


@_attrs_define
class SetupStatusResponse:
    """Response body for `GET /v1/setup/status`.

    Attributes:
        backends (list[ComponentStatusResponse]): AI backend library availability (one entry per backend).
        ffmpeg (ComponentStatusResponse): Availability information for a single environment component.
        initialized (bool): Whether the one-time setup wizard has been completed.
        runtime_payload_installed (bool): Whether the packaged runtime payload is already present under
            `resources/libs`.
    """

    backends: list[ComponentStatusResponse]
    ffmpeg: ComponentStatusResponse
    initialized: bool
    runtime_payload_installed: bool
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        backends = []
        for backends_item_data in self.backends:
            backends_item = backends_item_data.to_dict()
            backends.append(backends_item)

        ffmpeg = self.ffmpeg.to_dict()

        initialized = self.initialized

        runtime_payload_installed = self.runtime_payload_installed

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "backends": backends,
                "ffmpeg": ffmpeg,
                "initialized": initialized,
                "runtime_payload_installed": runtime_payload_installed,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.component_status_response import ComponentStatusResponse

        d = dict(src_dict)
        backends = []
        _backends = d.pop("backends")
        for backends_item_data in _backends:
            backends_item = ComponentStatusResponse.from_dict(backends_item_data)

            backends.append(backends_item)

        ffmpeg = ComponentStatusResponse.from_dict(d.pop("ffmpeg"))

        initialized = d.pop("initialized")

        runtime_payload_installed = d.pop("runtime_payload_installed")

        setup_status_response = cls(
            backends=backends,
            ffmpeg=ffmpeg,
            initialized=initialized,
            runtime_payload_installed=runtime_payload_installed,
        )

        setup_status_response.additional_properties = d
        return setup_status_response

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
