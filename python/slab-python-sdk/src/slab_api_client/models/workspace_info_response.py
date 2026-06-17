from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="WorkspaceInfoResponse")


@_attrs_define
class WorkspaceInfoResponse:
    """
    Attributes:
        model_config_dir (str):
        name (str):
        root_path (str):
        session_state_dir (str):
        settings_path (str):
        slab_dir (str):
        settings_overlay_path (None | str | Unset):
    """

    model_config_dir: str
    name: str
    root_path: str
    session_state_dir: str
    settings_path: str
    slab_dir: str
    settings_overlay_path: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        model_config_dir = self.model_config_dir

        name = self.name

        root_path = self.root_path

        session_state_dir = self.session_state_dir

        settings_path = self.settings_path

        slab_dir = self.slab_dir

        settings_overlay_path: None | str | Unset
        if isinstance(self.settings_overlay_path, Unset):
            settings_overlay_path = UNSET
        else:
            settings_overlay_path = self.settings_overlay_path

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "modelConfigDir": model_config_dir,
                "name": name,
                "rootPath": root_path,
                "sessionStateDir": session_state_dir,
                "settingsPath": settings_path,
                "slabDir": slab_dir,
            }
        )
        if settings_overlay_path is not UNSET:
            field_dict["settingsOverlayPath"] = settings_overlay_path

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        model_config_dir = d.pop("modelConfigDir")

        name = d.pop("name")

        root_path = d.pop("rootPath")

        session_state_dir = d.pop("sessionStateDir")

        settings_path = d.pop("settingsPath")

        slab_dir = d.pop("slabDir")

        def _parse_settings_overlay_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        settings_overlay_path = _parse_settings_overlay_path(
            d.pop("settingsOverlayPath", UNSET)
        )

        workspace_info_response = cls(
            model_config_dir=model_config_dir,
            name=name,
            root_path=root_path,
            session_state_dir=session_state_dir,
            settings_path=settings_path,
            slab_dir=slab_dir,
            settings_overlay_path=settings_overlay_path,
        )

        workspace_info_response.additional_properties = d
        return workspace_info_response

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
