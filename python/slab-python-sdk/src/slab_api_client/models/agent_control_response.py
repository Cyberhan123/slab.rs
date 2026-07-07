from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.agent_status_value import AgentStatusValue
from ..types import UNSET, Unset

T = TypeVar("T", bound="AgentControlResponse")


@_attrs_define
class AgentControlResponse:
    """
    Attributes:
        thread_id (str):
        delivered (bool | None | Unset):
        status (AgentStatusValue | None | Unset):
    """

    thread_id: str
    delivered: bool | None | Unset = UNSET
    status: AgentStatusValue | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        thread_id = self.thread_id

        delivered: bool | None | Unset
        if isinstance(self.delivered, Unset):
            delivered = UNSET
        else:
            delivered = self.delivered

        status: None | str | Unset
        if isinstance(self.status, Unset):
            status = UNSET
        elif isinstance(self.status, AgentStatusValue):
            status = self.status.value
        else:
            status = self.status

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "thread_id": thread_id,
            }
        )
        if delivered is not UNSET:
            field_dict["delivered"] = delivered
        if status is not UNSET:
            field_dict["status"] = status

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        thread_id = d.pop("thread_id")

        def _parse_delivered(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        delivered = _parse_delivered(d.pop("delivered", UNSET))

        def _parse_status(data: object) -> AgentStatusValue | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                status_type_1 = AgentStatusValue(data)

                return status_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(AgentStatusValue | None | Unset, data)

        status = _parse_status(d.pop("status", UNSET))

        agent_control_response = cls(
            thread_id=thread_id,
            delivered=delivered,
            status=status,
        )

        agent_control_response.additional_properties = d
        return agent_control_response

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
