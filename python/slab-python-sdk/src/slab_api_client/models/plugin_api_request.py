from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.slab_string_map import SlabStringMap


T = TypeVar("T", bound="PluginApiRequest")


@_attrs_define
class PluginApiRequest:
    """
    Attributes:
        method (str):
        path (str):
        body (None | str | Unset):
        headers (SlabStringMap | Unset):
        timeout_ms (int | None | Unset):
    """

    method: str
    path: str
    body: None | str | Unset = UNSET
    headers: SlabStringMap | Unset = UNSET
    timeout_ms: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        method = self.method

        path = self.path

        body: None | str | Unset
        if isinstance(self.body, Unset):
            body = UNSET
        else:
            body = self.body

        headers: dict[str, Any] | Unset = UNSET
        if not isinstance(self.headers, Unset):
            headers = self.headers.to_dict()

        timeout_ms: int | None | Unset
        if isinstance(self.timeout_ms, Unset):
            timeout_ms = UNSET
        else:
            timeout_ms = self.timeout_ms

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "method": method,
                "path": path,
            }
        )
        if body is not UNSET:
            field_dict["body"] = body
        if headers is not UNSET:
            field_dict["headers"] = headers
        if timeout_ms is not UNSET:
            field_dict["timeoutMs"] = timeout_ms

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.slab_string_map import SlabStringMap

        d = dict(src_dict)
        method = d.pop("method")

        path = d.pop("path")

        def _parse_body(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        body = _parse_body(d.pop("body", UNSET))

        _headers = d.pop("headers", UNSET)
        headers: SlabStringMap | Unset
        if isinstance(_headers, Unset):
            headers = UNSET
        else:
            headers = SlabStringMap.from_dict(_headers)

        def _parse_timeout_ms(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        timeout_ms = _parse_timeout_ms(d.pop("timeoutMs", UNSET))

        plugin_api_request = cls(
            method=method,
            path=path,
            body=body,
            headers=headers,
            timeout_ms=timeout_ms,
        )

        plugin_api_request.additional_properties = d
        return plugin_api_request

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
