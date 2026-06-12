from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.i18n_message_ref import I18NMessageRef


T = TypeVar("T", bound="I18NPayload")


@_attrs_define
class I18NPayload:
    """ """

    additional_properties: dict[str, I18NMessageRef] = _attrs_field(
        init=False, factory=dict
    )

    def to_dict(self) -> dict[str, Any]:

        field_dict: dict[str, Any] = {}
        for prop_name, prop in self.additional_properties.items():
            field_dict[prop_name] = prop.to_dict()

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.i18n_message_ref import I18NMessageRef

        d = dict(src_dict)
        i18n_payload = cls()

        additional_properties = {}
        for prop_name, prop_dict in d.items():
            additional_property = I18NMessageRef.from_dict(prop_dict)

            additional_properties[prop_name] = additional_property

        i18n_payload.additional_properties = additional_properties
        return i18n_payload

    @property
    def additional_keys(self) -> list[str]:
        return list(self.additional_properties.keys())

    def __getitem__(self, key: str) -> I18NMessageRef:
        return self.additional_properties[key]

    def __setitem__(self, key: str, value: I18NMessageRef) -> None:
        self.additional_properties[key] = value

    def __delitem__(self, key: str) -> None:
        del self.additional_properties[key]

    def __contains__(self, key: str) -> bool:
        return key in self.additional_properties
