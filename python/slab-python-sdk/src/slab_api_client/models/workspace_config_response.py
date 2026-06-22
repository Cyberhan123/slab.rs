from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.workspace_config_response_plugins import (
        WorkspaceConfigResponsePlugins,
    )


T = TypeVar("T", bound="WorkspaceConfigResponse")


@_attrs_define
class WorkspaceConfigResponse:
    """
    Attributes:
        schema_version (int):
        plugins (WorkspaceConfigResponsePlugins | Unset):
    """

    schema_version: int
    plugins: WorkspaceConfigResponsePlugins | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        schema_version = self.schema_version

        plugins: dict[str, Any] | Unset = UNSET
        if not isinstance(self.plugins, Unset):
            plugins = self.plugins.to_dict()

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "schemaVersion": schema_version,
            }
        )
        if plugins is not UNSET:
            field_dict["plugins"] = plugins

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.workspace_config_response_plugins import (
            WorkspaceConfigResponsePlugins,
        )

        d = dict(src_dict)
        schema_version = d.pop("schemaVersion")

        _plugins = d.pop("plugins", UNSET)
        plugins: WorkspaceConfigResponsePlugins | Unset
        if isinstance(_plugins, Unset):
            plugins = UNSET
        else:
            plugins = WorkspaceConfigResponsePlugins.from_dict(_plugins)

        workspace_config_response = cls(
            schema_version=schema_version,
            plugins=plugins,
        )

        workspace_config_response.additional_properties = d
        return workspace_config_response

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
