from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.plugin_agent_capability_contribution import (
        PluginAgentCapabilityContribution,
    )
    from ..models.plugin_command_contribution import PluginCommandContribution
    from ..models.plugin_language_server_contribution import (
        PluginLanguageServerContribution,
    )
    from ..models.plugin_route_contribution import PluginRouteContribution
    from ..models.plugin_settings_contribution import PluginSettingsContribution
    from ..models.plugin_sidebar_contribution import PluginSidebarContribution


T = TypeVar("T", bound="PluginContributesManifest")


@_attrs_define
class PluginContributesManifest:
    """
    Attributes:
        agent_capabilities (list[PluginAgentCapabilityContribution] | Unset):
        commands (list[PluginCommandContribution] | Unset):
        language_servers (list[PluginLanguageServerContribution] | Unset):
        routes (list[PluginRouteContribution] | Unset):
        settings (list[PluginSettingsContribution] | Unset):
        sidebar (list[PluginSidebarContribution] | Unset):
    """

    agent_capabilities: list[PluginAgentCapabilityContribution] | Unset = UNSET
    commands: list[PluginCommandContribution] | Unset = UNSET
    language_servers: list[PluginLanguageServerContribution] | Unset = UNSET
    routes: list[PluginRouteContribution] | Unset = UNSET
    settings: list[PluginSettingsContribution] | Unset = UNSET
    sidebar: list[PluginSidebarContribution] | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        agent_capabilities: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.agent_capabilities, Unset):
            agent_capabilities = []
            for agent_capabilities_item_data in self.agent_capabilities:
                agent_capabilities_item = agent_capabilities_item_data.to_dict()
                agent_capabilities.append(agent_capabilities_item)

        commands: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.commands, Unset):
            commands = []
            for commands_item_data in self.commands:
                commands_item = commands_item_data.to_dict()
                commands.append(commands_item)

        language_servers: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.language_servers, Unset):
            language_servers = []
            for language_servers_item_data in self.language_servers:
                language_servers_item = language_servers_item_data.to_dict()
                language_servers.append(language_servers_item)

        routes: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.routes, Unset):
            routes = []
            for routes_item_data in self.routes:
                routes_item = routes_item_data.to_dict()
                routes.append(routes_item)

        settings: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.settings, Unset):
            settings = []
            for settings_item_data in self.settings:
                settings_item = settings_item_data.to_dict()
                settings.append(settings_item)

        sidebar: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.sidebar, Unset):
            sidebar = []
            for sidebar_item_data in self.sidebar:
                sidebar_item = sidebar_item_data.to_dict()
                sidebar.append(sidebar_item)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if agent_capabilities is not UNSET:
            field_dict["agentCapabilities"] = agent_capabilities
        if commands is not UNSET:
            field_dict["commands"] = commands
        if language_servers is not UNSET:
            field_dict["languageServers"] = language_servers
        if routes is not UNSET:
            field_dict["routes"] = routes
        if settings is not UNSET:
            field_dict["settings"] = settings
        if sidebar is not UNSET:
            field_dict["sidebar"] = sidebar

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.plugin_agent_capability_contribution import (
            PluginAgentCapabilityContribution,
        )
        from ..models.plugin_command_contribution import PluginCommandContribution
        from ..models.plugin_language_server_contribution import (
            PluginLanguageServerContribution,
        )
        from ..models.plugin_route_contribution import PluginRouteContribution
        from ..models.plugin_settings_contribution import PluginSettingsContribution
        from ..models.plugin_sidebar_contribution import PluginSidebarContribution

        d = dict(src_dict)
        _agent_capabilities = d.pop("agentCapabilities", UNSET)
        agent_capabilities: list[PluginAgentCapabilityContribution] | Unset = UNSET
        if _agent_capabilities is not UNSET:
            agent_capabilities = []
            for agent_capabilities_item_data in _agent_capabilities:
                agent_capabilities_item = PluginAgentCapabilityContribution.from_dict(
                    agent_capabilities_item_data
                )

                agent_capabilities.append(agent_capabilities_item)

        _commands = d.pop("commands", UNSET)
        commands: list[PluginCommandContribution] | Unset = UNSET
        if _commands is not UNSET:
            commands = []
            for commands_item_data in _commands:
                commands_item = PluginCommandContribution.from_dict(commands_item_data)

                commands.append(commands_item)

        _language_servers = d.pop("languageServers", UNSET)
        language_servers: list[PluginLanguageServerContribution] | Unset = UNSET
        if _language_servers is not UNSET:
            language_servers = []
            for language_servers_item_data in _language_servers:
                language_servers_item = PluginLanguageServerContribution.from_dict(
                    language_servers_item_data
                )

                language_servers.append(language_servers_item)

        _routes = d.pop("routes", UNSET)
        routes: list[PluginRouteContribution] | Unset = UNSET
        if _routes is not UNSET:
            routes = []
            for routes_item_data in _routes:
                routes_item = PluginRouteContribution.from_dict(routes_item_data)

                routes.append(routes_item)

        _settings = d.pop("settings", UNSET)
        settings: list[PluginSettingsContribution] | Unset = UNSET
        if _settings is not UNSET:
            settings = []
            for settings_item_data in _settings:
                settings_item = PluginSettingsContribution.from_dict(settings_item_data)

                settings.append(settings_item)

        _sidebar = d.pop("sidebar", UNSET)
        sidebar: list[PluginSidebarContribution] | Unset = UNSET
        if _sidebar is not UNSET:
            sidebar = []
            for sidebar_item_data in _sidebar:
                sidebar_item = PluginSidebarContribution.from_dict(sidebar_item_data)

                sidebar.append(sidebar_item)

        plugin_contributes_manifest = cls(
            agent_capabilities=agent_capabilities,
            commands=commands,
            language_servers=language_servers,
            routes=routes,
            settings=settings,
            sidebar=sidebar,
        )

        plugin_contributes_manifest.additional_properties = d
        return plugin_contributes_manifest

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
