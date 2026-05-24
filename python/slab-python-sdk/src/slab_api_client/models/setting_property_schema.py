from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.setting_value_type import SettingValueType
from ..types import UNSET, Unset

T = TypeVar("T", bound="SettingPropertySchema")


@_attrs_define
class SettingPropertySchema:
    """
    Attributes:
        type_ (SettingValueType):
        default_value (Any | Unset):
        enum (list[str] | None | Unset):
        json_schema (Any | Unset):
        maximum (int | None | Unset):
        minimum (int | None | Unset):
        multiline (bool | Unset):
        order (int | Unset):
        pattern (None | str | Unset):
        secret (bool | Unset):
    """

    type_: SettingValueType
    default_value: Any | Unset = UNSET
    enum: list[str] | None | Unset = UNSET
    json_schema: Any | Unset = UNSET
    maximum: int | None | Unset = UNSET
    minimum: int | None | Unset = UNSET
    multiline: bool | Unset = UNSET
    order: int | Unset = UNSET
    pattern: None | str | Unset = UNSET
    secret: bool | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        type_ = self.type_.value

        default_value = self.default_value

        enum: list[str] | None | Unset
        if isinstance(self.enum, Unset):
            enum = UNSET
        elif isinstance(self.enum, list):
            enum = self.enum

        else:
            enum = self.enum

        json_schema = self.json_schema

        maximum: int | None | Unset
        if isinstance(self.maximum, Unset):
            maximum = UNSET
        else:
            maximum = self.maximum

        minimum: int | None | Unset
        if isinstance(self.minimum, Unset):
            minimum = UNSET
        else:
            minimum = self.minimum

        multiline = self.multiline

        order = self.order

        pattern: None | str | Unset
        if isinstance(self.pattern, Unset):
            pattern = UNSET
        else:
            pattern = self.pattern

        secret = self.secret

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "type": type_,
            }
        )
        if default_value is not UNSET:
            field_dict["default_value"] = default_value
        if enum is not UNSET:
            field_dict["enum"] = enum
        if json_schema is not UNSET:
            field_dict["json_schema"] = json_schema
        if maximum is not UNSET:
            field_dict["maximum"] = maximum
        if minimum is not UNSET:
            field_dict["minimum"] = minimum
        if multiline is not UNSET:
            field_dict["multiline"] = multiline
        if order is not UNSET:
            field_dict["order"] = order
        if pattern is not UNSET:
            field_dict["pattern"] = pattern
        if secret is not UNSET:
            field_dict["secret"] = secret

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        type_ = SettingValueType(d.pop("type"))

        default_value = d.pop("default_value", UNSET)

        def _parse_enum(data: object) -> list[str] | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                enum_type_0 = cast(list[str], data)

                return enum_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[str] | None | Unset, data)

        enum = _parse_enum(d.pop("enum", UNSET))

        json_schema = d.pop("json_schema", UNSET)

        def _parse_maximum(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        maximum = _parse_maximum(d.pop("maximum", UNSET))

        def _parse_minimum(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        minimum = _parse_minimum(d.pop("minimum", UNSET))

        multiline = d.pop("multiline", UNSET)

        order = d.pop("order", UNSET)

        def _parse_pattern(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        pattern = _parse_pattern(d.pop("pattern", UNSET))

        secret = d.pop("secret", UNSET)

        setting_property_schema = cls(
            type_=type_,
            default_value=default_value,
            enum=enum,
            json_schema=json_schema,
            maximum=maximum,
            minimum=minimum,
            multiline=multiline,
            order=order,
            pattern=pattern,
            secret=secret,
        )

        setting_property_schema.additional_properties = d
        return setting_property_schema

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
