from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.agent_status_value import AgentStatusValue
from ..types import UNSET, Unset

T = TypeVar("T", bound="AgentThreadResponse")


@_attrs_define
class AgentThreadResponse:
    """Persisted agent thread summary.

    Attributes:
        config_json (str):
        created_at (str):
        depth (int):
        id (str):
        session_id (str):
        status (AgentStatusValue): Serializable mirror of [`AgentThreadStatus`].
        updated_at (str):
        completion_text (None | str | Unset):
        parent_id (None | str | Unset):
        role_name (None | str | Unset):
    """

    config_json: str
    created_at: str
    depth: int
    id: str
    session_id: str
    status: AgentStatusValue
    updated_at: str
    completion_text: None | str | Unset = UNSET
    parent_id: None | str | Unset = UNSET
    role_name: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        config_json = self.config_json

        created_at = self.created_at

        depth = self.depth

        id = self.id

        session_id = self.session_id

        status = self.status.value

        updated_at = self.updated_at

        completion_text: None | str | Unset
        if isinstance(self.completion_text, Unset):
            completion_text = UNSET
        else:
            completion_text = self.completion_text

        parent_id: None | str | Unset
        if isinstance(self.parent_id, Unset):
            parent_id = UNSET
        else:
            parent_id = self.parent_id

        role_name: None | str | Unset
        if isinstance(self.role_name, Unset):
            role_name = UNSET
        else:
            role_name = self.role_name

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "config_json": config_json,
                "created_at": created_at,
                "depth": depth,
                "id": id,
                "session_id": session_id,
                "status": status,
                "updated_at": updated_at,
            }
        )
        if completion_text is not UNSET:
            field_dict["completion_text"] = completion_text
        if parent_id is not UNSET:
            field_dict["parent_id"] = parent_id
        if role_name is not UNSET:
            field_dict["role_name"] = role_name

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        config_json = d.pop("config_json")

        created_at = d.pop("created_at")

        depth = d.pop("depth")

        id = d.pop("id")

        session_id = d.pop("session_id")

        status = AgentStatusValue(d.pop("status"))

        updated_at = d.pop("updated_at")

        def _parse_completion_text(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        completion_text = _parse_completion_text(d.pop("completion_text", UNSET))

        def _parse_parent_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        parent_id = _parse_parent_id(d.pop("parent_id", UNSET))

        def _parse_role_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        role_name = _parse_role_name(d.pop("role_name", UNSET))

        agent_thread_response = cls(
            config_json=config_json,
            created_at=created_at,
            depth=depth,
            id=id,
            session_id=session_id,
            status=status,
            updated_at=updated_at,
            completion_text=completion_text,
            parent_id=parent_id,
            role_name=role_name,
        )

        agent_thread_response.additional_properties = d
        return agent_thread_response

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
