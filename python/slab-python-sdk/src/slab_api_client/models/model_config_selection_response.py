from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.model_config_preset_option_response import (
        ModelConfigPresetOptionResponse,
    )
    from ..models.model_config_variant_option_response import (
        ModelConfigVariantOptionResponse,
    )


T = TypeVar("T", bound="ModelConfigSelectionResponse")


@_attrs_define
class ModelConfigSelectionResponse:
    """
    Attributes:
        presets (list[ModelConfigPresetOptionResponse]):
        variants (list[ModelConfigVariantOptionResponse]):
        default_preset_id (None | str | Unset):
        default_variant_id (None | str | Unset):
        effective_preset_id (None | str | Unset):
        effective_variant_id (None | str | Unset):
        selected_preset_id (None | str | Unset):
        selected_variant_id (None | str | Unset):
    """

    presets: list[ModelConfigPresetOptionResponse]
    variants: list[ModelConfigVariantOptionResponse]
    default_preset_id: None | str | Unset = UNSET
    default_variant_id: None | str | Unset = UNSET
    effective_preset_id: None | str | Unset = UNSET
    effective_variant_id: None | str | Unset = UNSET
    selected_preset_id: None | str | Unset = UNSET
    selected_variant_id: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        presets = []
        for presets_item_data in self.presets:
            presets_item = presets_item_data.to_dict()
            presets.append(presets_item)

        variants = []
        for variants_item_data in self.variants:
            variants_item = variants_item_data.to_dict()
            variants.append(variants_item)

        default_preset_id: None | str | Unset
        if isinstance(self.default_preset_id, Unset):
            default_preset_id = UNSET
        else:
            default_preset_id = self.default_preset_id

        default_variant_id: None | str | Unset
        if isinstance(self.default_variant_id, Unset):
            default_variant_id = UNSET
        else:
            default_variant_id = self.default_variant_id

        effective_preset_id: None | str | Unset
        if isinstance(self.effective_preset_id, Unset):
            effective_preset_id = UNSET
        else:
            effective_preset_id = self.effective_preset_id

        effective_variant_id: None | str | Unset
        if isinstance(self.effective_variant_id, Unset):
            effective_variant_id = UNSET
        else:
            effective_variant_id = self.effective_variant_id

        selected_preset_id: None | str | Unset
        if isinstance(self.selected_preset_id, Unset):
            selected_preset_id = UNSET
        else:
            selected_preset_id = self.selected_preset_id

        selected_variant_id: None | str | Unset
        if isinstance(self.selected_variant_id, Unset):
            selected_variant_id = UNSET
        else:
            selected_variant_id = self.selected_variant_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "presets": presets,
                "variants": variants,
            }
        )
        if default_preset_id is not UNSET:
            field_dict["default_preset_id"] = default_preset_id
        if default_variant_id is not UNSET:
            field_dict["default_variant_id"] = default_variant_id
        if effective_preset_id is not UNSET:
            field_dict["effective_preset_id"] = effective_preset_id
        if effective_variant_id is not UNSET:
            field_dict["effective_variant_id"] = effective_variant_id
        if selected_preset_id is not UNSET:
            field_dict["selected_preset_id"] = selected_preset_id
        if selected_variant_id is not UNSET:
            field_dict["selected_variant_id"] = selected_variant_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.model_config_preset_option_response import (
            ModelConfigPresetOptionResponse,
        )
        from ..models.model_config_variant_option_response import (
            ModelConfigVariantOptionResponse,
        )

        d = dict(src_dict)
        presets = []
        _presets = d.pop("presets")
        for presets_item_data in _presets:
            presets_item = ModelConfigPresetOptionResponse.from_dict(presets_item_data)

            presets.append(presets_item)

        variants = []
        _variants = d.pop("variants")
        for variants_item_data in _variants:
            variants_item = ModelConfigVariantOptionResponse.from_dict(
                variants_item_data
            )

            variants.append(variants_item)

        def _parse_default_preset_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        default_preset_id = _parse_default_preset_id(d.pop("default_preset_id", UNSET))

        def _parse_default_variant_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        default_variant_id = _parse_default_variant_id(
            d.pop("default_variant_id", UNSET)
        )

        def _parse_effective_preset_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        effective_preset_id = _parse_effective_preset_id(
            d.pop("effective_preset_id", UNSET)
        )

        def _parse_effective_variant_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        effective_variant_id = _parse_effective_variant_id(
            d.pop("effective_variant_id", UNSET)
        )

        def _parse_selected_preset_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        selected_preset_id = _parse_selected_preset_id(
            d.pop("selected_preset_id", UNSET)
        )

        def _parse_selected_variant_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        selected_variant_id = _parse_selected_variant_id(
            d.pop("selected_variant_id", UNSET)
        )

        model_config_selection_response = cls(
            presets=presets,
            variants=variants,
            default_preset_id=default_preset_id,
            default_variant_id=default_variant_id,
            effective_preset_id=effective_preset_id,
            effective_variant_id=effective_variant_id,
            selected_preset_id=selected_preset_id,
            selected_variant_id=selected_variant_id,
        )

        model_config_selection_response.additional_properties = d
        return model_config_selection_response

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
