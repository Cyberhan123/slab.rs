from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="PluginRouteContribution")


@_attrs_define
class PluginRouteContribution:
    """
    Attributes:
        id (str):
        path (str):
        entry (None | str | Unset):
        title (None | str | Unset):
        title_key (None | str | Unset):
    """

    id: str
    path: str
    entry: None | str | Unset = UNSET
    title: None | str | Unset = UNSET
    title_key: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        id = self.id

        path = self.path

        entry: None | str | Unset
        if isinstance(self.entry, Unset):
            entry = UNSET
        else:
            entry = self.entry

        title: None | str | Unset
        if isinstance(self.title, Unset):
            title = UNSET
        else:
            title = self.title

        title_key: None | str | Unset
        if isinstance(self.title_key, Unset):
            title_key = UNSET
        else:
            title_key = self.title_key

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "id": id,
                "path": path,
            }
        )
        if entry is not UNSET:
            field_dict["entry"] = entry
        if title is not UNSET:
            field_dict["title"] = title
        if title_key is not UNSET:
            field_dict["titleKey"] = title_key

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        id = d.pop("id")

        path = d.pop("path")

        def _parse_entry(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        entry = _parse_entry(d.pop("entry", UNSET))

        def _parse_title(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        title = _parse_title(d.pop("title", UNSET))

        def _parse_title_key(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        title_key = _parse_title_key(d.pop("titleKey", UNSET))

        plugin_route_contribution = cls(
            id=id,
            path=path,
            entry=entry,
            title=title,
            title_key=title_key,
        )

        plugin_route_contribution.additional_properties = d
        return plugin_route_contribution

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
