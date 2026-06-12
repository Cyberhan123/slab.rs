from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.plugin_agent_hook_lifecycle_event import PluginAgentHookLifecycleEvent
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.plugin_agent_hook_transport import PluginAgentHookTransport


T = TypeVar("T", bound="PluginAgentHookContribution")


@_attrs_define
class PluginAgentHookContribution:
    """
    Attributes:
        events (list[PluginAgentHookLifecycleEvent]):
        id (str):
        transport (PluginAgentHookTransport):
        description (None | str | Unset):
        description_key (None | str | Unset):
    """

    events: list[PluginAgentHookLifecycleEvent]
    id: str
    transport: PluginAgentHookTransport
    description: None | str | Unset = UNSET
    description_key: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        events = []
        for events_item_data in self.events:
            events_item = events_item_data.value
            events.append(events_item)

        id = self.id

        transport = self.transport.to_dict()

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        description_key: None | str | Unset
        if isinstance(self.description_key, Unset):
            description_key = UNSET
        else:
            description_key = self.description_key

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "events": events,
                "id": id,
                "transport": transport,
            }
        )
        if description is not UNSET:
            field_dict["description"] = description
        if description_key is not UNSET:
            field_dict["descriptionKey"] = description_key

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.plugin_agent_hook_transport import PluginAgentHookTransport

        d = dict(src_dict)
        events = []
        _events = d.pop("events")
        for events_item_data in _events:
            events_item = PluginAgentHookLifecycleEvent(events_item_data)

            events.append(events_item)

        id = d.pop("id")

        transport = PluginAgentHookTransport.from_dict(d.pop("transport"))

        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))

        def _parse_description_key(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description_key = _parse_description_key(d.pop("descriptionKey", UNSET))

        plugin_agent_hook_contribution = cls(
            events=events,
            id=id,
            transport=transport,
            description=description,
            description_key=description_key,
        )

        plugin_agent_hook_contribution.additional_properties = d
        return plugin_agent_hook_contribution

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
