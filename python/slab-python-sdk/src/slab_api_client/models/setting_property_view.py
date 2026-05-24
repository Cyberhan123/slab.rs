from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.setting_property_schema import SettingPropertySchema


T = TypeVar("T", bound="SettingPropertyView")


@_attrs_define
class SettingPropertyView:
    """
    Attributes:
        editable (bool):
        effective_value (Any):
        is_overridden (bool):
        label (str):
        pmid (str):
        schema (SettingPropertySchema):
        search_terms (list[str]):
        description_md (str | Unset):
        override_value (Any | None | Unset):
    """

    editable: bool
    effective_value: Any
    is_overridden: bool
    label: str
    pmid: str
    schema: SettingPropertySchema
    search_terms: list[str]
    description_md: str | Unset = UNSET
    override_value: Any | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        editable = self.editable

        effective_value = self.effective_value

        is_overridden = self.is_overridden

        label = self.label

        pmid = self.pmid

        schema = self.schema.to_dict()

        search_terms = self.search_terms

        description_md = self.description_md

        override_value: Any | None | Unset
        if isinstance(self.override_value, Unset):
            override_value = UNSET
        else:
            override_value = self.override_value

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "editable": editable,
                "effective_value": effective_value,
                "is_overridden": is_overridden,
                "label": label,
                "pmid": pmid,
                "schema": schema,
                "search_terms": search_terms,
            }
        )
        if description_md is not UNSET:
            field_dict["description_md"] = description_md
        if override_value is not UNSET:
            field_dict["override_value"] = override_value

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.setting_property_schema import SettingPropertySchema

        d = dict(src_dict)
        editable = d.pop("editable")

        effective_value = d.pop("effective_value")

        is_overridden = d.pop("is_overridden")

        label = d.pop("label")

        pmid = d.pop("pmid")

        schema = SettingPropertySchema.from_dict(d.pop("schema"))

        search_terms = cast(list[str], d.pop("search_terms"))

        description_md = d.pop("description_md", UNSET)

        def _parse_override_value(data: object) -> Any | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(Any | None | Unset, data)

        override_value = _parse_override_value(d.pop("override_value", UNSET))

        setting_property_view = cls(
            editable=editable,
            effective_value=effective_value,
            is_overridden=is_overridden,
            label=label,
            pmid=pmid,
            schema=schema,
            search_terms=search_terms,
            description_md=description_md,
            override_value=override_value,
        )

        setting_property_view.additional_properties = d
        return setting_property_view

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
