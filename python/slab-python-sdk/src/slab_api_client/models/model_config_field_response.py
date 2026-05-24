from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.model_config_field_scope_response import ModelConfigFieldScopeResponse
from ..models.model_config_origin_response import ModelConfigOriginResponse
from ..models.model_config_value_type_response import ModelConfigValueTypeResponse
from ..types import UNSET, Unset

T = TypeVar("T", bound="ModelConfigFieldResponse")


@_attrs_define
class ModelConfigFieldResponse:
    """
    Attributes:
        editable (bool):
        effective_value (Any):
        label (str):
        locked (bool):
        origin (ModelConfigOriginResponse):
        path (str):
        scope (ModelConfigFieldScopeResponse):
        value_type (ModelConfigValueTypeResponse):
        description_md (None | str | Unset):
        json_schema (Any | Unset):
    """

    editable: bool
    effective_value: Any
    label: str
    locked: bool
    origin: ModelConfigOriginResponse
    path: str
    scope: ModelConfigFieldScopeResponse
    value_type: ModelConfigValueTypeResponse
    description_md: None | str | Unset = UNSET
    json_schema: Any | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        editable = self.editable

        effective_value = self.effective_value

        label = self.label

        locked = self.locked

        origin = self.origin.value

        path = self.path

        scope = self.scope.value

        value_type = self.value_type.value

        description_md: None | str | Unset
        if isinstance(self.description_md, Unset):
            description_md = UNSET
        else:
            description_md = self.description_md

        json_schema = self.json_schema

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "editable": editable,
                "effective_value": effective_value,
                "label": label,
                "locked": locked,
                "origin": origin,
                "path": path,
                "scope": scope,
                "value_type": value_type,
            }
        )
        if description_md is not UNSET:
            field_dict["description_md"] = description_md
        if json_schema is not UNSET:
            field_dict["json_schema"] = json_schema

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        editable = d.pop("editable")

        effective_value = d.pop("effective_value")

        label = d.pop("label")

        locked = d.pop("locked")

        origin = ModelConfigOriginResponse(d.pop("origin"))

        path = d.pop("path")

        scope = ModelConfigFieldScopeResponse(d.pop("scope"))

        value_type = ModelConfigValueTypeResponse(d.pop("value_type"))

        def _parse_description_md(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description_md = _parse_description_md(d.pop("description_md", UNSET))

        json_schema = d.pop("json_schema", UNSET)

        model_config_field_response = cls(
            editable=editable,
            effective_value=effective_value,
            label=label,
            locked=locked,
            origin=origin,
            path=path,
            scope=scope,
            value_type=value_type,
            description_md=description_md,
            json_schema=json_schema,
        )

        model_config_field_response.additional_properties = d
        return model_config_field_response

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
