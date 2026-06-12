from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.server_i18n_key import ServerI18NKey
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.i18n_message_ref_params import I18NMessageRefParams


T = TypeVar("T", bound="I18NMessageRef")


@_attrs_define
class I18NMessageRef:
    """
    Attributes:
        key (ServerI18NKey):
        params (I18NMessageRefParams | Unset):
    """

    key: ServerI18NKey
    params: I18NMessageRefParams | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        key = self.key.value

        params: dict[str, Any] | Unset = UNSET
        if not isinstance(self.params, Unset):
            params = self.params.to_dict()

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "key": key,
            }
        )
        if params is not UNSET:
            field_dict["params"] = params

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.i18n_message_ref_params import I18NMessageRefParams

        d = dict(src_dict)
        key = ServerI18NKey(d.pop("key"))

        _params = d.pop("params", UNSET)
        params: I18NMessageRefParams | Unset
        if isinstance(_params, Unset):
            params = UNSET
        else:
            params = I18NMessageRefParams.from_dict(_params)

        i18n_message_ref = cls(
            key=key,
            params=params,
        )

        i18n_message_ref.additional_properties = d
        return i18n_message_ref

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
