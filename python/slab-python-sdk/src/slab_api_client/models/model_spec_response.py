from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.pricing_response import PricingResponse


T = TypeVar("T", bound="ModelSpecResponse")


@_attrs_define
class ModelSpecResponse:
    """Provider-specific model configuration (response).

    Attributes:
        context_window (int | None | Unset):
        filename (None | str | Unset):
        hub_provider (None | str | Unset):
        local_path (None | str | Unset):
        pricing (None | PricingResponse | Unset):
        provider_id (None | str | Unset):
        remote_model_id (None | str | Unset):
        repo_id (None | str | Unset):
    """

    context_window: int | None | Unset = UNSET
    filename: None | str | Unset = UNSET
    hub_provider: None | str | Unset = UNSET
    local_path: None | str | Unset = UNSET
    pricing: None | PricingResponse | Unset = UNSET
    provider_id: None | str | Unset = UNSET
    remote_model_id: None | str | Unset = UNSET
    repo_id: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.pricing_response import PricingResponse

        context_window: int | None | Unset
        if isinstance(self.context_window, Unset):
            context_window = UNSET
        else:
            context_window = self.context_window

        filename: None | str | Unset
        if isinstance(self.filename, Unset):
            filename = UNSET
        else:
            filename = self.filename

        hub_provider: None | str | Unset
        if isinstance(self.hub_provider, Unset):
            hub_provider = UNSET
        else:
            hub_provider = self.hub_provider

        local_path: None | str | Unset
        if isinstance(self.local_path, Unset):
            local_path = UNSET
        else:
            local_path = self.local_path

        pricing: dict[str, Any] | None | Unset
        if isinstance(self.pricing, Unset):
            pricing = UNSET
        elif isinstance(self.pricing, PricingResponse):
            pricing = self.pricing.to_dict()
        else:
            pricing = self.pricing

        provider_id: None | str | Unset
        if isinstance(self.provider_id, Unset):
            provider_id = UNSET
        else:
            provider_id = self.provider_id

        remote_model_id: None | str | Unset
        if isinstance(self.remote_model_id, Unset):
            remote_model_id = UNSET
        else:
            remote_model_id = self.remote_model_id

        repo_id: None | str | Unset
        if isinstance(self.repo_id, Unset):
            repo_id = UNSET
        else:
            repo_id = self.repo_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if context_window is not UNSET:
            field_dict["context_window"] = context_window
        if filename is not UNSET:
            field_dict["filename"] = filename
        if hub_provider is not UNSET:
            field_dict["hub_provider"] = hub_provider
        if local_path is not UNSET:
            field_dict["local_path"] = local_path
        if pricing is not UNSET:
            field_dict["pricing"] = pricing
        if provider_id is not UNSET:
            field_dict["provider_id"] = provider_id
        if remote_model_id is not UNSET:
            field_dict["remote_model_id"] = remote_model_id
        if repo_id is not UNSET:
            field_dict["repo_id"] = repo_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.pricing_response import PricingResponse

        d = dict(src_dict)

        def _parse_context_window(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        context_window = _parse_context_window(d.pop("context_window", UNSET))

        def _parse_filename(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        filename = _parse_filename(d.pop("filename", UNSET))

        def _parse_hub_provider(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        hub_provider = _parse_hub_provider(d.pop("hub_provider", UNSET))

        def _parse_local_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        local_path = _parse_local_path(d.pop("local_path", UNSET))

        def _parse_pricing(data: object) -> None | PricingResponse | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                pricing_type_1 = PricingResponse.from_dict(data)

                return pricing_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | PricingResponse | Unset, data)

        pricing = _parse_pricing(d.pop("pricing", UNSET))

        def _parse_provider_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        provider_id = _parse_provider_id(d.pop("provider_id", UNSET))

        def _parse_remote_model_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        remote_model_id = _parse_remote_model_id(d.pop("remote_model_id", UNSET))

        def _parse_repo_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        repo_id = _parse_repo_id(d.pop("repo_id", UNSET))

        model_spec_response = cls(
            context_window=context_window,
            filename=filename,
            hub_provider=hub_provider,
            local_path=local_path,
            pricing=pricing,
            provider_id=provider_id,
            remote_model_id=remote_model_id,
            repo_id=repo_id,
        )

        model_spec_response.additional_properties = d
        return model_spec_response

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
