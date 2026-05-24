from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.settings_section_view import SettingsSectionView


T = TypeVar("T", bound="SettingsDocumentView")


@_attrs_define
class SettingsDocumentView:
    """
    Attributes:
        schema_version (int):
        sections (list[SettingsSectionView]):
        settings_path (str):
        warnings (list[str]):
    """

    schema_version: int
    sections: list[SettingsSectionView]
    settings_path: str
    warnings: list[str]
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        schema_version = self.schema_version

        sections = []
        for sections_item_data in self.sections:
            sections_item = sections_item_data.to_dict()
            sections.append(sections_item)

        settings_path = self.settings_path

        warnings = self.warnings

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "schema_version": schema_version,
                "sections": sections,
                "settings_path": settings_path,
                "warnings": warnings,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.settings_section_view import SettingsSectionView

        d = dict(src_dict)
        schema_version = d.pop("schema_version")

        sections = []
        _sections = d.pop("sections")
        for sections_item_data in _sections:
            sections_item = SettingsSectionView.from_dict(sections_item_data)

            sections.append(sections_item)

        settings_path = d.pop("settings_path")

        warnings = cast(list[str], d.pop("warnings"))

        settings_document_view = cls(
            schema_version=schema_version,
            sections=sections,
            settings_path=settings_path,
            warnings=warnings,
        )

        settings_document_view.additional_properties = d
        return settings_document_view

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
