from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.chat_model_source import ChatModelSource
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.chat_model_capabilities import ChatModelCapabilities


T = TypeVar("T", bound="ChatModelOption")


@_attrs_define
class ChatModelOption:
    """A selectable chat model option from the compatibility route `GET /v1/chat/models`.

    Attributes:
        capabilities (ChatModelCapabilities):
        display_name (str): User-facing display label.
        downloaded (bool): Whether model artifacts are already downloaded locally.
        id (str): Stable option id used in `POST /v1/chat/completions`.
        pending (bool): Whether a model download task is running.
        source (ChatModelSource): Chat model source type.
        backend_id (None | str | Unset): Backend id when `source = local`, e.g. `"ggml.llama"`.
        provider_id (None | str | Unset): Cloud provider id when `source = cloud`.
        provider_name (None | str | Unset): Cloud provider name when `source = cloud`.
    """

    capabilities: ChatModelCapabilities
    display_name: str
    downloaded: bool
    id: str
    pending: bool
    source: ChatModelSource
    backend_id: None | str | Unset = UNSET
    provider_id: None | str | Unset = UNSET
    provider_name: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        capabilities = self.capabilities.to_dict()

        display_name = self.display_name

        downloaded = self.downloaded

        id = self.id

        pending = self.pending

        source = self.source.value

        backend_id: None | str | Unset
        if isinstance(self.backend_id, Unset):
            backend_id = UNSET
        else:
            backend_id = self.backend_id

        provider_id: None | str | Unset
        if isinstance(self.provider_id, Unset):
            provider_id = UNSET
        else:
            provider_id = self.provider_id

        provider_name: None | str | Unset
        if isinstance(self.provider_name, Unset):
            provider_name = UNSET
        else:
            provider_name = self.provider_name

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "capabilities": capabilities,
                "display_name": display_name,
                "downloaded": downloaded,
                "id": id,
                "pending": pending,
                "source": source,
            }
        )
        if backend_id is not UNSET:
            field_dict["backend_id"] = backend_id
        if provider_id is not UNSET:
            field_dict["provider_id"] = provider_id
        if provider_name is not UNSET:
            field_dict["provider_name"] = provider_name

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.chat_model_capabilities import ChatModelCapabilities

        d = dict(src_dict)
        capabilities = ChatModelCapabilities.from_dict(d.pop("capabilities"))

        display_name = d.pop("display_name")

        downloaded = d.pop("downloaded")

        id = d.pop("id")

        pending = d.pop("pending")

        source = ChatModelSource(d.pop("source"))

        def _parse_backend_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        backend_id = _parse_backend_id(d.pop("backend_id", UNSET))

        def _parse_provider_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        provider_id = _parse_provider_id(d.pop("provider_id", UNSET))

        def _parse_provider_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        provider_name = _parse_provider_name(d.pop("provider_name", UNSET))

        chat_model_option = cls(
            capabilities=capabilities,
            display_name=display_name,
            downloaded=downloaded,
            id=id,
            pending=pending,
            source=source,
            backend_id=backend_id,
            provider_id=provider_id,
            provider_name=provider_name,
        )

        chat_model_option.additional_properties = d
        return chat_model_option

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
