from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="UpdateModelConfigSelectionRequest")


@_attrs_define
class UpdateModelConfigSelectionRequest:
    """Request body for `PUT /v1/models/{id}/config-selection`.

    Attributes:
        selected_preset_id (None | str | Unset):
        selected_variant_id (None | str | Unset):
    """

    selected_preset_id: None | str | Unset = UNSET
    selected_variant_id: None | str | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
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

        field_dict.update({})
        if selected_preset_id is not UNSET:
            field_dict["selected_preset_id"] = selected_preset_id
        if selected_variant_id is not UNSET:
            field_dict["selected_variant_id"] = selected_variant_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

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

        update_model_config_selection_request = cls(
            selected_preset_id=selected_preset_id,
            selected_variant_id=selected_variant_id,
        )

        return update_model_config_selection_request
