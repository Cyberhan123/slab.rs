from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.settings_subsection_view import SettingsSubsectionView


T = TypeVar("T", bound="SettingsSectionView")


@_attrs_define
class SettingsSectionView:
    """
    Attributes:
        id (str):
        subsections (list[SettingsSubsectionView]):
        title (str):
        description_md (str | Unset):
    """

    id: str
    subsections: list[SettingsSubsectionView]
    title: str
    description_md: str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        id = self.id

        subsections = []
        for subsections_item_data in self.subsections:
            subsections_item = subsections_item_data.to_dict()
            subsections.append(subsections_item)

        title = self.title

        description_md = self.description_md

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "id": id,
                "subsections": subsections,
                "title": title,
            }
        )
        if description_md is not UNSET:
            field_dict["description_md"] = description_md

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.settings_subsection_view import SettingsSubsectionView

        d = dict(src_dict)
        id = d.pop("id")

        subsections = []
        _subsections = d.pop("subsections")
        for subsections_item_data in _subsections:
            subsections_item = SettingsSubsectionView.from_dict(subsections_item_data)

            subsections.append(subsections_item)

        title = d.pop("title")

        description_md = d.pop("description_md", UNSET)

        settings_section_view = cls(
            id=id,
            subsections=subsections,
            title=title,
            description_md=description_md,
        )

        settings_section_view.additional_properties = d
        return settings_section_view

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
