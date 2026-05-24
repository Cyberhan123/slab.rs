from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.model_config_field_response import ModelConfigFieldResponse


T = TypeVar("T", bound="ModelConfigSectionResponse")


@_attrs_define
class ModelConfigSectionResponse:
    """
    Attributes:
        fields (list[ModelConfigFieldResponse]):
        id (str):
        label (str):
        description_md (None | str | Unset):
    """

    fields: list[ModelConfigFieldResponse]
    id: str
    label: str
    description_md: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        fields = []
        for fields_item_data in self.fields:
            fields_item = fields_item_data.to_dict()
            fields.append(fields_item)

        id = self.id

        label = self.label

        description_md: None | str | Unset
        if isinstance(self.description_md, Unset):
            description_md = UNSET
        else:
            description_md = self.description_md

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "fields": fields,
                "id": id,
                "label": label,
            }
        )
        if description_md is not UNSET:
            field_dict["description_md"] = description_md

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.model_config_field_response import ModelConfigFieldResponse

        d = dict(src_dict)
        fields = []
        _fields = d.pop("fields")
        for fields_item_data in _fields:
            fields_item = ModelConfigFieldResponse.from_dict(fields_item_data)

            fields.append(fields_item)

        id = d.pop("id")

        label = d.pop("label")

        def _parse_description_md(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description_md = _parse_description_md(d.pop("description_md", UNSET))

        model_config_section_response = cls(
            fields=fields,
            id=id,
            label=label,
            description_md=description_md,
        )

        model_config_section_response.additional_properties = d
        return model_config_section_response

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
