from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.plugin_language_server_transport_type_0 import (
        PluginLanguageServerTransportType0,
    )
    from ..models.plugin_language_server_transport_type_1 import (
        PluginLanguageServerTransportType1,
    )
    from ..models.plugin_language_server_transport_type_2 import (
        PluginLanguageServerTransportType2,
    )


T = TypeVar("T", bound="PluginLanguageServerContribution")


@_attrs_define
class PluginLanguageServerContribution:
    """
    Attributes:
        id (str):
        languages (list[str]):
        transport (PluginLanguageServerTransportType0 | PluginLanguageServerTransportType1 |
            PluginLanguageServerTransportType2):
    """

    id: str
    languages: list[str]
    transport: (
        PluginLanguageServerTransportType0
        | PluginLanguageServerTransportType1
        | PluginLanguageServerTransportType2
    )
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.plugin_language_server_transport_type_0 import (
            PluginLanguageServerTransportType0,
        )
        from ..models.plugin_language_server_transport_type_1 import (
            PluginLanguageServerTransportType1,
        )

        id = self.id

        languages = self.languages

        transport: dict[str, Any]
        if isinstance(self.transport, PluginLanguageServerTransportType0):
            transport = self.transport.to_dict()
        elif isinstance(self.transport, PluginLanguageServerTransportType1):
            transport = self.transport.to_dict()
        else:
            transport = self.transport.to_dict()

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "id": id,
                "languages": languages,
                "transport": transport,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.plugin_language_server_transport_type_0 import (
            PluginLanguageServerTransportType0,
        )
        from ..models.plugin_language_server_transport_type_1 import (
            PluginLanguageServerTransportType1,
        )
        from ..models.plugin_language_server_transport_type_2 import (
            PluginLanguageServerTransportType2,
        )

        d = dict(src_dict)
        id = d.pop("id")

        languages = cast(list[str], d.pop("languages"))

        def _parse_transport(
            data: object,
        ) -> (
            PluginLanguageServerTransportType0
            | PluginLanguageServerTransportType1
            | PluginLanguageServerTransportType2
        ):
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_plugin_language_server_transport_type_0 = (
                    PluginLanguageServerTransportType0.from_dict(data)
                )

                return componentsschemas_plugin_language_server_transport_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_plugin_language_server_transport_type_1 = (
                    PluginLanguageServerTransportType1.from_dict(data)
                )

                return componentsschemas_plugin_language_server_transport_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            if not isinstance(data, dict):
                raise TypeError()
            componentsschemas_plugin_language_server_transport_type_2 = (
                PluginLanguageServerTransportType2.from_dict(data)
            )

            return componentsschemas_plugin_language_server_transport_type_2

        transport = _parse_transport(d.pop("transport"))

        plugin_language_server_contribution = cls(
            id=id,
            languages=languages,
            transport=transport,
        )

        plugin_language_server_contribution.additional_properties = d
        return plugin_language_server_contribution

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
