from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="SessionResponse")


@_attrs_define
class SessionResponse:
    """Response for a single chat session.

    Attributes:
        created_at (str):
        id (str):
        name (str):
        updated_at (str):
        state_path (None | str | Unset):
    """

    created_at: str
    id: str
    name: str
    updated_at: str
    state_path: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        created_at = self.created_at

        id = self.id

        name = self.name

        updated_at = self.updated_at

        state_path: None | str | Unset
        if isinstance(self.state_path, Unset):
            state_path = UNSET
        else:
            state_path = self.state_path

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "created_at": created_at,
                "id": id,
                "name": name,
                "updated_at": updated_at,
            }
        )
        if state_path is not UNSET:
            field_dict["state_path"] = state_path

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        created_at = d.pop("created_at")

        id = d.pop("id")

        name = d.pop("name")

        updated_at = d.pop("updated_at")

        def _parse_state_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        state_path = _parse_state_path(d.pop("state_path", UNSET))

        session_response = cls(
            created_at=created_at,
            id=id,
            name=name,
            updated_at=updated_at,
            state_path=state_path,
        )

        session_response.additional_properties = d
        return session_response

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
