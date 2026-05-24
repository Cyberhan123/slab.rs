from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.plugin_language_server_transport_type_2_type import (
    PluginLanguageServerTransportType2Type,
)
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.slab_string_map import SlabStringMap


T = TypeVar("T", bound="PluginLanguageServerTransportType2")


@_attrs_define
class PluginLanguageServerTransportType2:
    """Node.js package-based language server bundled inside the plugin directory.
    The runtime looks for the command binary in the plugin's `node_modules/.bin/`
    directory before falling back to the system PATH, so the language server
    can be shipped as a plain npm dependency of the plugin without requiring
    any system-wide installation.  This is the recommended transport for
    wrapping VS Code extension language servers (e.g. `typescript-language-server`,
    `pyright-langserver`) as slab plugins.

        Attributes:
            package (str): npm package name used to identify this language server (e.g.
                `"typescript-language-server"`).  The package must be present in the
                plugin's install directory under `node_modules/`.
            type_ (PluginLanguageServerTransportType2Type):
            args (list[str] | Unset):
            command (None | str | Unset): Executable name to invoke.  Defaults to the value of `package` when
                omitted.  Resolved against the plugin's `node_modules/.bin/` first.
            env (SlabStringMap | Unset):
    """

    package: str
    type_: PluginLanguageServerTransportType2Type
    args: list[str] | Unset = UNSET
    command: None | str | Unset = UNSET
    env: SlabStringMap | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        package = self.package

        type_ = self.type_.value

        args: list[str] | Unset = UNSET
        if not isinstance(self.args, Unset):
            args = self.args

        command: None | str | Unset
        if isinstance(self.command, Unset):
            command = UNSET
        else:
            command = self.command

        env: dict[str, Any] | Unset = UNSET
        if not isinstance(self.env, Unset):
            env = self.env.to_dict()

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "package": package,
                "type": type_,
            }
        )
        if args is not UNSET:
            field_dict["args"] = args
        if command is not UNSET:
            field_dict["command"] = command
        if env is not UNSET:
            field_dict["env"] = env

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.slab_string_map import SlabStringMap

        d = dict(src_dict)
        package = d.pop("package")

        type_ = PluginLanguageServerTransportType2Type(d.pop("type"))

        args = cast(list[str], d.pop("args", UNSET))

        def _parse_command(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        command = _parse_command(d.pop("command", UNSET))

        _env = d.pop("env", UNSET)
        env: SlabStringMap | Unset
        if isinstance(_env, Unset):
            env = UNSET
        else:
            env = SlabStringMap.from_dict(_env)

        plugin_language_server_transport_type_2 = cls(
            package=package,
            type_=type_,
            args=args,
            command=command,
            env=env,
        )

        plugin_language_server_transport_type_2.additional_properties = d
        return plugin_language_server_transport_type_2

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
