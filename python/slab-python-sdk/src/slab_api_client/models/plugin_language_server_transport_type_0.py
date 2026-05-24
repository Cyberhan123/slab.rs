from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.plugin_language_server_transport_type_0_type import (
    PluginLanguageServerTransportType0Type,
)
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.slab_string_map import SlabStringMap


T = TypeVar("T", bound="PluginLanguageServerTransportType0")


@_attrs_define
class PluginLanguageServerTransportType0:
    """
    Attributes:
        command (str):
        type_ (PluginLanguageServerTransportType0Type):
        args (list[str] | Unset):
        env (SlabStringMap | Unset):
    """

    command: str
    type_: PluginLanguageServerTransportType0Type
    args: list[str] | Unset = UNSET
    env: SlabStringMap | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        command = self.command

        type_ = self.type_.value

        args: list[str] | Unset = UNSET
        if not isinstance(self.args, Unset):
            args = self.args

        env: dict[str, Any] | Unset = UNSET
        if not isinstance(self.env, Unset):
            env = self.env.to_dict()

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "command": command,
                "type": type_,
            }
        )
        if args is not UNSET:
            field_dict["args"] = args
        if env is not UNSET:
            field_dict["env"] = env

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.slab_string_map import SlabStringMap

        d = dict(src_dict)
        command = d.pop("command")

        type_ = PluginLanguageServerTransportType0Type(d.pop("type"))

        args = cast(list[str], d.pop("args", UNSET))

        _env = d.pop("env", UNSET)
        env: SlabStringMap | Unset
        if isinstance(_env, Unset):
            env = UNSET
        else:
            env = SlabStringMap.from_dict(_env)

        plugin_language_server_transport_type_0 = cls(
            command=command,
            type_=type_,
            args=args,
            env=env,
        )

        plugin_language_server_transport_type_0.additional_properties = d
        return plugin_language_server_transport_type_0

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
