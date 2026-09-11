from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

from ..types import UNSET, Unset

T = TypeVar("T", bound="RolloutSessionEntry")


@_attrs_define
class RolloutSessionEntry:
    """One rollout session (thread) discovered on disk.

    Attributes:
        file_name (str):
        has_trace (bool): Whether the rollout links to a trace bundle (`SessionMeta.trace_path`).
        session_id (str):
        size_bytes (int):
        started_at (str):
        thread_id (str):
        role_name (None | str | Unset):
    """

    file_name: str
    has_trace: bool
    session_id: str
    size_bytes: int
    started_at: str
    thread_id: str
    role_name: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        file_name = self.file_name

        has_trace = self.has_trace

        session_id = self.session_id

        size_bytes = self.size_bytes

        started_at = self.started_at

        thread_id = self.thread_id

        role_name: None | str | Unset
        if isinstance(self.role_name, Unset):
            role_name = UNSET
        else:
            role_name = self.role_name

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "file_name": file_name,
                "has_trace": has_trace,
                "session_id": session_id,
                "size_bytes": size_bytes,
                "started_at": started_at,
                "thread_id": thread_id,
            }
        )
        if role_name is not UNSET:
            field_dict["role_name"] = role_name

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        d = dict(src_dict)
        file_name = d.pop("file_name")

        has_trace = d.pop("has_trace")

        session_id = d.pop("session_id")

        size_bytes = d.pop("size_bytes")

        started_at = d.pop("started_at")

        thread_id = d.pop("thread_id")

        def _parse_role_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        role_name = _parse_role_name(d.pop("role_name", UNSET))

        rollout_session_entry = cls(
            file_name=file_name,
            has_trace=has_trace,
            session_id=session_id,
            size_bytes=size_bytes,
            started_at=started_at,
            thread_id=thread_id,
            role_name=role_name,
        )

        rollout_session_entry.additional_properties = d
        return rollout_session_entry

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
