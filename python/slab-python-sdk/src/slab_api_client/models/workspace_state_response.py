from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.recent_workspace_response import RecentWorkspaceResponse
    from ..models.workspace_config_response import WorkspaceConfigResponse
    from ..models.workspace_info_response import WorkspaceInfoResponse


T = TypeVar("T", bound="WorkspaceStateResponse")


@_attrs_define
class WorkspaceStateResponse:
    """
    Attributes:
        recent (list[RecentWorkspaceResponse]):
        config (None | Unset | WorkspaceConfigResponse):
        current (None | Unset | WorkspaceInfoResponse):
    """

    recent: list[RecentWorkspaceResponse]
    config: None | Unset | WorkspaceConfigResponse = UNSET
    current: None | Unset | WorkspaceInfoResponse = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.workspace_config_response import WorkspaceConfigResponse
        from ..models.workspace_info_response import WorkspaceInfoResponse

        recent = []
        for recent_item_data in self.recent:
            recent_item = recent_item_data.to_dict()
            recent.append(recent_item)

        config: dict[str, Any] | None | Unset
        if isinstance(self.config, Unset):
            config = UNSET
        elif isinstance(self.config, WorkspaceConfigResponse):
            config = self.config.to_dict()
        else:
            config = self.config

        current: dict[str, Any] | None | Unset
        if isinstance(self.current, Unset):
            current = UNSET
        elif isinstance(self.current, WorkspaceInfoResponse):
            current = self.current.to_dict()
        else:
            current = self.current

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "recent": recent,
            }
        )
        if config is not UNSET:
            field_dict["config"] = config
        if current is not UNSET:
            field_dict["current"] = current

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.recent_workspace_response import RecentWorkspaceResponse
        from ..models.workspace_config_response import WorkspaceConfigResponse
        from ..models.workspace_info_response import WorkspaceInfoResponse

        d = dict(src_dict)
        recent = []
        _recent = d.pop("recent")
        for recent_item_data in _recent:
            recent_item = RecentWorkspaceResponse.from_dict(recent_item_data)

            recent.append(recent_item)

        def _parse_config(data: object) -> None | Unset | WorkspaceConfigResponse:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                config_type_1 = WorkspaceConfigResponse.from_dict(data)

                return config_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | WorkspaceConfigResponse, data)

        config = _parse_config(d.pop("config", UNSET))

        def _parse_current(data: object) -> None | Unset | WorkspaceInfoResponse:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                current_type_1 = WorkspaceInfoResponse.from_dict(data)

                return current_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | WorkspaceInfoResponse, data)

        current = _parse_current(d.pop("current", UNSET))

        workspace_state_response = cls(
            recent=recent,
            config=config,
            current=current,
        )

        workspace_state_response.additional_properties = d
        return workspace_state_response

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
