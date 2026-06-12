from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.i18n_payload import I18NPayload


T = TypeVar("T", bound="OpenAiError")


@_attrs_define
class OpenAiError:
    """
    Attributes:
        message (str):
        type_ (str):
        code (None | str | Unset):
        i18n (I18NPayload | None | Unset):
        param (None | str | Unset):
    """

    message: str
    type_: str
    code: None | str | Unset = UNSET
    i18n: I18NPayload | None | Unset = UNSET
    param: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.i18n_payload import I18NPayload

        message = self.message

        type_ = self.type_

        code: None | str | Unset
        if isinstance(self.code, Unset):
            code = UNSET
        else:
            code = self.code

        i18n: dict[str, Any] | None | Unset
        if isinstance(self.i18n, Unset):
            i18n = UNSET
        elif isinstance(self.i18n, I18NPayload):
            i18n = self.i18n.to_dict()
        else:
            i18n = self.i18n

        param: None | str | Unset
        if isinstance(self.param, Unset):
            param = UNSET
        else:
            param = self.param

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "message": message,
                "type": type_,
            }
        )
        if code is not UNSET:
            field_dict["code"] = code
        if i18n is not UNSET:
            field_dict["i18n"] = i18n
        if param is not UNSET:
            field_dict["param"] = param

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.i18n_payload import I18NPayload

        d = dict(src_dict)
        message = d.pop("message")

        type_ = d.pop("type")

        def _parse_code(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        code = _parse_code(d.pop("code", UNSET))

        def _parse_i18n(data: object) -> I18NPayload | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                i18n_type_1 = I18NPayload.from_dict(data)

                return i18n_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(I18NPayload | None | Unset, data)

        i18n = _parse_i18n(d.pop("i18n", UNSET))

        def _parse_param(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        param = _parse_param(d.pop("param", UNSET))

        open_ai_error = cls(
            message=message,
            type_=type_,
            code=code,
            i18n=i18n,
            param=param,
        )

        open_ai_error.additional_properties = d
        return open_ai_error

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
