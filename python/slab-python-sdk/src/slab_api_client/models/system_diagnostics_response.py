from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.system_diagnostic_path_response import SystemDiagnosticPathResponse


T = TypeVar("T", bound="SystemDiagnosticsResponse")


@_attrs_define
class SystemDiagnosticsResponse:
    """Read-only support snapshot for local desktop diagnostics.

    Attributes:
        admin_token_configured (bool):
        cloud_http_trace_enabled (bool):
        generated_at (str):
        paths (list[SystemDiagnosticPathResponse]):
        status (str):
        swagger_enabled (bool):
        transport_mode (str):
        version (str):
        cors_allowed_origins (None | str | Unset):
    """

    admin_token_configured: bool
    cloud_http_trace_enabled: bool
    generated_at: str
    paths: list[SystemDiagnosticPathResponse]
    status: str
    swagger_enabled: bool
    transport_mode: str
    version: str
    cors_allowed_origins: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        admin_token_configured = self.admin_token_configured

        cloud_http_trace_enabled = self.cloud_http_trace_enabled

        generated_at = self.generated_at

        paths = []
        for paths_item_data in self.paths:
            paths_item = paths_item_data.to_dict()
            paths.append(paths_item)

        status = self.status

        swagger_enabled = self.swagger_enabled

        transport_mode = self.transport_mode

        version = self.version

        cors_allowed_origins: None | str | Unset
        if isinstance(self.cors_allowed_origins, Unset):
            cors_allowed_origins = UNSET
        else:
            cors_allowed_origins = self.cors_allowed_origins

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "admin_token_configured": admin_token_configured,
                "cloud_http_trace_enabled": cloud_http_trace_enabled,
                "generated_at": generated_at,
                "paths": paths,
                "status": status,
                "swagger_enabled": swagger_enabled,
                "transport_mode": transport_mode,
                "version": version,
            }
        )
        if cors_allowed_origins is not UNSET:
            field_dict["cors_allowed_origins"] = cors_allowed_origins

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.system_diagnostic_path_response import (
            SystemDiagnosticPathResponse,
        )

        d = dict(src_dict)
        admin_token_configured = d.pop("admin_token_configured")

        cloud_http_trace_enabled = d.pop("cloud_http_trace_enabled")

        generated_at = d.pop("generated_at")

        paths = []
        _paths = d.pop("paths")
        for paths_item_data in _paths:
            paths_item = SystemDiagnosticPathResponse.from_dict(paths_item_data)

            paths.append(paths_item)

        status = d.pop("status")

        swagger_enabled = d.pop("swagger_enabled")

        transport_mode = d.pop("transport_mode")

        version = d.pop("version")

        def _parse_cors_allowed_origins(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        cors_allowed_origins = _parse_cors_allowed_origins(
            d.pop("cors_allowed_origins", UNSET)
        )

        system_diagnostics_response = cls(
            admin_token_configured=admin_token_configured,
            cloud_http_trace_enabled=cloud_http_trace_enabled,
            generated_at=generated_at,
            paths=paths,
            status=status,
            swagger_enabled=swagger_enabled,
            transport_mode=transport_mode,
            version=version,
            cors_allowed_origins=cors_allowed_origins,
        )

        system_diagnostics_response.additional_properties = d
        return system_diagnostics_response

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
